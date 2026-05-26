//! auth-demo — argon2 password hashing + signed session cookies + RS256 JWT,
//! demonstrating the enterprise dual-mode authentication pattern.

pub mod email_verify;
pub mod jwt;
pub mod password;
pub mod password_reset;
pub mod sessions;
pub mod totp;

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
        .route("/auth/totp/enroll", post(totp_enroll))
        .route("/auth/totp/confirm", post(totp_confirm))
        .route("/auth/totp/disable", post(totp_disable))
        .route("/auth/verify-email/request", post(verify_email_request))
        .route("/auth/verify-email/confirm", post(verify_email_confirm))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
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
    pub totp_secret: Option<String>,
    pub totp_enabled: i64,
    pub totp_last_verified_at: Option<String>,
}

impl User {
    #[must_use]
    pub fn has_totp(&self) -> bool {
        self.totp_enabled == 1
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    pub id: i64,
    pub email: String,
    pub is_email_verified: bool,
    pub is_admin: bool,
    pub totp_enabled: bool,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            is_email_verified: u.is_email_verified == 1,
            is_admin: u.is_admin == 1,
            totp_enabled: u.totp_enabled == 1,
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
    #[error("totp required for this account")]
    TotpRequired,
    #[error("invalid totp code")]
    InvalidTotp,
    #[error("totp is already enabled; disable it first to re-enroll")]
    TotpAlreadyEnabled,
    #[error("totp is not enabled for this account")]
    TotpNotEnabled,
    #[error("verification link is invalid, expired, or already used")]
    InvalidVerificationToken,
    #[error("password reset link is invalid, expired, or already used")]
    InvalidResetToken,
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
            ApiError::InvalidCredentials
            | ApiError::Unauthorized
            | ApiError::TotpRequired
            | ApiError::InvalidTotp
            | ApiError::InvalidVerificationToken
            | ApiError::InvalidResetToken => (
                StatusCode::UNAUTHORIZED,
                "https://memberclub.test/problems/unauthorized",
                "Unauthorized",
            ),
            ApiError::EmailAlreadyRegistered | ApiError::TotpAlreadyEnabled => (
                StatusCode::CONFLICT,
                "https://memberclub.test/problems/conflict",
                "Conflict",
            ),
            ApiError::WeakPassword | ApiError::InvalidEmail | ApiError::TotpNotEnabled => (
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
         RETURNING id, email, password_hash, is_email_verified, is_admin, created_at, updated_at,
                   totp_secret, totp_enabled, totp_last_verified_at",
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
    /// 6-digit TOTP code. Required if the account has 2FA enabled.
    /// Mutually exclusive with `recovery_code`.
    #[serde(default)]
    pub totp: Option<String>,
    /// One-time recovery code. Consumed on use.
    #[serde(default)]
    pub recovery_code: Option<String>,
}

async fn login(
    State(s): State<AppState>,
    jar: SignedCookieJar,
    Json(body): Json<LoginBody>,
) -> Result<(SignedCookieJar, Json<LoginResponse>), ApiError> {
    let user: Option<User> = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, is_email_verified, is_admin, created_at, updated_at,
                totp_secret, totp_enabled, totp_last_verified_at
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

    // Second factor — required iff the user has TOTP enabled.
    if user.has_totp() {
        let ok = if let Some(code) = body.totp.as_deref() {
            totp::verify_and_maybe_enable(&s.pool, user.id, code).await?
        } else if let Some(code) = body.recovery_code.as_deref() {
            totp::consume_recovery_code(&s.pool, user.id, code).await?
        } else {
            return Err(ApiError::TotpRequired);
        };
        if !ok {
            return Err(ApiError::InvalidTotp);
        }
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
// TOTP handlers (Phase 6.6)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TotpEnrollmentDto {
    pub secret: String,
    pub provisioning_uri: String,
    /// Shown ONCE. The server only stores their hashes.
    pub recovery_codes: Vec<String>,
}

/// Begin TOTP enrollment for the authenticated user. The response carries
/// the secret + provisioning URI + 8 recovery codes; the client renders
/// the URI as a QR code and shows the recovery codes once. TOTP is NOT
/// yet enabled — the user must call `/auth/totp/confirm` with a valid
/// code to finalize.
async fn totp_enroll(
    State(s): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<TotpEnrollmentDto>, ApiError> {
    if user.0.has_totp() {
        return Err(ApiError::TotpAlreadyEnabled);
    }
    let art = totp::begin_enrollment(&s.pool, user.0.id, &user.0.email).await?;
    audit(&s.pool, Some(user.0.id), "totp.enrollment_started", None).await?;
    Ok(Json(TotpEnrollmentDto {
        secret: art.secret_b32,
        provisioning_uri: art.provisioning_uri,
        recovery_codes: art.recovery_codes,
    }))
}

#[derive(Debug, Deserialize)]
pub struct TotpCodeBody {
    pub code: String,
}

/// Confirm enrollment by submitting a current 6-digit code. On success,
/// `totp_enabled` flips to true and every subsequent login requires TOTP.
async fn totp_confirm(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Json(body): Json<TotpCodeBody>,
) -> Result<StatusCode, ApiError> {
    if user.0.has_totp() {
        return Err(ApiError::TotpAlreadyEnabled);
    }
    let ok = totp::verify_and_maybe_enable(&s.pool, user.0.id, &body.code).await?;
    if !ok {
        return Err(ApiError::InvalidTotp);
    }
    audit(&s.pool, Some(user.0.id), "totp.enabled", None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct TotpDisableBody {
    /// Step-up: must submit a fresh 6-digit code (or a recovery code) at
    /// disable time. Prevents a stolen session from quietly removing 2FA.
    pub code: String,
}

async fn totp_disable(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Json(body): Json<TotpDisableBody>,
) -> Result<StatusCode, ApiError> {
    if !user.0.has_totp() {
        return Err(ApiError::TotpNotEnabled);
    }
    let ok = totp::verify_and_maybe_enable(&s.pool, user.0.id, &body.code).await?
        || totp::consume_recovery_code(&s.pool, user.0.id, &body.code).await?;
    if !ok {
        return Err(ApiError::InvalidTotp);
    }
    totp::disable(&s.pool, user.0.id).await?;
    audit(&s.pool, Some(user.0.id), "totp.disabled", None).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Email verification handlers (Phase 6.4)
// ---------------------------------------------------------------------------

/// TTL for an email-verification token. 24 hours is the conventional
/// "click the link in your inbox" window.
const EMAIL_VERIFY_TTL_SECS: i64 = 60 * 60 * 24;

/// Request a new email-verification token for the authenticated user.
///
/// In production this endpoint would email the link to the user and
/// respond with `{"sent": true}` so the response body never carries the
/// secret. In dev/test builds (`debug_assertions`) we additionally
/// include the plaintext token in the response so integration tests can
/// drive the confirm endpoint without an inbox.
///
/// Calling this endpoint a second time invalidates any previous unused
/// tokens for the same user — see [`email_verify::issue`].
#[tracing::instrument(skip(s, user))]
async fn verify_email_request(
    State(s): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = email_verify::issue(&s.pool, user.0.id, EMAIL_VERIFY_TTL_SECS).await?;
    audit(
        &s.pool,
        Some(user.0.id),
        "user.email_verification_requested",
        None,
    )
    .await?;

    // In a real deployment, hand the token to the mail sender here and
    // respond with just `{"sent": true}`. In debug builds we leak the
    // token in the response body so tests don't need an SMTP server.
    if cfg!(debug_assertions) {
        Ok(Json(serde_json::json!({
            "sent": true,
            "token": token,
        })))
    } else {
        // Token is intentionally discarded in release builds — the mailer
        // would have consumed it. We drop it explicitly to make that
        // intent visible (and to avoid an unused-variable warning).
        drop(token);
        Ok(Json(serde_json::json!({ "sent": true })))
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailConfirmBody {
    pub token: String,
}

/// Confirm an email-verification token. No authentication required —
/// the token itself is the credential. On success the user's
/// `is_email_verified` flag is flipped to 1 in the same transaction
/// that consumes the token and writes the audit-log row.
#[tracing::instrument(skip(s, body))]
async fn verify_email_confirm(
    State(s): State<AppState>,
    Json(body): Json<VerifyEmailConfirmBody>,
) -> Result<StatusCode, ApiError> {
    match email_verify::confirm(&s.pool, &body.token).await? {
        Some(_user_id) => Ok(StatusCode::NO_CONTENT),
        None => Err(ApiError::InvalidVerificationToken),
    }
}

// ---------------------------------------------------------------------------
// Password reset handlers (Phase 6.5)
// ---------------------------------------------------------------------------

/// TTL for a password-reset token. 1 hour: short enough that a stolen
/// inbox doesn't sit on a live reset for days, long enough that a user
/// who clicks "forgot password" before lunch can finish after.
const PASSWORD_RESET_TTL_SECS: i64 = 60 * 60;

/// Total time budget for `POST /auth/forgot-password`. We pad to this so
/// the response time doesn't reveal whether the email is registered.
/// 250 ms is comfortably above the argon2 hash + DB write a real
/// reset takes; it can be tuned per deployment.
const FORGOT_PASSWORD_TIME_BUDGET_MS: u64 = 250;

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordBody {
    pub email: String,
}

/// Request a password reset.
///
/// Always returns `204 No Content`, regardless of whether the email
/// exists. The response time is padded to a fixed budget so an attacker
/// can't distinguish "found" from "not found" by latency either.
///
/// Internally:
///   * Real user → [`password_reset::issue`] mints a token, the response
///     body (debug builds only) echoes it for test convenience.
///   * Unknown email → no DB write, but the padding sleep still runs.
///
/// In dev/test builds the response body carries `{"sent": true, "token":
/// "..."}` when the user existed; in release builds it is always an
/// empty 204 (the token would be sent over SMTP by the mailer).
#[tracing::instrument(skip(s, body), fields(email = %body.email))]
async fn forgot_password(
    State(s): State<AppState>,
    Json(body): Json<ForgotPasswordBody>,
) -> Result<axum::response::Response, ApiError> {
    let started = std::time::Instant::now();

    // Do the lookup. Issue a token only if the user is real; otherwise
    // skip the DB write but spend the same wall-clock time below.
    let mut issued: Option<(i64, String)> = None;
    if validate_email(&body.email).is_ok()
        && let Some(user_id) = password_reset::user_id_by_email(&s.pool, &body.email).await?
    {
        let token = password_reset::issue(&s.pool, user_id, PASSWORD_RESET_TTL_SECS).await?;
        audit(
            &s.pool,
            Some(user_id),
            "user.password_reset_requested",
            None,
        )
        .await?;
        issued = Some((user_id, token));
    }

    // Constant-time pad — sleep so all responses land at the same
    // wall-clock duration regardless of which branch above ran.
    let target = std::time::Duration::from_millis(FORGOT_PASSWORD_TIME_BUDGET_MS);
    if let Some(rest) = target.checked_sub(started.elapsed()) {
        tokio::time::sleep(rest).await;
    }

    // Debug builds leak the token in the response so integration tests
    // don't need an SMTP server. Release builds always 204.
    if cfg!(debug_assertions)
        && let Some((_uid, token)) = issued
    {
        return Ok(Json(serde_json::json!({
            "sent": true,
            "token": token,
        }))
        .into_response());
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordBody {
    pub token: String,
    pub new_password: String,
}

/// Complete a password reset.
///
/// Validates the new password the same way `register` does, hashes it
/// with argon2id, then asks [`password_reset::complete`] to perform the
/// four-statement transaction (consume token, swap hash, revoke
/// sessions, audit). On a miss (unknown/expired/used) the response is
/// `401` and nothing has changed in the DB.
#[tracing::instrument(skip(s, body))]
async fn reset_password(
    State(s): State<AppState>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<StatusCode, ApiError> {
    validate_password(&body.new_password)?;
    let new_hash =
        password::hash(&body.new_password).map_err(|e| ApiError::Password(e.to_string()))?;
    match password_reset::complete(&s.pool, &body.token, &new_hash).await? {
        Some(_user_id) => Ok(StatusCode::NO_CONTENT),
        None => Err(ApiError::InvalidResetToken),
    }
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
        "SELECT id, email, password_hash, is_email_verified, is_admin, created_at, updated_at,
                totp_secret, totp_enabled, totp_last_verified_at
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
