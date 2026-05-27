//! memberclub-api — capstone integration service.
//!
//! Combines the patterns from:
//!   * `projects/03-notes-api` (Axum router, problem-details errors, metrics
//!     middleware, request-id propagation)
//!   * `projects/04-auth-demo` (argon2 password hashing, signed session
//!     cookies, HS256 JWT, AuthenticatedUser extractor)
//!   * `projects/05-rbac-policy-lab` (pure-function policy predicates,
//!     Forbidden enum, the `require!` macro)
//!
//! into one self-contained Axum service. No path dependencies on the
//! individual `projects/*` crates — every pattern lives inline so the
//! whole service can be read top-to-bottom.

pub mod auth;
pub mod billing;
pub mod notes;
pub mod policy;

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use axum::Json;
use axum::Router;
use axum::extract::{FromRef, MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::{Cookie, Key, SameSite};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::info_span;

use crate::auth::{
    ACCESS_TTL_SECS, AuthenticatedUser, Jwt, User, UserDto, audit, fetch_user_by_email, password,
    random_token_b64url, sessions, validate_email, validate_password,
};
use crate::policy::Forbidden;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cookie_key: Key,
    pub jwt: Arc<Jwt>,
    pub metrics: Arc<PrometheusHandle>,
    /// Stripe webhook signing secret. `None` disables `/webhooks/stripe`
    /// — useful in test environments where Stripe isn't configured.
    pub stripe_webhook_secret: Option<Arc<str>>,
}

/// Cache the first installed Prometheus handle so subsequent `AppState::new`
/// calls in the same process (e.g. across tests) reuse the same render
/// surface that the global recorder actually writes to.
static METRICS_HANDLE: OnceLock<Arc<PrometheusHandle>> = OnceLock::new();

impl AppState {
    pub fn new(pool: SqlitePool, cookie_key: Key, jwt: Jwt) -> Self {
        let metrics = METRICS_HANDLE
            .get_or_init(|| {
                let handle = PrometheusBuilder::new()
                    .install_recorder()
                    .unwrap_or_else(|_| PrometheusBuilder::new().build_recorder().handle());
                Arc::new(handle)
            })
            .clone();

        metrics::describe_counter!(
            "http_requests_total",
            "All HTTP requests, by route + method + status class"
        );
        metrics::describe_histogram!(
            "http_request_duration_seconds",
            "Request latency, by route + method"
        );

        Self {
            pool,
            cookie_key,
            jwt: Arc::new(jwt),
            metrics,
            stripe_webhook_secret: None,
        }
    }

    /// Builder: attach the Stripe webhook secret. Without this the
    /// `/webhooks/stripe` handler responds 401 to every request.
    #[must_use]
    pub fn with_stripe_webhook_secret(mut self, secret: impl Into<Arc<str>>) -> Self {
        self.stripe_webhook_secret = Some(secret.into());
        self
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(s: &AppState) -> Key {
        s.cookie_key.clone()
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
        .merge(notes::router());

    Router::new()
        .route("/healthz", get(health))
        .route("/metrics", get(metrics_handler))
        .nest("/v1", v1)
        // Phase 8 — billing routes (Checkout, Portal, /webhooks/stripe).
        // The webhook is intentionally NOT under /v1: Stripe doesn't
        // know our version namespace, and a future v1-only rate-limit
        // shouldn't throttle Stripe deliveries.
        .merge(billing::router())
        .with_state(state)
        .layer(middleware::from_fn(record_metrics))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                let req_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("-");
                info_span!("http", method=%req.method(), uri=%req.uri(), req_id = %req_id)
            }),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

async fn metrics_handler(State(s): State<AppState>) -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        s.metrics.render(),
    )
}

async fn record_metrics(req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_string(), |p| p.as_str().to_string());

    if route == "/metrics" {
        return next.run(req).await;
    }

    let method = req.method().clone();
    let started = Instant::now();
    let res = next.run(req).await;
    let elapsed = started.elapsed().as_secs_f64();
    let status_class = status_class_of(res.status());

    metrics::counter!(
        "http_requests_total",
        "route" => route.clone(),
        "method" => method.to_string(),
        "status_class" => status_class.to_string(),
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "route" => route,
        "method" => method.to_string(),
    )
    .record(elapsed);

    res
}

fn status_class_of(status: StatusCode) -> &'static str {
    match status.as_u16() / 100 {
        1 => "1xx",
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Auth handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: UserDto,
    pub access_token: String,
    pub expires_in: i64,
}

const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 30;

