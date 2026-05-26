# Lesson 3.2 — Schema Design

> **Concept first:** a good schema makes invalid states impossible. Three normal forms are enough for 95% of real systems.
> **Time:** 30 minutes.

## The three normal forms (1NF, 2NF, 3NF) — in plain English

1. **1NF:** each cell holds one value. No comma-separated lists in a column; no JSON-of-arrays where you meant a join table.
2. **2NF:** every non-key column depends on the *entire* primary key. (Only matters when the PK is composite.)
3. **3NF:** non-key columns depend on the key *only* — not on other non-key columns.

These rules are about *eliminating duplication of facts*. If you can change a fact in two places, you can drift them. Eventually you will.

The rest of normalization (4NF, BCNF, 5NF) is academia. Ship 3NF and you're fine.

## When to denormalize

You deliberately violate 3NF when:

- A read is performance-critical and you measured the join cost. (Have the measurement; never "I think it'll be slow.")
- The data is *append-only* — denormalized snapshots in an `audit_log` are normal. The fact at write time *is* the truth forever.
- You have a materialized view; the DB keeps it in sync for you.

Otherwise, normalize. Storage is cheap; *consistency* is expensive.

## Naming conventions (pick one and stick)

The conventions we use in this curriculum (and in 90% of modern Postgres codebases):

- **Tables: plural, `snake_case`.** `users`, `subscriptions`, `audit_logs`.
- **Columns: singular, `snake_case`.** `email`, `created_at`, `user_id`.
- **Primary key: `id`.** Always.
- **Foreign keys: `<other_table_singular>_id`.** `user_id` (refers to `users.id`).
- **Booleans: positive phrasing.** `is_admin`, not `not_admin`.
- **Timestamps end in `_at`.** `created_at`, `updated_at`, `deleted_at`.
- **Counts end in `_count`.** `failed_login_count`.
- **Money columns end in `_cents`.** `amount_cents`. Always `BIGINT`. Never floats.

These conventions are mechanical, so they're frictionless to apply. The savings compound.

## A real-world starter schema

The MemberClub initial schema, lightly annotated:

```sql
CREATE TABLE users (
    id              BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    public_id       UUID        NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    email           CITEXT      NOT NULL UNIQUE,
    password_hash   TEXT        NOT NULL,
    is_email_verified BOOLEAN   NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX users_created_at_idx ON users (created_at DESC);

CREATE TABLE sessions (
    id            BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    token_hash    BYTEA       NOT NULL UNIQUE,
    user_id       BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent    TEXT,
    ip_inet       INET,
    issued_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at    TIMESTAMPTZ NOT NULL,
    revoked_at    TIMESTAMPTZ
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id) WHERE revoked_at IS NULL;

CREATE TABLE audit_logs (
    id          BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_id    BIGINT      REFERENCES users(id),                -- NULL = system
    action      TEXT        NOT NULL,                            -- e.g. "user.created"
    target      JSONB       NOT NULL,                            -- the resource snapshot
    metadata    JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX audit_logs_actor_created_idx ON audit_logs (actor_id, created_at DESC);
```

Five things to notice:

1. **`public_id UUID v4-ish`** alongside the integer `id`. The integer is fast for joins; the UUID is what we put in URLs.
2. **`CITEXT`** — case-insensitive text. `alice@b.com` and `ALICE@b.com` are equal at the DB level. Massive bug-prevention for emails.
3. **`NOT NULL` everywhere.** Only `revoked_at` and `user_agent`/`ip_inet` are nullable, and the `NULL` carries clear meaning.
4. **Partial index `WHERE revoked_at IS NULL`.** Lookups for "active sessions" hit only the non-revoked subset.
5. **Audit log writes are *append-only* and deliberately denormalized** — `target` is a JSONB snapshot of the resource at write time, so future schema changes don't rewrite history.

## `created_at` / `updated_at` — the universal pair

Every business table gets these. We populate `created_at` once on insert and bump `updated_at` on every change. A trigger does the latter so application code can't forget:

```sql
CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS trigger AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_touch_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
```

Set up once; works forever.

## Soft delete vs hard delete

| | Hard delete | Soft delete (`deleted_at IS NOT NULL`) |
|---|---|---|
| Pros | Simple, fast, GDPR-clean | Can be undone, audit trail |
| Cons | Truly gone | All queries must filter `WHERE deleted_at IS NULL` |
| When to use | User accounts (GDPR), session rows | Content, orders, anything you might need to restore |

Don't do *both*. Pick once per table.

## Why this matters

- **Schemas are forever.** A migration that runs at scale is irreversible-ish; rolling back a million-row migration is multiple hours of nail-biting.
- **Constraints prevent bugs the app can't.** A `UNIQUE (email)` constraint protects against a race condition the application could only solve with a distributed lock.
- **Naming conventions compound.** Every new developer instantly knows what `_id`, `_at`, `_cents` mean.

## Green-bar checkpoint

- You can sketch a `users` / `posts` / `comments` schema following the conventions above.
- You can articulate when to use a partial index vs a full index.
- You can pick soft-delete vs hard-delete for three different tables and justify each.

Next: `lessons/03-sql-by-doing.md`.
