# projects/02c-sqlx-notes

Phase 3 — Step B. The *same* notes domain as Step A, re-implemented in Rust with **sqlx + SQLite**. Read this alongside Step A; that's the whole point.

## What it teaches

- `sqlx` API: `Pool`, `query`, `query_as`, `fetch_all` / `fetch_one` / `fetch_optional`, `execute`.
- Hand-written SQL with placeholders (`?` for SQLite, `$1` for Postgres).
- `sqlx::migrate!("./migrations")` for schema migrations from versioned SQL files.
- Typed errors with `thiserror`, including a `#[from] sqlx::Error` blanket.
- Integration tests against an in-memory SQLite — hermetic, no Docker required.
- DB-level constraints (`CHECK (length(body) > 0 AND length(body) <= 4096)`) — the constraint enforces the same rule the library does, *belt and braces*.

## Run the tests

```bash
cargo test -p sqlx-notes
cargo clippy -p sqlx-notes -- -D warnings
```

Or via the workspace gate:

```bash
make verify
```

## File map

| File | Purpose |
|---|---|
| `migrations/20260526120000_create_notes.sql` | The schema migration |
| `src/lib.rs` | `list`, `add`, `delete`, `get`, `migrate`, plus `Note` and `NotesError` |
| `tests/notes.rs` | 12 integration tests over an in-memory SQLite pool |

## Why sqlx + raw SQL?

You write the exact SQL the database runs. No ORM abstraction sits between you and the planner. When `EXPLAIN ANALYZE` says "use a different index" you change the SQL directly. That's the operational ceiling we need for the capstone.

For Postgres (Phase 4 onward), the only changes are:

- `?` placeholders become `$1`, `$2`.
- `INTEGER PRIMARY KEY AUTOINCREMENT` becomes `BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY`.
- `strftime(...)` becomes `now()`.
- The connection URL changes from `sqlite::memory:` to `postgres://…`.

Otherwise the same code works. That's the reward for hand-writing SQL.
