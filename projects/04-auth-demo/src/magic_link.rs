//! Phase 6 stretch E6.8 — passwordless ("magic link") login.
//!
//! Same one-shot token machinery as [`crate::email_verify`] and
//! [`crate::password_reset`]: random plaintext, SHA-256 in the DB,
//! `used_at` for single-use semantics, `expires_at` for TTL. The
//! caller decides what success looks like — for magic-link it's
//! "issue a session + JWT pair the same way password login does."
//!
//! ## Enumeration safety
//!
//! `POST /auth/magic/request` always succeeds (204 + optional debug
//! body) regardless of whether the email is registered, and pads the
//! response time to a constant budget — exactly the shape of the
//! forgot-password handler. An attacker who measures latency or status
//! cannot enumerate accounts.

use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::email_verify::{new_token, token_hash};

/// TTL for a magic-link token. 15 minutes is a tight window — magic
/// links are inherently powerful (they're a session in a URL), so we
/// don't want them sitting in an inbox for hours.
pub const MAGIC_LINK_TTL_SECS: i64 = 60 * 15;

/// Look up a user by email. Stays here (rather than inline in the
/// handler) so tests can probe the constant-time path, paralleling
/// [`crate::password_reset::user_id_by_email`].
pub async fn user_id_by_email(pool: &SqlitePool, email: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await
}

/// Issue a fresh magic-link token for `user_id`. Atomically invalidates
/// any prior unused tokens for that user — re-requesting a magic link
/// must not leave the previously-emailed link still hot.
pub async fn issue(pool: &SqlitePool, user_id: i64, ttl_secs: i64) -> Result<String, sqlx::Error> {
    let token = new_token();
    let hash = token_hash(&token);
    let expires_at = (Utc::now() + Duration::seconds(ttl_secs)).to_rfc3339();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE magic_links
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO magic_links (user_id, token_hash, expires_at)
         VALUES (?, ?, ?)",
    )
    .bind(user_id)
    .bind(&hash)
    .bind(&expires_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(token)
}

/// Atomically consume a magic-link token.
///
/// Returns `Some(user_id)` on success. On a miss — unknown, expired,
/// or already-used — returns `None` and the DB is unchanged. Callers
/// map `None` to a 401.
pub async fn confirm(pool: &SqlitePool, token: &str) -> Result<Option<i64>, sqlx::Error> {
    let hash = token_hash(token);

    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM magic_links
         WHERE token_hash = ?
           AND used_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((id, user_id)) = row else {
        tx.rollback().await?;
        return Ok(None);
    };

    sqlx::query(
        "UPDATE magic_links
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO audit_logs (actor_id, action, detail) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind("user.magic_link_consumed")
        .bind(Option::<&str>::None)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Some(user_id))
}
