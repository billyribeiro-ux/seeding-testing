-- Phase 6.4 — email verification.
--
-- One row per outstanding (or used) verification link. The plaintext token
-- is sent to the user's inbox; we store only its SHA-256 hash, exactly as
-- we do for session cookies. On confirm, we atomically:
--   1. mark the row consumed (`used_at = NOW()`),
--   2. flip `users.is_email_verified = 1`,
--   3. write an audit-log entry.
--
-- "Resend" issues a new row AND invalidates any prior unused rows for the
-- same user, so a previously-leaked link can no longer be replayed.

CREATE TABLE IF NOT EXISTS email_verifications (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  BLOB    NOT NULL UNIQUE,
    expires_at  TEXT    NOT NULL,
    used_at     TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS email_verifications_user_unused_idx
    ON email_verifications (user_id) WHERE used_at IS NULL;
