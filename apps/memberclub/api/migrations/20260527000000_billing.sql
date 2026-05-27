-- Phase 8 — Stripe billing schema for MemberClub.
--
-- Rule from ADR 0008 (Stripe is the rail; our DB is the source of truth):
-- every Stripe object we care about gets mirrored into a local table.
-- The webhook is the only path that mutates these rows; user-facing
-- handlers (/v1/me/subscription, billing portal redirects, etc.) only
-- READ them. That way a Stripe outage doesn't break the read path,
-- and a "what did Stripe do last week" question can be answered with
-- a SELECT.

-- One row per (user, Stripe customer). Most users have exactly one;
-- the few who change email after signup may have several.
CREATE TABLE IF NOT EXISTS stripe_customers (
    user_id              INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stripe_customer_id   TEXT    NOT NULL UNIQUE,
    created_at           TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (user_id, stripe_customer_id)
);

CREATE INDEX IF NOT EXISTS stripe_customers_user_idx
    ON stripe_customers (user_id);

-- The user's most recent subscription state, mirrored from
-- customer.subscription.{created,updated,deleted}. We keep the raw
-- `status` Stripe returns (active, trialing, past_due, canceled, …)
-- rather than collapsing — Phase 8 lessons compare against each.
CREATE TABLE IF NOT EXISTS stripe_subscriptions (
    stripe_subscription_id TEXT    PRIMARY KEY,
    user_id                INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan_id                TEXT    NOT NULL,
    status                 TEXT    NOT NULL,
    current_period_end     TEXT,
    cancel_at_period_end   INTEGER NOT NULL DEFAULT 0 CHECK (cancel_at_period_end IN (0, 1)),
    created_at             TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at             TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS stripe_subscriptions_user_idx
    ON stripe_subscriptions (user_id);

-- One row per Stripe Invoice we've seen. We mirror amount + currency
-- as i64 cents + 3-letter code (ADR 0003: never store money as float).
CREATE TABLE IF NOT EXISTS stripe_invoices (
    stripe_invoice_id   TEXT    PRIMARY KEY,
    user_id             INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount_cents        INTEGER NOT NULL,
    currency            TEXT    NOT NULL CHECK (length(currency) = 3),
    status              TEXT    NOT NULL,
    hosted_invoice_url  TEXT,
    received_at         TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS stripe_invoices_user_idx
    ON stripe_invoices (user_id);

-- THE big-picture rule: every webhook delivery is recorded here BEFORE
-- we update any other table. The webhook handler's first SQL statement
-- is an `INSERT … ON CONFLICT DO NOTHING` keyed by stripe_event_id;
-- if the row already exists, we return 200 OK without re-processing.
-- That makes the handler idempotent against Stripe's at-least-once
-- delivery guarantee — replaying the same event a hundred times
-- updates the DB exactly once.
CREATE TABLE IF NOT EXISTS stripe_events (
    stripe_event_id  TEXT    PRIMARY KEY,
    event_type       TEXT    NOT NULL,
    received_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    processed_at     TEXT
);

CREATE INDEX IF NOT EXISTS stripe_events_type_idx
    ON stripe_events (event_type);
