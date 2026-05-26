# Lesson 3.7 — Step B: Postgres in Docker

> **Concept first:** dev = test = CI parity. Run the same Postgres locally that runs in CI and in production. Docker Compose makes that boring.
> **Time:** 30 minutes.

## What we're starting

The repo root already has a `compose.yaml` defining three services we use in dev:

```yaml
services:
  db:      postgres:17-alpine                  # 127.0.0.1:5432
  redis:   redis:7.4-alpine                    # 127.0.0.1:6379
  mailhog: mailhog/mailhog:latest              # 127.0.0.1:1025 + 8025
```

All bound to `127.0.0.1` only — they're invisible from the network.

## Bring it up

```bash
docker compose up -d
docker compose ps        # all healthy
docker compose logs db --tail=20
```

Expected output: a healthy `db`, `redis`, and `mailhog` container.

## Get a `psql` shell

```bash
docker compose exec db psql -U app -d app
```

You're inside the Postgres prompt:

```
app=# SELECT version();
              version
------------------------------------
 PostgreSQL 17.5 on x86_64-pc-linux…
app=# \dt
Did not find any relations.
app=# \q
```

The colon-commands are `psql` meta-commands:

| Command | What it does |
|---|---|
| `\l`           | List databases |
| `\dt`          | List tables in this DB |
| `\d <table>`   | Describe a table |
| `\di`          | List indexes |
| `\df`          | List functions |
| `\x`           | Toggle expanded display (one column per line — useful for wide rows) |
| `\timing`      | Toggle query timing |
| `\?`           | Help |
| `\q`           | Quit |

## Connection URL

The standard format:

```
postgres://USER:PASSWORD@HOST:PORT/DBNAME?sslmode=disable
```

For our local dev stack:

```
postgres://app:app@localhost:5432/app
```

Put it in `.env`:

```bash
cp .env.example .env
# (already populated with DATABASE_URL=postgres://app:app@localhost:5432/app)
```

## sqlx-cli for migrations

We installed `sqlx-cli` in Phase 0. Usage:

```bash
export $(grep -v '^#' .env | xargs)             # load .env into the shell
sqlx database create                            # creates the DB if it doesn't exist
sqlx migrate add create_users                   # generates migrations/<timestamp>_create_users.sql
# ... edit the SQL file ...
sqlx migrate run                                # apply pending migrations
sqlx migrate info                               # show what's applied
sqlx migrate revert                             # roll back the latest (only in dev)
```

The migration files are ordinary SQL. Versioned by timestamp prefix. Committed to git.

> Migration files are *append-only* in production. To "edit" a past migration, write a new one that fixes whatever it did.

## A first migration

```bash
sqlx migrate add create_notes
```

Edit the generated file:

```sql
CREATE TABLE notes (
    id          BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    body        TEXT        NOT NULL CHECK (length(body) > 0 AND length(body) <= 4096),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX notes_created_at_idx ON notes (created_at DESC);
```

Run it:

```bash
sqlx migrate run
```

In `psql`:

```
app=# \d notes
                                          Table "public.notes"
   Column   |           Type           | Collation | Nullable |          Default
-----------+--------------------------+-----------+----------+--------------------------------
 id         | bigint                   |           | not null | generated always as identity
 body       | text                     |           | not null |
 created_at | timestamp with time zone |           | not null | now()
Indexes:
    "notes_pkey" PRIMARY KEY, btree (id)
    "notes_created_at_idx" btree (created_at DESC)
Check constraints:
    "notes_body_check" CHECK (length(body) > 0 AND length(body) <= 4096)
```

## Inspecting query plans

```sql
EXPLAIN ANALYZE
SELECT * FROM notes ORDER BY created_at DESC LIMIT 10;
```

You should see the planner use `notes_created_at_idx`.

## Shutting down

```bash
docker compose down            # stops containers; keeps volumes (data persists)
docker compose down -v         # stops AND removes volumes (data destroyed)
```

> `down -v` is *destructive*. Use it when you want a known-clean state. Never on a production host.

## Why this matters

- **Dev environments that look like prod kill a whole class of "works on my machine" bugs.** Postgres 17 in compose = Postgres 17 in CI = Postgres 17 (or higher) in prod.
- **Migrations are an audit trail.** Every schema change is in git, with the commit that introduced it.
- **The `psql` shell is your forensic flashlight.** When a production incident happens at 2 AM, you'll reach for it before anything else.

## Green-bar checkpoint

- `docker compose up -d && docker compose ps` shows three healthy containers.
- `docker compose exec db psql -U app -d app` drops you into a working shell.
- You can use `sqlx migrate add` + `sqlx migrate run` to apply a schema change.

Next: `lessons/08-stepB-sqlx-fundamentals.md`.
