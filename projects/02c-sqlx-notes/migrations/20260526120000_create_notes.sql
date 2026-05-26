-- The same `notes` domain as Step A, expressed in pure SQL.
-- Read this alongside projects/02b-sqlite-notes-svelte/src/lib/server/db/schema.ts.

CREATE TABLE IF NOT EXISTS notes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    body        TEXT    NOT NULL CHECK (length(body) > 0 AND length(body) <= 4096),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS notes_created_at_idx ON notes (created_at DESC);
