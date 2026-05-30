# Lesson 3.8 — sqlx Fundamentals

> **Concept first:** sqlx lets you write raw SQL in Rust strings while the compiler *type-checks* the SQL against a real database. Best of both worlds: ORM-grade safety, hand-written-SQL-grade control.
> **Time:** 45 minutes.

## The mental model

You write SQL. sqlx checks it. At compile time. Against a real Postgres (or against a cached `.sqlx/` snapshot if you're offline). When the SQL is correct, you ship.

The two macros that make this work:

| Macro | Returns |
|---|---|
| `sqlx::query!(...)` | An anonymous, statically-checked query. Columns become struct fields by name. |
| `sqlx::query_as!(Type, ...)` | Same checks, but maps the result into your named type `Type`. |

There are also runtime versions (`sqlx::query`, `sqlx::query_as::<_, T>`) for when you can't know the SQL at compile time (e.g. dynamic ORDER BY column). 99% of code uses the macros.

## The connection pool

A `sqlx::Pool<DB>` holds a configurable number of connections and hands them out per query. **One pool per process.**

```rust
use sqlx::{PgPool, postgres::PgPoolOptions};

let pool: PgPool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(2)
    .acquire_timeout(std::time::Duration::from_secs(5))
    .connect(&std::env::var("DATABASE_URL")?).await?;
```

Pass `&pool` around. Sharing is `Arc`-cheap.

## `query!` — anonymous, compile-time-checked

```rust
let row = sqlx::query!(
    "SELECT id, email, created_at FROM users WHERE id = $1",
    user_id           // i64
)
.fetch_one(&pool)
.await?;

println!("{} {}", row.id, row.email);
```

Behind the scenes, sqlx connects to your DB at *compile time*, runs `PREPARE`, and uses the result types to generate a Rust struct on the fly. If you mistype `emial`, your build fails. If `users` doesn't have a `created_at` column, your build fails. If `$1` is the wrong type, your build fails.

## `query_as!` — into your own type

```rust
struct User {
    id: i64,
    email: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

let user = sqlx::query_as!(
    User,
    "SELECT id, email, created_at FROM users WHERE id = $1",
    user_id
)
.fetch_one(&pool)
.await?;
```

Same checks, but the result is your `User`. We prefer `query_as!` for return values that flow further than one function.

Note the `query_as!` *macro* assigns columns to fields **by position**, in `SELECT` order — it does not require (or use) a `#[derive(sqlx::FromRow)]`. That derive is for the *runtime* form `query_as::<_, User>(...)` (the one `projects/02c-sqlx-notes` uses), which maps by column name instead.

## Fetch modes

| Method | Returns | Use when |
|---|---|---|
| `.fetch_all(&pool)`     | `Vec<Row>` | You want everything |
| `.fetch_one(&pool)`     | `Row` (errors if 0 or >1) | You expect exactly one (e.g. by PK) |
| `.fetch_optional(&pool)`| `Option<Row>` (errors only if >1) | You expect zero or one |
| `.execute(&pool)`       | `QueryResult` (`rows_affected()`; `last_insert_rowid()` on SQLite — Postgres has no auto-id here, use `RETURNING`) | INSERT / UPDATE / DELETE |
| `.fetch(&pool)`         | A `Stream<Item = Row>` | Large result sets — read row by row |

## Transactions

```rust
let mut tx = pool.begin().await?;

sqlx::query!("INSERT INTO orders (user_id, amount_cents) VALUES ($1, $2)", uid, amount)
    .execute(&mut *tx).await?;

sqlx::query!("INSERT INTO payments (order_id, status) VALUES (lastval(), 'pending')")
    .execute(&mut *tx).await?;

tx.commit().await?;
// If you return Err before commit, Drop ROLLBACKs for you.
```

The `&mut *tx` re-borrow is required by the borrow checker; it's the standard sqlx idiom.

## Offline mode — `.sqlx/`

Compile-time checking needs a live DB. For CI, deploy artifacts, and offline builds, sqlx supports an offline cache:

```bash
export SQLX_OFFLINE=true
cargo sqlx prepare --workspace      # write .sqlx/ snapshots of every query
git add .sqlx && git commit         # commit them
```

Now `cargo build` uses the cached query metadata instead of connecting to a DB. CI sets `SQLX_OFFLINE=true` and reads `.sqlx/`.

When you change a query and forget to re-run `prepare`, the build fails locally (no `.sqlx/` entry). CI catches it.

## Migrations from Rust

```rust
sqlx::migrate!("./migrations").run(&pool).await?;
```

This reads `migrations/<timestamp>_*.sql`, applies anything not yet applied, and tracks state in a `_sqlx_migrations` table. Same files `sqlx-cli` uses.

## Type mappings (Postgres)

| SQL type | Rust type (via sqlx) |
|---|---|
| `BIGINT`, `INT8` | `i64` |
| `INTEGER`, `INT4` | `i32` |
| `BOOL` | `bool` |
| `TEXT`, `VARCHAR`, `CHAR` | `String` |
| `TIMESTAMPTZ` | `chrono::DateTime<Utc>` (with `chrono` feature) |
| `UUID` | `uuid::Uuid` (with `uuid` feature) |
| `JSONB` | `serde_json::Value` or `sqlx::types::Json<T>` |
| `NUMERIC` | `rust_decimal::Decimal` (with `rust_decimal` feature) |
| `BYTEA` | `Vec<u8>` |

Enable features in `Cargo.toml`:

```toml
sqlx = { workspace = true, features = [
    "runtime-tokio", "tls-rustls", "postgres", "macros", "migrate",
    "chrono", "uuid", "json"
]}
```

## SQLite mode (what `projects/02c-sqlx-notes` uses)

For the curriculum's hard-evidence project we use `sqlx::SqlitePool` with an in-memory database — no Docker required, identical API. The only differences:

| Postgres | SQLite |
|---|---|
| `$1` placeholders | `?` placeholders |
| `BIGINT GENERATED ALWAYS AS IDENTITY` | `INTEGER PRIMARY KEY AUTOINCREMENT` |
| `TIMESTAMPTZ`, `now()` | `TEXT`, `strftime('%Y-%m-%dT%H:%M:%fZ','now')` |
| `RETURNING *` (since v8.2) | `RETURNING *` (since v3.35) |

Everything else is the same.

## Why this matters

- **Compile-time SQL checks** prevent the most common bug class in raw-SQL code: a column rename that breaks queries.
- **Offline mode keeps CI fast and reproducible.** The `.sqlx/` directory is your "this code typechecks against this schema" proof.
- **Hand-written SQL is the highest-ceiling option.** Every Postgres feature (window functions, materialized views, partitioning, custom types) is available. ORMs always lag.

## Green-bar checkpoint

- You can write a `query_as!` that returns a `Vec<MyType>` and explain how the type-check happens.
- You can apply migrations via both `sqlx-cli` and `sqlx::migrate!` from code.
- You can articulate when to use `fetch_one` vs `fetch_optional` vs `execute`.

Next: `lessons/09-stepB-build-sqlx-notes.md`.
