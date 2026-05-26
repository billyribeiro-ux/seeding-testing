# Lesson 3.1 — Relational Fundamentals

> **Concept first:** a relational database is a set of named *tables*, each with named *columns* of typed *values*, related to each other by *keys*. That sentence is the whole game.
> **Time:** 30 minutes.

## The vocabulary

| Word | Plain English |
|---|---|
| Table | A spreadsheet of one kind of thing (users, orders, notes). |
| Row | One instance of that thing (one user). |
| Column | A typed attribute (email, age, is_admin). |
| Primary key (PK) | A column (or set) whose value uniquely identifies each row. Usually `id`. |
| Foreign key (FK) | A column whose value must match a PK in another table. The "this belongs to that" arrow. |
| Index | A separate data structure the DB consults to find rows fast. |
| Schema | The set of table + column definitions. |
| Constraint | A rule the DB enforces (NOT NULL, UNIQUE, CHECK). |
| Query | A SQL statement (SELECT, INSERT, UPDATE, DELETE). |
| Transaction | A bundle of statements that succeed or fail as a unit. |

If you internalize these eleven words, you've internalized most of databases.

## Why "relational"?

Imagine you store everything as one giant table:

```
+----+--------+--------------+----------+--------+-----------+
| id | name   | email        | order_id | amount | shipped   |
+----+--------+--------------+----------+--------+-----------+
| 1  | alice  | a@b.com      | 101      | 1200   | true      |
| 2  | alice  | a@b.com      | 102      |  800   | false     |
| 3  | bob    | b@c.com      | 103      | 1500   | true      |
+----+--------+--------------+----------+--------+-----------+
```

Alice's name and email are duplicated. Update one and you risk forgetting the other. **That's** the bug normalization prevents.

The relational fix:

```
users:                              orders:
+----+--------+-----------+         +-----+---------+--------+---------+
| id | name   | email     |         | id  | user_id | amount | shipped |
+----+--------+-----------+         +-----+---------+--------+---------+
| 1  | alice  | a@b.com   |         | 101 | 1       | 1200   | true    |
| 2  | bob    | b@c.com   |         | 102 | 1       |  800   | false   |
+----+--------+-----------+         | 103 | 2       | 1500   | true    |
                                    +-----+---------+--------+---------+
```

`orders.user_id` is a **foreign key** to `users.id`. Each fact lives in exactly one place. To see Alice's orders, you `JOIN`.

## Types — the most common ones

| SQL type | Postgres | Rust (via sqlx) | Use for |
|---|---|---|---|
| `INTEGER` | `INT4` / `INT2` | `i32` / `i16` | Small ints |
| `BIGINT` | `INT8` | `i64` | IDs, timestamps as ms, money cents |
| `TEXT` | `TEXT` | `String` | Strings of any size |
| `VARCHAR(n)` | `VARCHAR(n)` | `String` | Bounded strings (rarely worth it in Postgres) |
| `BOOLEAN` | `BOOL` | `bool` | true/false |
| `TIMESTAMPTZ` | `TIMESTAMPTZ` | `chrono::DateTime<Utc>` | A point in time, with timezone (always use this over `TIMESTAMP`) |
| `UUID` | `UUID` | `uuid::Uuid` | Externally-issued opaque IDs |
| `JSONB` | `JSONB` | `serde_json::Value` or your type via `sqlx::types::Json<T>` | Schemaless extras |
| `NUMERIC(p,s)` | `NUMERIC` | `rust_decimal::Decimal` | Exact decimal math (taxes, percentages) |
| `BIGINT` | `INT8` | `i64` | **Money in cents** (see Phase 8) |

Two strong opinions:

- **Always use `TIMESTAMPTZ`, never `TIMESTAMP`.** "Timestamp without timezone" is a footgun.
- **Never use `FLOAT`/`DOUBLE` for money.** We obsess over this in Phase 8.

## NULL is not zero, not empty, not false

`NULL` means "no value." It is not equal to anything, including itself:

```sql
SELECT NULL = NULL;     -- result: NULL (not true!)
SELECT NULL IS NULL;    -- result: true
```

Every column you declare without an explicit `NOT NULL` constraint can be `NULL`. Most fields *shouldn't be*. The default discipline:

> **Default `NOT NULL`. Document every `NULL`-able column with a comment explaining why.**

## Primary keys — three flavours

1. **Serial / `BIGSERIAL` / `BIGINT GENERATED ALWAYS AS IDENTITY`** — auto-incrementing integer. Compact, fast, leaks volume (your competitor can guess how many users you have).
2. **UUID v4** — random 128-bit. No leak, no ordering, slightly bigger indexes.
3. **UUID v7** — time-ordered 128-bit. Best of both worlds; this is the 2026 default for new schemas.

In MemberClub we use **`BIGINT IDENTITY` for internal IDs, `UUID v7` for any ID exposed to users** (so URLs don't reveal volume).

## Foreign keys — what they enforce

```sql
CREATE TABLE notes (
    id      BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body    TEXT   NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

The `REFERENCES users(id)` says: every `notes.user_id` must match an existing `users.id`. Try to insert a note with `user_id = 999` when no such user exists — the DB rejects it.

`ON DELETE CASCADE` says: if a user is deleted, their notes go too. Other options: `RESTRICT` (block the deletion), `SET NULL` (orphan the notes), `SET DEFAULT`.

## Why this matters

- **Bad schema is technical debt that compounds.** Renaming a column in a 50M-row table at 3 AM under load is no fun.
- **Constraints are guardrails on the *database*** — they protect you even when the application has a bug.
- **Types are documentation.** A `BIGINT amount_cents NOT NULL` column tells every reader exactly what it holds.

## Green-bar checkpoint

- You can explain `NULL` semantics: why `NULL = NULL` is `NULL` (not true).
- You can sketch a `users` + `notes` schema with proper PKs and FKs.
- You can pick between `BIGSERIAL`, `UUID v4`, and `UUID v7` based on requirements.

Next: `lessons/02-schema-design.md`.
