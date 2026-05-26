-- Phase 6.5 — password reset tokens.
--
-- Mirrors `email_verifications` exactly. One row per outstanding (or used)
-- reset link. The plaintext token is emailed; we store only its SHA-256
-- hash. On `complete`, in a single transaction:
--   1. mark the row consumed (`used_at = NOW()`),
--   2. swap `users.password_hash` for the new argon2id PHC string,
--   3. revoke every active session for the user (kills attackers who
--      phished the old password and are riding a live session),
--   4. write an audit-log entry.
--
-- "Forgot password" issues a new row AND invalidates any prior unused rows
-- for the same user, so an old emailed link can't be replayed once the
-- user clicks "resend."

CREATE TABLE IF NOT EXISTS password_resets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  BLOB    NOT NULL UNIQUE,
    expires_at  TEXT    NOT NULL,
    used_at     TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS password_resets_user_unused_idx
    ON password_resets (user_id) WHERE used_at IS NULL;
