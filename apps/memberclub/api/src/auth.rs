//! Authentication primitives — inlined from `projects/04-auth-demo/`.
//!
//! - `password`: argon2id hashing + verify.
//! - `sessions`: random token in a signed cookie; SHA-256 of the token in the DB.
//! - `jwt`: HS256 access-token issuing + verifying.
//! - `AuthenticatedUser` extractor — accepts either a signed cookie OR a
//!   Bearer JWT.
//!
//! This is the dual-mode pattern: browsers carry cookies, API clients
//! carry tokens, and the server speaks both via one extractor.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, password_hash::rand_core::OsRng as Argon2Rng};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::SignedCookieJar;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::TryRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{ApiError, AppState};

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub tier: String,
    pub org_id: i64,
    pub email_verified: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDto {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub tier: String,
    pub org_id: i64,
    pub email_verified: bool,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            role: u.role,
            tier: u.tier,
            org_id: u.org_id,
            email_verified: u.email_verified == 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Password (argon2id)
// ---------------------------------------------------------------------------

pub mod password {
    use super::{Argon2, Argon2Rng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

    /// Fixed sentinel hash used during login when the user does not exist —
    /// keeps the verify-path timing roughly constant.
    pub static SENTINEL_HASH: &str = "$argon2id$v=19$m=65536,t=3,p=4$ZGV2c2VudGluZWxzYWx0$M+pq1Mhgw3qfb8MK1pSwfn9k0NLm6/MhAJjvgMTcUjk";

    pub fn hash(plain: &str) -> Result<String, argon2::password_hash::Error> {
        let salt = SaltString::generate(&mut Argon2Rng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(plain.as_bytes(), &salt)?;
        Ok(hash.to_string())
    }

    pub fn verify(plain: &str, stored: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return false;
        };
        Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// Sessions — cookie carries a random token; DB stores SHA-256 of it.
// ---------------------------------------------------------------------------

pub mod sessions {
    use super::{Digest, Duration, Sha256, SqlitePool, Utc};

    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct Session {
        pub id: i64,
        pub user_id: i64,
    }

    #[must_use]
    pub fn token_hash(token: &str) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(token.as_bytes());
        h.finalize().to_vec()
    }

    pub async fn insert(
        pool: &SqlitePool,
        user_id: i64,
        token: &str,
        ttl_secs: i64,
    ) -> Result<(), sqlx::Error> {
        let expires_at = (Utc::now() + Duration::seconds(ttl_secs)).to_rfc3339();
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, ?)")
            .bind(token_hash(token))
            .bind(user_id)
            .bind(expires_at)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn revoke(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE sessions SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE token_hash = ?",
        )
        .bind(token_hash(token))
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_active(
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<Session>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let s: Option<Session> = sqlx::query_as::<_, Session>(
            "SELECT id, user_id FROM sessions \
             WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?",
        )
        .bind(token_hash(token))
        .bind(now)
        .fetch_optional(pool)
        .await?;
        Ok(s)
    }
}

// ---------------------------------------------------------------------------
// JWT — HS256, single secret. Access-only (no refresh in this integration).
// ---------------------------------------------------------------------------

pub const ACCESS_TTL_SECS: i64 = 15 * 60;
const ISSUER: &str = "memberclub-api";
const AUDIENCE: &str = "memberclub-api";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub iat: i64,
    pub nbf: i64,
    pub exp: i64,
    pub jti: String,
}

pub struct Jwt {
    enc: EncodingKey,
    dec: DecodingKey,
}

impl Jwt {
    #[must_use]
    pub fn new(secret: &[u8]) -> Self {
        Self {
            enc: EncodingKey::from_secret(secret),
            dec: DecodingKey::from_secret(secret),
        }
    }

    #[must_use]
    pub fn random_secret() -> [u8; 32] {
        let mut buf = [0u8; 32];
        rand::rngs::SysRng
            .try_fill_bytes(&mut buf)
            .expect("OS RNG must work");
        buf
    }

    pub fn issue_access(&self, user_id: i64) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            sub: user_id.to_string(),
            iat: now,
            nbf: now,
            exp: now + ACCESS_TTL_SECS,
            jti: jti(),
        };
        let header = Header::new(Algorithm::HS256);
        encode(&header, &claims, &self.enc)
    }

    pub fn verify_access(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut v = Validation::new(Algorithm::HS256);
        v.set_audience(&[AUDIENCE]);
        v.set_issuer(&[ISSUER]);
        v.leeway = 60;
        decode::<Claims>(token, &self.dec, &v).map(|d| d.claims)
    }
}

fn jti() -> String {
    let mut buf = [0u8; 16];
    rand::rngs::SysRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    hex::encode(buf)
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

#[must_use]
pub fn random_token_b64url(bytes: usize) -> String {
    use base64::Engine;
    let mut buf = vec![0u8; bytes];
    rand::rngs::SysRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}

// ---------------------------------------------------------------------------
// AuthenticatedUser extractor — accepts Bearer JWT OR signed session cookie.
// ---------------------------------------------------------------------------

pub struct AuthenticatedUser(pub User);

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
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

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

pub const USER_COLUMNS: &str =
    "id, email, password_hash, role, tier, org_id, email_verified, created_at, updated_at";

pub async fn fetch_user(pool: &SqlitePool, id: i64) -> Result<Option<User>, ApiError> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?");
    let user = sqlx::query_as::<_, User>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn fetch_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>, ApiError> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE email = ?");
    let user = sqlx::query_as::<_, User>(sqlx::AssertSqlSafe(sql))
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

pub fn validate_email(email: &str) -> Result<(), ApiError> {
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

pub fn validate_password(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < 12 {
        return Err(ApiError::WeakPassword);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

pub async fn audit(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let h = password::hash("correct horse battery staple").unwrap();
        assert!(password::verify("correct horse battery staple", &h));
        assert!(!password::verify("wrong", &h));
    }

    #[test]
    fn jwt_access_round_trip() {
        let jwt = Jwt::new(&Jwt::random_secret());
        let t = jwt.issue_access(42).unwrap();
        let claims = jwt.verify_access(&t).unwrap();
        assert_eq!(claims.sub, "42");
    }

    #[test]
    fn jwt_tampered_signature_rejected() {
        let a = Jwt::new(&Jwt::random_secret());
        let b = Jwt::new(&Jwt::random_secret());
        let t = a.issue_access(42).unwrap();
        assert!(b.verify_access(&t).is_err());
    }
}
