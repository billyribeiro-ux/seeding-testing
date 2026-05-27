-- Phase 6 stretch E6.8 — OAuth (Google) and magic-link login.
--
-- 1. Extend `users` with a `google_sub` column. The Google subject ("sub"
--    claim from the ID token / userinfo endpoint) is the durable, opaque
--    user identifier — NOT the email, which can change. We add it as a
--    nullable UNIQUE column so password-only users keep working (NULL is
--    not constrained by UNIQUE in SQLite) and an OAuth-linked user is
--    keyed by the sub the way an enterprise SSO integration would do it.
--
-- 2. Add a transient `oauth_states` table that holds the (state CSRF
--    nonce + PKCE code_verifier + optional return_to redirect) tuple
--    between the /start and /callback halves of the authorization-code
--    flow. Rows have an explicit `expires_at` so a cron-like cleanup can
--    sweep stale entries — the /callback handler additionally consumes
--    the row by primary key the moment it's used, so a state cannot be
--    redeemed twice.
--
-- 3. Add a `magic_links` table for passwordless login. Same shape as
--    `email_verifications` and `password_resets` — hash the token,
--    one-shot via `used_at`, TTL via `expires_at`, and re-requesting
--    invalidates any prior unused rows for that user.

ALTER TABLE users ADD COLUMN google_sub TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS users_google_sub_unique_idx
    ON users (google_sub) WHERE google_sub IS NOT NULL;

CREATE TABLE IF NOT EXISTS oauth_states (
    state          TEXT PRIMARY KEY,
    code_verifier  TEXT NOT NULL,
    redirect_to    TEXT,
    expires_at     TEXT NOT NULL,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS magic_links (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  BLOB    NOT NULL UNIQUE,
    expires_at  TEXT    NOT NULL,
    used_at     TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS magic_links_user_unused_idx
    ON magic_links (user_id) WHERE used_at IS NULL;
