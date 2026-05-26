-- MemberClub API — combined initial schema.
-- Inlines (and adapts) the schemas from auth-demo (users, sessions, audit_logs)
-- and adds a memberclub-shaped `notes` table with multi-tenancy + tier gating.

CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    email           TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash   TEXT    NOT NULL,
    role            TEXT    NOT NULL DEFAULT 'member'
                            CHECK (role IN ('member', 'moderator', 'admin')),
    tier            TEXT    NOT NULL DEFAULT 'free'
                            CHECK (tier IN ('free', 'pro', 'elite')),
    org_id          INTEGER NOT NULL DEFAULT 1,
    email_verified  INTEGER NOT NULL DEFAULT 0 CHECK (email_verified IN (0, 1)),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token_hash  BLOB    NOT NULL UNIQUE,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issued_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at  TEXT    NOT NULL,
    revoked_at  TEXT
);
CREATE INDEX IF NOT EXISTS sessions_user_id_active_idx
    ON sessions (user_id) WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS notes (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id        INTEGER NOT NULL,
    body          TEXT    NOT NULL CHECK (length(body) >= 1 AND length(body) <= 4096),
    published_at  TEXT,
    min_tier      TEXT    NOT NULL DEFAULT 'free'
                          CHECK (min_tier IN ('free', 'pro', 'elite')),
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS notes_owner_idx ON notes (owner_id);
CREATE INDEX IF NOT EXISTS notes_org_idx   ON notes (org_id);

CREATE TABLE IF NOT EXISTS audit_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_id    INTEGER REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT    NOT NULL,
    detail      TEXT,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