#[tracing::instrument(skip(s, jar, body), fields(email = %body.email))]
async fn register(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<RegisterBody>,
) -> Result<(StatusCode, SignedCookieJar, Json<LoginResponse>), ApiError> {
    validate_email(&body.email)?;
    validate_password(&body.password)?;

    let hash = password::hash(&body.password).map_err(|e| ApiError::Password(e.to_string()))?;

    let sql = format!(
        "INSERT INTO users (email, password_hash) VALUES (?, ?) RETURNING {}",
        auth::USER_COLUMNS
    );
    let user: User = sqlx::query_as::<_, User>(&sql)
        .bind(&body.email)
        .bind(&hash)
        .fetch_one(&s.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                ApiError::EmailAlreadyRegistered
            }
            _ => ApiError::Db(e),
        })?;

    audit(&s.pool, Some(user.id), "user.registered", None).await?;

    // Session + JWT — same dual-mode as auth-demo: cookie for browser, token
    // in the JSON body for API clients.
    let (jar, response) = issue_session(&s, jar, user).await?;
    Ok((StatusCode::CREATED, jar, Json(response)))
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[tracing::instrument(skip(s, body), fields(email = %body.email))]
async fn login(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<LoginBody>,
) -> Result<(SignedCookieJar, Json<LoginResponse>), ApiError> {
    let user = fetch_user_by_email(&s.pool, &body.email).await?;

    let Some(user) = user else {
        // Constant-time-ish: still hash a sentinel.
        let _ = password::verify(&body.password, password::SENTINEL_HASH);
        return Err(ApiError::InvalidCredentials);
    };
    if !password::verify(&body.password, &user.password_hash) {
        return Err(ApiError::InvalidCredentials);
    }

    audit(&s.pool, Some(user.id), "user.logged_in", None).await?;

    let (jar, response) = issue_session(&s, jar, user).await?;
    Ok((jar, Json(response)))
}

#[tracing::instrument(skip(s, jar))]
async fn logout(
    State(s): State<AppState>,
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, StatusCode), ApiError> {
    if let Some(c) = jar.get("session") {
        sessions::revoke(&s.pool, c.value()).await?;
    }
    let cleared = Cookie::build(("session", ""))
        .http_only(true)
        .secure(cfg!(not(debug_assertions)))
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    Ok((jar.add(cleared), StatusCode::NO_CONTENT))
}

#[tracing::instrument(skip(user), fields(user_id = user.0.id))]
async fn me(user: AuthenticatedUser) -> Json<UserDto> {
    Json(UserDto::from(user.0))
}

async fn issue_session(
    s: &AppState,
    jar: SignedCookieJar,
    user: User,
) -> Result<(SignedCookieJar, LoginResponse), ApiError> {
    let token = random_token_b64url(32);
    sessions::insert(&s.pool, user.id, &token, SESSION_TTL_SECS).await?;
    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .secure(cfg!(not(debug_assertions)))
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();
    let access_token = s.jwt.issue_access(user.id)?;
    Ok((
        jar.add(cookie),
        LoginResponse {
            user: UserDto::from(user),
            access_token,
            expires_in: ACCESS_TTL_SECS,
        },
    ))
}

// ---------------------------------------------------------------------------
// ApiError + IntoResponse — problem-details (RFC 7807)
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden(Forbidden),
    #[error("not found")]
    NotFound,
    #[error("email already registered")]
    EmailAlreadyRegistered,
    #[error("invalid password — must be 12+ characters")]
    WeakPassword,
    #[error("invalid email")]
    InvalidEmail,
    #[error("body cannot be empty")]
    EmptyBody,
    #[error("body cannot exceed 4096 characters")]
    TooLong,
    #[error("invalid tier — must be one of free|pro|elite")]
    InvalidTier,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("password hashing failure: {0}")]
    Password(String),
    #[error(transparent)]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, title): (StatusCode, &'static str, &'static str) = match &self {
            ApiError::InvalidCredentials | ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "https://memberclub.test/problems/unauthorized",
                "Unauthorized",
            ),
            ApiError::Forbidden(_) => (
                StatusCode::FORBIDDEN,
                "https://memberclub.test/problems/forbidden",
                "Forbidden",
            ),
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                "https://memberclub.test/problems/not-found",
                "Not Found",
            ),
            ApiError::EmailAlreadyRegistered => (
                StatusCode::CONFLICT,
                "https://memberclub.test/problems/conflict",
                "Conflict",
            ),
            ApiError::WeakPassword
            | ApiError::InvalidEmail
            | ApiError::EmptyBody
            | ApiError::TooLong
            | ApiError::InvalidTier
            | ApiError::BadRequest(_) => (
                StatusCode::BAD_REQUEST,
                "https://memberclub.test/problems/invalid-input",
                "Bad Request",
            ),
            ApiError::Db(_) | ApiError::Password(_) | ApiError::Jwt(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "https://memberclub.test/problems/internal",
                "Internal Server Error",
            ),
        };

        if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "request failed");
        } else if status == StatusCode::UNAUTHORIZED {
            // Don't leak which step failed.
            tracing::warn!(status = %status, "auth rejection");
        } else {
            tracing::warn!(error = %self, status = %status, "client error");
        }

        let detail = if status.is_server_error() {
            "internal error".to_string()
        } else if status == StatusCode::UNAUTHORIZED {
            "invalid credentials".to_string()
        } else if status == StatusCode::FORBIDDEN {
            // Generic message on the wire — the variant is logged + audited.
            "request not permitted".to_string()
        } else {
            self.to_string()
        };

        let mut resp = (
            status,
            Json(ProblemDetails {
                kind,
                title,
                status: status.as_u16(),
                detail,
            }),
        )
            .into_response();
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        resp
    }
}
