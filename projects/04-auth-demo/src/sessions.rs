//! Session storage: the cookie holds a random token; the DB stores its SHA-256.

use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
}

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
    sqlx::query("UPDATE sessions SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE token_hash = ?")
        .bind(token_hash(token))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_active(pool: &SqlitePool, token: &str) -> Result<Option<Session>, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let s: Option<Session> = sqlx::query_as::<_, Session>(
        "SELECT id, user_id FROM sessions
         WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?",
    )
    .bind(token_hash(token))
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(s)
}
