//! Phase 6.6 — TOTP (Time-based One-Time Password) 2FA.
//!
//! The standard RFC 6238 flow:
//!   1. Server generates a 20-byte random secret and stores it under
//!      `users.totp_secret`.
//!   2. Server returns a provisioning URI; the client renders it as a QR
//!      code; the user scans it with their authenticator app.
//!   3. User types the 6-digit code; the server verifies; on success,
//!      `users.totp_enabled` flips to 1.
//!   4. From now on, /auth/login requires the second factor.
//!
//! Eight recovery codes are issued at enrollment so a lost-phone user can
//! still get in. Each one is consumed on use (single-shot).

use rand::TryRngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use totp_rs::{Algorithm, Secret, TOTP};

const ISSUER: &str = "auth-demo";
const TOTP_DIGITS: usize = 6;
const TOTP_SKEW: u8 = 1;
const TOTP_STEP_SECS: u64 = 30;
const SECRET_LEN: usize = 20;
const RECOVERY_CODES: usize = 8;

#[derive(Debug)]
pub struct EnrollmentArtifacts {
    /// Base-32 encoded for display alongside the QR code.
    pub secret_b32: String,
    /// `otpauth://totp/...` — render this as a QR code.
    pub provisioning_uri: String,
    /// Eight one-time recovery codes; shown once, then hashed in the DB.
    pub recovery_codes: Vec<String>,
}

/// Generate a fresh secret + 8 recovery codes for a user, persisting them
/// (secret on `users`, hashed codes in `totp_recovery_codes`). Does NOT
/// flip `totp_enabled` — that happens in `confirm_enrollment` after the
/// user proves they scanned the QR by typing a valid code.
pub async fn begin_enrollment(
    pool: &SqlitePool,
    user_id: i64,
    user_email: &str,
) -> Result<EnrollmentArtifacts, sqlx::Error> {
    let mut secret_bytes = [0u8; SECRET_LEN];
    rand::rngs::OsRng
        .try_fill_bytes(&mut secret_bytes)
        .expect("OS RNG must work");
    let secret = Secret::Raw(secret_bytes.to_vec());

    let totp = TOTP::new(
        Algorithm::SHA1,
        TOTP_DIGITS,
        TOTP_SKEW,
        TOTP_STEP_SECS,
        secret.to_bytes().expect("secret -> bytes"),
        Some(ISSUER.into()),
        user_email.to_string(),
    )
    .expect("TOTP construction");

    let secret_b32 = secret.to_encoded().to_string();
    let provisioning_uri = totp.get_url();

    let codes: Vec<String> = (0..RECOVERY_CODES).map(|_| new_recovery_code()).collect();

    // Persist atomically: write the secret on the user, write hashed
    // recovery codes, AND clear any previously-unconsumed codes (so a
    // re-enrollment invalidates the old set).
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE users SET totp_secret = ?, totp_enabled = 0 WHERE id = ?")
        .bind(&secret_b32)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for code in &codes {
        sqlx::query("INSERT INTO totp_recovery_codes (user_id, code_hash) VALUES (?, ?)")
            .bind(user_id)
            .bind(hash_code(code).to_vec())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(EnrollmentArtifacts {
        secret_b32,
        provisioning_uri,
        recovery_codes: codes,
    })
}

/// Verify the typed 6-digit code against the stored secret. On success,
/// flips `totp_enabled = 1` if this is a *confirmation* (called for a
/// not-yet-enabled user) and records `totp_last_verified_at`.
pub async fn verify_and_maybe_enable(
    pool: &SqlitePool,
    user_id: i64,
    code: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(Option<String>, i64)> =
        sqlx::query_as("SELECT totp_secret, totp_enabled FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    let Some((Some(secret_b32), enabled)) = row else {
        return Ok(false);
    };

    if !verify_code(&secret_b32, code) {
        return Ok(false);
    }

    if enabled == 0 {
        sqlx::query(
            "UPDATE users SET totp_enabled = 1,
                              totp_last_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE users SET totp_last_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
    }
    Ok(true)
}

/// Single-shot redemption of a recovery code. Returns `true` if the code
/// matched an unconsumed row; the row is atomically marked consumed.
pub async fn consume_recovery_code(
    pool: &SqlitePool,
    user_id: i64,
    code: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE totp_recovery_codes
         SET consumed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ? AND code_hash = ? AND consumed_at IS NULL",
    )
    .bind(user_id)
    .bind(hash_code(code).to_vec())
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(false);
    }
    // Mark the user as freshly verified — recovery is a successful 2FA.
    sqlx::query(
        "UPDATE users SET totp_last_verified_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(true)
}

/// Disable TOTP for a user. Requires the caller to have *just* verified a
/// fresh code or recovery code; this function does NOT re-verify.
pub async fn disable(pool: &SqlitePool, user_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE users SET totp_enabled = 0, totp_secret = NULL,
                          totp_last_verified_at = NULL
         WHERE id = ?",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM totp_recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn verify_code(secret_b32: &str, code: &str) -> bool {
    let Ok(secret_bytes) = Secret::Encoded(secret_b32.to_string()).to_bytes() else {
        return false;
    };
    let Ok(totp) = TOTP::new(
        Algorithm::SHA1,
        TOTP_DIGITS,
        TOTP_SKEW,
        TOTP_STEP_SECS,
        secret_bytes,
        Some(ISSUER.into()),
        "ignored".to_string(),
    ) else {
        return false;
    };
    totp.check_current(code).unwrap_or(false)
}

/// Generate a recovery code: 4 hex segments separated by dashes
/// (`a1b2-c3d4-e5f6-7890`). 16 hex chars = 64 bits of entropy — enough
/// to make brute-force online attacks impractical.
fn new_recovery_code() -> String {
    let mut buf = [0u8; 8];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    let s = hex::encode(buf);
    format!("{}-{}-{}-{}", &s[0..4], &s[4..8], &s[8..12], &s[12..16])
}

/// Hash a recovery code (we never store plaintext).
fn hash_code(code: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    h.finalize().into()
}

// ---------------------------------------------------------------------------
// Test helper: derive a *current* 6-digit code from a stored secret.
// Production code never calls this — it's only used by integration tests
// to simulate what the authenticator app would generate right now.
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub fn current_code_for_secret(secret_b32: &str) -> Option<String> {
    let secret_bytes = Secret::Encoded(secret_b32.to_string()).to_bytes().ok()?;
    let totp = TOTP::new(
        Algorithm::SHA1,
        TOTP_DIGITS,
        TOTP_SKEW,
        TOTP_STEP_SECS,
        secret_bytes,
        Some(ISSUER.into()),
        "ignored".to_string(),
    )
    .ok()?;
    totp.generate_current().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_generates_then_verifies() {
        let mut buf = [0u8; SECRET_LEN];
        rand::rngs::OsRng.try_fill_bytes(&mut buf).unwrap();
        let secret_b32 = Secret::Raw(buf.to_vec()).to_encoded().to_string();

        let code = current_code_for_secret(&secret_b32).unwrap();
        assert!(
            verify_code(&secret_b32, &code),
            "expected current code to verify"
        );
        assert!(
            !verify_code(&secret_b32, "000000"),
            "the literal 000000 must not verify (overwhelmingly likely)"
        );
    }

    #[test]
    fn recovery_code_is_hyphenated_hex() {
        let c = new_recovery_code();
        assert_eq!(c.len(), 19);
        assert!(
            c.chars().all(|ch| ch == '-' || ch.is_ascii_hexdigit()),
            "got {c}"
        );
        assert_eq!(c.matches('-').count(), 3);
    }
}
