-- The "business" table — pretend this is a transfers ledger, an orders table,
-- a webhook receiver, anything that has side-effects to publish on write.

CREATE TABLE IF NOT EXISTS transfers (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    from_account TEXT    NOT NULL,
    to_account   TEXT    NOT NULL,
    amount_cents INTEGER NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2_100_000_000_00),
    created_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- The outbox — every pending side effect.
--
-- Three rules baked into the schema:
--   1. status is one of pending / processing / done / failed (CHECK enforces it).
--   2. next_attempt_at carries the exponential backoff schedule.
--   3. attempts counts retries so we can cap.

CREATE TABLE IF NOT EXISTS outbox (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    kind            TEXT    NOT NULL,
    payload         TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','processing','done','failed')),
    attempts        INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error      TEXT,
    next_attempt_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at    TEXT
);

-- Hot path: workers poll for the next pending row.
CREATE INDEX IF NOT EXISTS outbox_pending_idx
    ON outbox (next_attempt_at) WHERE status = 'pending';

-- Reporting: failed rows by kind.
CREATE INDEX IF NOT EXISTS outbox_failed_by_kind_idx
    ON outbox (kind, completed_at) WHERE status = 'failed';
