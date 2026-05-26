//! auth-demo — argon2 password hashing + signed session cookies + RS256 JWT,
//! demonstrating the enterprise dual-mode authentication pattern.

pub mod jwt;
pub mod password;
pub mod sessions;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum_extra::extract::SignedCookieJar;
use axum_extra::extract::cookie::{Cookie, Key, SameSite};
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cookie_key: Key,
    pub jwt: Arc<jwt::Jwt>,
}

impl AppState {
    pub fn new(pool: SqlitePool, cookie_key: Key, jwt: jwt::Jwt) -> Self {
        Self {
            pool,
            cookie_key,
            jwt: Arc::new(jwt),
        }
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
    Router::new()
        .route("/healthz", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/refresh", post(refresh))
        .route("/me", get(me))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

// ---------------------------------------------------------------------------
// Domain model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub is_email_verified: i64,
    pub is_admin: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    pub id: i64,
    pub email: String,
    pub is_email_verified: bool,
    pub is_admin: bool,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            is_email_verified: u.is_email_verified == 1,
            is_admin: u.is_admin == 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unauthorized")]
    Unauthorized,
    #[error("email already registered")]
    EmailAlreadyRegistered,
    #[error("invalid password — must be 12+ characters")]
    WeakPassword,
    #[error("invalid email")]
    InvalidEmail,
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
    fn into_response(self) -> axum::response::Response {
        let (status, kind, title): (StatusCode, &'static str, &'static str) = match &self {
            ApiError::InvalidCredentials | ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "https://memberclub.test/problems/unauthorized",
                "Unauthorized",
            ),
            ApiError::EmailAlreadyRegistered => (
                StatusCode::CONFLICT,
                "https://memberclub.test/problems/email-taken",
                "Conflict",
            ),
            ApiError::WeakPassword | ApiError::InvalidEmail => (
                StatusCode::BAD_REQUEST,
                "https://memberclub.test/problems/invalid-input",
                "Bad Request",
            ),
            _ => (
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

// ---------------------------------------------------------------------------
// Handlers
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
    pub refresh_token: String,
    pub expires_in: i64,
}

async fn register(
    State(s): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    validate_email(&body.email)?;
    validate_password(&body.password)?;

    let hash = password::hash(&body.password).map_err(|e| ApiError::Password(e.to_string()))?;

    let user: User = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash) VALUES (?, ?)
         RETURNING id, email, password_hash, is_email_verified, is_admin, created_at, updated_at",
    )
    .bind(&body.email)
    .bind(&hash)
    .fetch_one(&s.pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::EmailAlreadyRegistered,
        _ => ApiError::Db(e),
    })?;

    audit(&s.pool, Some(user.id), "user.registered", None).await?;
    Ok((StatusCode::CREATED, Json(UserDto::from(user))))
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

async fn login(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<LoginBody>,
) -> Result<(SignedCookieJar, Json<LoginResponse>), ApiError> {
    let user: Option<User> = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, is_email_verified, is_admin, created_at, updated_at
         FROM users WHERE email = ?",
    )
    .bind(&body.email)
    .fetch_optional(&s.pool)
    .await?;

    let Some(user) = user else {
        // Constant-time-ish: still hash a sentinel so timing doesn't leak existence.
        let _ = password::verify(&body.password, password::SENTINEL_HASH);
        return Err(ApiError::InvalidCredentials);
    };
    if !password::verify(&body.password, &user.password_hash) {
        return Err(ApiError::InvalidCredentials);
    }

    // Issue a session cookie.
    let session_token = random_token_b64url(32);
    sessions::insert(&s.pool, user.id, &session_token, 60 * 60 * 24 * 30).await?;
    let cookie = Cookie::build(("session", session_token))
        .http_only(true)
        .secure(cfg!(not(debug_assertions)))
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();

    // Issue access + refresh JWTs.
    let access_token = s.jwt.issue_access(user.id)?;
    let refresh_token = s.jwt.issue_refresh(user.id)?;

    audit(&s.pool, Some(user.id), "user.logged_in", None).await?;

    Ok((
        jar.add(cookie),
        Json(LoginResponse {
            user: UserDto::from(user),
            access_token,
            refresh_token,
            expires_in: jwt::ACCESS_TTL_SECS,
        }),
    ))
}

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

#[derive(Debug, Deserialize)]
pub struct RefreshBody {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

async fn refresh(
    State(s): State<AppState>,
    Json(body): Json<RefreshBody>,
) -> Result<Json<RefreshResponse>, ApiError> {
    let claims = s.jwt.verify_refresh(&body.refresh_token)?;
    let user_id: i64 = claims.sub.parse().map_err(|_| ApiError::Unauthorized)?;
    let access_token = s.jwt.issue_access(user_id)?;
    let refresh_token = s.jwt.issue_refresh(user_id)?; // rotation
    Ok(Json(RefreshResponse {
        access_token,
        refresh_token,
        expires_in: jwt::ACCESS_TTL_SECS,
    }))
}

async fn me(user: AuthenticatedUser) -> Json<UserDto> {
    Json(UserDto::from(user.0))
}

// ---------------------------------------------------------------------------
// Extractor — accepts either Authorization: Bearer ... or a signed cookie.
// ---------------------------------------------------------------------------

pub struct AuthenticatedUser(pub User);

impl axum::extract::FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 1. Authorization: Bearer <jwt>
        if let Some(token) = bearer_from(&parts.headers) {
            let claims = state.jwt.verify_access(&token)?;
            let user_id: i64 = claims.sub.parse().map_err(|_| ApiError::Unauthorized)?;
            let user = fetch_user(&state.pool, user_id)
                .await?
                .ok_or(ApiError::Unauthorized)?;
            return Ok(Self(user));
        }

        // 2. Signed session cookie
        let jar = SignedCookieJar::from_headers(&parts.headers, state.cookie_key.clone());
        if let Some(cookie) = jar.get("session") {
            if let Some(session) = sessions::find_active(&state.pool, cookie.value()).await? {
                let user = fetch_user(&state.pool, session.user_id)
                    .await?
                    .ok_or(ApiError::Unauthorized)?;
                return Ok(Self(user));
            }
        }

        Err(ApiError::Unauthorized)
    }
}

fn bearer_from(headers: &axum::http::HeaderMap) -> Option<String> {
    let v = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = v
        .strip_prefix("Bearer ")
        .or_else(|| v.strip_prefix("bearer "))?;
    Some(token.to_string())
}

async fn fetch_user(pool: &SqlitePool, id: i64) -> Result<Option<User>, ApiError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, is_email_verified, is_admin, created_at, updated_at
         FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_email(email: &str) -> Result<(), ApiError> {
    if email.len() > 254 || !email.contains('@') || email.contains(' ') {
        return Err(ApiError::InvalidEmail);
    }
    let mut parts = email.split('@');
    let (local, domain) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return Err(ApiError::InvalidEmail);
    }
    Ok(())
}

fn validate_password(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < 12 {
        return Err(ApiError::WeakPassword);
    }
    Ok(())
}

pub fn random_token_b64url(bytes: usize) -> String {
    use base64::Engine;
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

async fn audit(
    pool: &SqlitePool,
    actor_id: Option<i64>,
    action: &str,
    detail: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO audit_logs (actor_id, action, detail) VALUES (?, ?, ?)")
        .bind(actor_id)
        .bind(action)
        .bind(detail)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
