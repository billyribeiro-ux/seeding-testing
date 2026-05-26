//! Phase 6.4 — Email verification tokens.
//!
//! A verification token is a 32-byte random value, base64url-encoded for
//! the URL we send to the user's inbox. The server stores only the SHA-256
//! hash, mirroring [`crate::sessions`]: if the DB leaks, an attacker can't
//! redeem links they read out of it.
//!
//! The flow has two halves:
//!
//! * [`issue`] — called from `POST /auth/verify-email/request`. Generates
//!   a fresh token, INSERTs the row, AND atomically invalidates any
//!   previous unused tokens for that user (the lesson 6.4 "resend"
//!   pattern). Returns the plaintext token so the caller can put it in an
//!   email (or, in tests, in the response body).
//! * [`confirm`] — called from `POST /auth/verify-email/confirm`. Looks
//!   up the row by hash, and if it's unused and unexpired, performs three
//!   side-effects in one transaction: marks `used_at`, flips
//!   `users.is_email_verified`, and writes an `audit_logs` row. Returns
//!   `Some(user_id)` on success and `None` for any miss — expired,
//!   unknown, or already-used. Callers map `None` to a 401.

use base64::Engine;
use chrono::{Duration, Utc};
use rand::TryRngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Generate a fresh 32-byte random verification token, base64url-encoded
/// (no padding). Roughly 43 characters; URL-safe and shell-safe.
#[must_use]
pub fn new_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// SHA-256 of the plaintext token. Stored in the `token_hash` column —
/// same shape as [`crate::sessions::token_hash`].
#[must_use]
pub fn token_hash(token: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().to_vec()
}

/// Issue a new verification token for `user_id`. Invalidates any prior
/// unused tokens for that user before inserting the new one — a leaked
/// or forwarded earlier link can no longer be redeemed once a resend
/// has been requested.
///
/// Returns the plaintext token. In production the caller emails it to
/// the user; in tests we return it directly in the response body.
pub async fn issue(pool: &SqlitePool, user_id: i64, ttl_secs: i64) -> Result<String, sqlx::Error> {
    let token = new_token();
    let hash = token_hash(&token);
    let expires_at = (Utc::now() + Duration::seconds(ttl_secs)).to_rfc3339();

    let mut tx = pool.begin().await?;
    // Invalidate any prior unused tokens for this user — the lesson 6.4
    // "resend invalidates the previous link" rule.
    sqlx::query(
        "UPDATE email_verifications
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO email_verifications (user_id, token_hash, expires_at)
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

/// Atomically consume a verification token: mark the row used, flip the
/// user's `is_email_verified` flag, and write an audit log entry — all
/// in a single transaction.
///
/// Returns `Some(user_id)` on success, `None` if the token is unknown,
/// expired, or already used (callers map `None` to 401).
pub async fn confirm(pool: &SqlitePool, token: &str) -> Result<Option<i64>, sqlx::Error> {
    let hash = token_hash(token);

    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM email_verifications
         WHERE token_hash = ?
           AND used_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((id, user_id)) = row else {
        // No matching row — drop the (read-only) txn and report a miss.
        tx.rollback().await?;
        return Ok(None);
    };

    sqlx::query(
        "UPDATE email_verifications
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE users SET is_email_verified = 1 WHERE id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("INSERT INTO audit_logs (actor_id, action, detail) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind("user.email_verified")
        .bind(Option::<&str>::None)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Some(user_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_token_is_url_safe_and_long_enough() {
        let t = new_token();
        // 32 bytes base64url-no-pad => ceil(32 * 4 / 3) = 43 chars.
        assert_eq!(t.len(), 43);
        assert!(
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "token must be URL-safe: {t}"
        );
    }

    #[test]
    fn token_hash_is_deterministic_and_32_bytes() {
        let a = token_hash("hello");
        let b = token_hash("hello");
        let c = token_hash("world");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn two_new_tokens_differ() {
        assert_ne!(new_token(), new_token());
    }
}
