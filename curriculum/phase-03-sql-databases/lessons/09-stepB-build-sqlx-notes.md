# Lesson 3.9 — Step B: Build `sqlx-notes` Step by Step

> **The graduate.** Re-implement the *same* notes domain in Rust + sqlx. Read this side-by-side with Lesson 3.6 to feel the trade.
> **Time:** 60 minutes.

## What we're building

`projects/02c-sqlx-notes` — a Rust library crate with:

- A migration that creates the `notes` table with a CHECK constraint.
- Functions `list`, `add`, `delete`, `get` over a `SqlitePool`.
- 12 integration tests against an in-memory SQLite database.
- A `NotesError` enum with `thiserror`.

No binary, no HTTP — that comes in Phase 4. This phase is about the *data layer*.

## Step 0 — Browse the project

```bash
cd projects/02c-sqlx-notes
ls -R
```

```
Cargo.toml
migrations/
  20260526120000_create_notes.sql
src/
  lib.rs
tests/
  notes.rs
```

## Step 1 — The migration

`migrations/20260526120000_create_notes.sql`:

```sql
CREATE TABLE IF NOT EXISTS notes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    body        TEXT    NOT NULL CHECK (length(body) > 0 AND length(body) <= 4096),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS notes_created_at_idx ON notes (created_at DESC);
```

Three things to notice:

- **`CHECK` constraint at the DB layer** mirrors the validation in `add()`. *Both* enforce the rule — if someone sneaks a raw SQL insert past the library, the DB still says no. Belt and braces.
- **`IF NOT EXISTS`** is harmless on the first run, idempotent on re-runs.
- **`AUTOINCREMENT`** — without it, SQLite re-uses deleted IDs. With it, IDs never go backwards. Tiny correctness win.

The filename's leading timestamp is how sqlx orders migrations. Pick a real timestamp (`date +%Y%m%d%H%M%S`).

## Step 2 — The error type

```rust
#[derive(Debug, Error)]
pub enum NotesError {
    #[error("body cannot be empty")]
    Empty,
    #[error("body cannot exceed 4096 characters (got {0})")]
    TooLong(usize),
    #[error("note {0} not found")]
    NotFound(i64),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}
```

- **One variant per domain failure mode.** Callers can `match` and react.
- **`#[from] sqlx::Error`** auto-converts via the `?` operator. We never have to `.map_err(...)`.
- **`#[error(transparent)]`** makes the `Db` variant render as its inner error message — the wrapping is invisible to humans.

## Step 3 — The query functions

```rust
pub async fn list(pool: &SqlitePool) -> NotesResult<Vec<Note>> {
    let rows = sqlx::query_as::<_, Note>(
        "SELECT id, body, created_at FROM notes ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

We use `query_as::<_, Note>` (runtime) rather than `query_as!(Note, …)` (macro) because the macro needs a live DB at compile time, and we want the project to build without one. In production code against a Postgres dev DB you'd use the macro and commit `.sqlx/`.

```rust
pub async fn add(pool: &SqlitePool, body: &str) -> NotesResult<Note> {
    let trimmed = body.trim();
    if trimmed.is_empty() { return Err(NotesError::Empty); }
    if trimmed.chars().count() > 4096 { return Err(NotesError::TooLong(trimmed.chars().count())); }

    let row = sqlx::query_as::<_, Note>(
        "INSERT INTO notes (body) VALUES (?) RETURNING id, body, created_at",
    )
    .bind(trimmed)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
```

Three patterns to remember:

1. **Validate *before* the DB call.** Cheap to fail fast; clearer error messages than the DB's `CHECK` constraint.
2. **`RETURNING *`** — one round-trip, no follow-up `SELECT` to grab the auto-assigned ID.
3. **`.bind(...)` per placeholder.** sqlx never builds SQL by string concatenation, so SQL injection is impossible by construction.

```rust
pub async fn delete(pool: &SqlitePool, id: i64) -> NotesResult<()> {
    let result = sqlx::query("DELETE FROM notes WHERE id = ?").bind(id).execute(pool).await?;
    if result.rows_affected() == 0 { return Err(NotesError::NotFound(id)); }
    Ok(())
}
```

- **`execute`** for non-`SELECT`. Returns the row count.
- **0 rows ⇒ not found, not silent.** We turn the no-op into an error so callers know.

## Step 4 — Tests

`tests/notes.rs` opens with:

```rust
async fn fresh_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1) // ":memory:" is per-connection
        .connect("sqlite::memory:")
        .await
        .expect("connect");
    migrate(&pool).await.expect("apply migrations");
    pool
}
```

Two non-obvious moves:

1. **`max_connections(1)`** — `sqlite::memory:` creates a *new* database per connection. With multiple connections, half your queries would see an empty DB. One connection = one shared DB inside the test.
2. **`migrate(&pool).await`** — calls our library's `migrate` function which runs `sqlx::migrate!("./migrations")` internally. The schema is bootstrapped exactly as it is in production.

Twelve tests:

```rust
#[tokio::test] async fn list_is_empty_initially()              { ... }
#[tokio::test] async fn add_persists_a_note()                  { ... }
#[tokio::test] async fn list_is_newest_first()                 { ... }
#[tokio::test] async fn add_trims_whitespace()                 { ... }
#[tokio::test] async fn add_rejects_empty()                    { ... }
#[tokio::test] async fn add_rejects_too_long()                 { ... }
#[tokio::test] async fn delete_removes_a_row()                 { ... }
#[tokio::test] async fn delete_unknown_id_returns_not_found()  { ... }
#[tokio::test] async fn get_returns_the_row()                  { ... }
#[tokio::test] async fn get_unknown_id_is_not_found()          { ... }
#[tokio::test] async fn created_at_is_parseable_iso8601()      { ... }
#[tokio::test] async fn check_constraint_blocks_empty_body_at_db_layer() { ... }
```

The last one is the *belt-and-braces* test — it issues a raw SQL `INSERT` with an empty body and asserts the DB itself rejects it. If the library's validation is removed by accident, the DB still defends the invariant.

## Step 5 — Run it

```bash
cargo test -p sqlx-notes                  # 12/12 green
cargo clippy -p sqlx-notes -- -D warnings # clean
cargo fmt -p sqlx-notes -- --check        # clean
make verify                               # whole workspace green
```

## Side-by-side: list-newest-first

**Drizzle (Step A):**

```ts
db.select().from(notes).orderBy(desc(notes.id)).all()
```

**sqlx (Step B):**

```rust
sqlx::query_as::<_, Note>("SELECT id, body, created_at FROM notes ORDER BY id DESC")
    .fetch_all(pool).await
```

Two different aesthetics, same outcome. Drizzle wins on terseness; sqlx wins on directness — when `EXPLAIN ANALYZE` says "use this hint," the sqlx version can take it without arguing with a library.

## Why this matters

- **The data layer is the part you tune at scale.** Hand-written SQL gives you every Postgres feature. ORMs invariably lag.
- **DB-layer constraints are a safety net.** Even when application logic is wrong (bug, race, refactor), the database refuses to corrupt itself.
- **Integration tests against the real DB engine** — even SQLite — catch class of bugs no mock would ever surface. The pattern scales: Phase 4 uses `testcontainers` to do the same against a real Postgres.

## Green-bar checkpoint

- `cargo test -p sqlx-notes` shows `12 passed`.
- You can explain why a `max_connections(1)` SQLite pool is required for `:memory:`.
- You can read the Drizzle and sqlx versions of `listNotes` and explain the trade.

Next: `lessons/10-drizzle-vs-sqlx.md`.
