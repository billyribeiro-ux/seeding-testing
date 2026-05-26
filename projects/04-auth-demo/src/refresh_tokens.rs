//! Phase 6 stretch — refresh-token rotation with reuse detection (E6.7).
//!
//! Every refresh JWT we issue lives as a row in `refresh_tokens`,
//! keyed by its `jti` claim. The lifecycle is **rotation in a family**:
//!
//! * `/auth/login` calls [`record_root`] — mints a brand-new family and
//!   stores the root refresh row with `parent_jti = NULL`.
//! * `/auth/refresh` calls [`rotate`] — atomically claims the presented
//!   row (`UPDATE … RETURNING`) and either inserts a child token in
//!   the same family, OR — if the row was already marked `used_at` —
//!   revokes the entire family and reports `ReuseDetected`.
//!
//! That last branch is the whole point of this module. A stolen
//! refresh token is fine for the attacker exactly until *either* side
//! tries to refresh again: the second call sees the first one's
//! `used_at`, knows something is wrong, and burns the whole family. The
//! attacker is kicked out; the legitimate user is forced to re-login.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// Outcome of a single rotation attempt.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Legitimate refresh — the new child row has been inserted in the
    /// family. Caller returns the new (access, refresh) pair to the
    /// client.
    Ok { user_id: i64, family_id: String },
    /// The presented token was already marked `used_at`. We just
    /// revoked the whole family and wrote an audit-log row. Caller
    /// returns 401 to whoever was holding it.
    ReuseDetected,
    /// The presented jti isn't in our table, OR the family was already
    /// revoked, OR the row has expired. Caller returns 401. We
    /// deliberately don't tell the wire which — they all look the same
    /// to the client.
    Unknown,
}

/// Record a brand-new root refresh row. Called by `/auth/login` after
/// minting the very first refresh JWT for the session.
pub async fn record_root(
    pool: &SqlitePool,
    jti: &str,
    user_id: i64,
    family_id: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, family_id, parent_jti, expires_at)
         VALUES (?, ?, ?, NULL, ?)",
    )
    .bind(jti)
    .bind(user_id)
    .bind(family_id)
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically rotate a refresh token.
///
/// `presented_jti` is the jti from the JWT the client just sent;
/// `new_jti` and `new_exp` are the jti/exp of the *replacement* JWT
/// we've already minted (but not yet handed out). On any non-`Ok`
/// outcome the new JWT bytes are simply discarded; nothing has been
/// inserted for them.
///
/// The single SQL statement that does the atomic claim is:
///
/// ```sql
/// UPDATE refresh_tokens
/// SET used_at = NOW()
/// WHERE jti = ? AND used_at IS NULL AND revoked_at IS NULL AND expires_at > NOW()
/// RETURNING user_id, family_id;
/// ```
///
/// If two clients refresh the same jti simultaneously, SQLite serializes
/// the writes — exactly one UPDATE returns a row; the other returns
/// nothing and trips the reuse path. The same property holds in
/// Postgres with `FOR UPDATE SKIP LOCKED` or simply by virtue of the
/// `UPDATE … RETURNING` being atomic per-row.
pub async fn rotate(
    pool: &SqlitePool,
    presented_jti: &str,
    new_jti: &str,
    new_exp: DateTime<Utc>,
) -> Result<Outcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let claimed: Option<(i64, String)> = sqlx::query_as(
        "UPDATE refresh_tokens
         SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE jti = ?
           AND used_at IS NULL
           AND revoked_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING user_id, family_id",
    )
    .bind(presented_jti)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((user_id, family_id)) = claimed {
        sqlx::query(
            "INSERT INTO refresh_tokens (jti, user_id, family_id, parent_jti, expires_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(new_jti)
        .bind(user_id)
        .bind(&family_id)
        .bind(presented_jti)
        .bind(new_exp.to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Outcome::Ok { user_id, family_id });
    }

    // Didn't claim. Figure out why so we can distinguish reuse from
    // simply-unknown.
    let row: Option<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT user_id, family_id, used_at, revoked_at
         FROM refresh_tokens WHERE jti = ?",
    )
    .bind(presented_jti)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((user_id, family_id, used_at, revoked_at)) = row else {
        tx.rollback().await?;
        return Ok(Outcome::Unknown);
    };

    if used_at.is_some() && revoked_at.is_none() {
        // Reuse detected — the row was already consumed by an earlier
        // call. Kill the whole family.
        let n = sqlx::query(
            "UPDATE refresh_tokens
             SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE family_id = ? AND revoked_at IS NULL",
        )
        .bind(&family_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        sqlx::query("INSERT INTO audit_logs (actor_id, action, detail) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind("refresh_token.reuse_detected")
            .bind(format!("family_id={family_id} revoked={n}"))
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        return Ok(Outcome::ReuseDetected);
    }

    // Already revoked, expired, or some other terminal state.
    tx.rollback().await?;
    Ok(Outcome::Unknown)
}

/// Revoke every active refresh token for a user across all families.
/// Called when the user's password is reset or every session is
/// otherwise force-logged-out. Returns the number of rows affected
/// (useful for audit-log detail).
pub async fn revoke_all_for_user(pool: &SqlitePool, user_id: i64) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE refresh_tokens
         SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}
