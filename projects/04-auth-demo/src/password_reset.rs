//! Phase 6.5 — password reset tokens.
//!
//! Same machinery as [`crate::email_verify`] with two extra rules that the
//! lesson is loud about:
//!
//!   1. **Don't leak whether the email exists.** `POST /auth/forgot-password`
//!      always returns the same status, in the same time. Issue a token
//!      only when the user is real; otherwise do equivalent work so the
//!      timing channel stays closed.
//!   2. **Revoke every active session on success.** A phished password
//!      may still be alive in a session somewhere; resetting the password
//!      MUST invalidate every session in the same transaction so the
//!      attacker is logged out the moment the legitimate user resets.
//!
//! The four-statement transaction is the L7 grade-school answer:
//!
//! ```text
//! BEGIN;
//!   UPDATE password_resets  SET used_at = ... WHERE id = ?;
//!   UPDATE users            SET password_hash = ? WHERE id = ?;
//!   UPDATE sessions         SET revoked_at = ... WHERE user_id = ? AND revoked_at IS NULL;
//!   INSERT INTO audit_logs (...) VALUES (...);
//! COMMIT;
//! ```
//!
//! If any one of those statements fails the whole reset rolls back —
//! we will not leave a user with a new password and stale sessions, or
//! a consumed token without a password change.

use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use crate::email_verify::{new_token, token_hash};

/// Issue a fresh password-reset token for `user_id`. Invalidates any prior
/// unused tokens for that user — a previously-leaked link cannot be
/// redeemed once the user clicks "resend."
///
/// Returns the plaintext token. In production the caller emails it to
/// the user; in dev/test the calling handler echoes it back in the
/// response body so integration tests can drive the flow without SMTP.
pub async fn issue(pool: &SqlitePool, user_id: i64, ttl_secs: i64) -> Result<String, sqlx::Error> {
    let token = new_token();
    let hash = token_hash(&token);
    let expires_at = (Utc::now() + Duration::seconds(ttl_secs)).to_rfc3339();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE password_resets
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO password_resets (user_id, token_hash, expires_at)
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

/// Atomically complete a password reset:
///
///   1. Consume the token (mark `used_at`).
///   2. Replace `users.password_hash` with `new_password_hash`.
///   3. Revoke every active session for the user.
///   4. Append an audit-log row.
///
/// Returns `Some(user_id)` on success, `None` if the token is unknown,
/// expired, or already used. Callers map `None` to a 401.
///
/// The new hash is computed by the caller (so this module stays free of
/// the argon2 dependency surface) and passed in as the PHC string.
pub async fn complete(
    pool: &SqlitePool,
    token: &str,
    new_password_hash: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let hash = token_hash(token);

    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM password_resets
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
        "UPDATE password_resets
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE users
         SET password_hash = ?,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(new_password_hash)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // Kill every live session. A phished old password may still be active
    // somewhere; this is the closing of that door.
    sqlx::query(
        "UPDATE sessions
         SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO audit_logs (actor_id, action, detail) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind("user.password_reset")
        .bind(Option::<&str>::None)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Some(user_id))
}

/// Look up a user by email for the forgot-password handler. Returns
/// `Some(id)` if the email is registered, `None` otherwise. Lives here
/// (rather than inline in the handler) so it can be stubbed for tests
/// that probe the constant-time path.
pub async fn user_id_by_email(pool: &SqlitePool, email: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await
}
