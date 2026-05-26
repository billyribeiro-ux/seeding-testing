-- Phase 6 TOTP/2FA migration. Run after the init migration.
--
-- - Adds enrollment columns to `users` (secret, enabled flag, last_verified).
-- - Adds the `totp_recovery_codes` table (one row per single-use code,
--   storing only the SHA-256 hash — like sessions, we never store the
--   plaintext in case the DB leaks).

ALTER TABLE users ADD COLUMN totp_secret             TEXT;
ALTER TABLE users ADD COLUMN totp_enabled            INTEGER NOT NULL DEFAULT 0 CHECK (totp_enabled IN (0, 1));
ALTER TABLE users ADD COLUMN totp_last_verified_at   TEXT;

CREATE TABLE IF NOT EXISTS totp_recovery_codes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash   BLOB    NOT NULL UNIQUE,
    consumed_at TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS totp_recovery_codes_user_unconsumed_idx
    ON totp_recovery_codes (user_id) WHERE consumed_at IS NULL;
