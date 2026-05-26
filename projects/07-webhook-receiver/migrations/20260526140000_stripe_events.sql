-- One row per delivered webhook event. The UNIQUE constraint on stripe_event_id
-- is what makes the handler idempotent — duplicate deliveries are no-ops.

CREATE TABLE IF NOT EXISTS stripe_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    stripe_event_id     TEXT    NOT NULL UNIQUE,
    event_type          TEXT    NOT NULL,
    created_at_stripe   INTEGER NOT NULL,                                                -- unix seconds
    received_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    processed_at        TEXT,
    payload             TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS stripe_events_event_type_idx ON stripe_events (event_type);
