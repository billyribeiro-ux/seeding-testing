# Lesson 5.3 — Integration Tests with `testcontainers`

> **Concept first:** mocking the database means testing the mock, not the system. `testcontainers` spins up a *real* Postgres container per test run. That's the gold standard.
> **Time:** 25 minutes.

## The mental model

- **Tests against in-memory SQLite** (what `sqlx-notes/tests/notes.rs` does today) are fast and hermetic. But SQLite isn't Postgres — some queries behave subtly differently. Good for the data-layer unit tests; not enough for the capstone.
- **Tests against a single shared Postgres** in `compose.yaml` are realistic but leaky — tests step on each other if they share schema.
- **Tests against a fresh testcontainers Postgres** combine the two: real engine, hermetic, per-suite isolation.

This is what we'll use for MemberClub from Phase 6 onward.

## The basic shape

```toml
[dev-dependencies]
testcontainers          = "0.23"
testcontainers-modules  = { version = "0.11", features = ["postgres"] }
sqlx                    = { workspace = true, features = ["postgres", "macros", "migrate"] }
```

```rust
use testcontainers_modules::postgres::Postgres;
use testcontainers::runners::AsyncRunner;
use sqlx::PgPool;

async fn fresh_pg() -> (PgPool, impl Drop) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url  = format!("postgres://postgres@127.0.0.1:{port}/postgres");
    let pool = PgPool::connect(&url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, container)
}

#[tokio::test]
async fn user_can_create_a_note() {
    let (pool, _container) = fresh_pg().await;
    // … assertions …
}
// _container is dropped here, which stops the Postgres container.
```

Per-test fresh database, ~1–3 second startup, totally isolated.

## "But Docker isn't available in CI"

It is in GitHub Actions (and most CI). Our `ci.yml` already runs Postgres as a service container. For testcontainers specifically:

- **GHA Linux runners** — Docker is available. testcontainers works out of the box.
- **GHA macOS runners** — Docker is NOT available by default. Either avoid macOS for tests, or use `Colima` / `Lima`.
- **Self-hosted runners** — install Docker.

The repo's `ci.yml` runs on `ubuntu-latest` for tests; this is the right call.

## "But Docker isn't available *locally*" (Phase 5 honesty box)

This sandbox doesn't have a Docker socket. The Phase 5 capstone *demonstrates* the pattern but the in-curriculum projects keep using **in-memory SQLite** for hard evidence — same sqlx API, no Docker dependency, identical test code shape. When you move to a workstation with Docker, swap `SqlitePool` for `PgPool`, use the snippet above, run `cargo nextest run`. Done.

## Patterns we use

### 1. Per-test container vs per-suite container

```rust
// per-test — the cleanest, ~1–3 s overhead per test
#[tokio::test]
async fn each_test_gets_its_own_db() { let (pool, _c) = fresh_pg().await; ... }

// per-suite — share a container across many tests, use a fresh schema per test
static PG: OnceCell<(PgConnection, Container)> = OnceCell::new();
```

Default to per-test. Move to per-suite only if you measure the overhead and care.

### 2. Schema reset between tests

Instead of starting a new container, you can wrap each test in a `BEGIN; ... ROLLBACK;` to throw away its changes. Faster, but fragile (DDL inside a transaction is finicky in some setups).

### 3. Reusable fixtures

```rust
async fn signed_in_user(pool: &PgPool) -> (User, Cookie) {
    let user = factory::user().email("alice@b.com").insert(pool).await;
    let session = factory::session().for_user(user.id).insert(pool).await;
    (user, session.cookie_value())
}

#[tokio::test]
async fn admin_can_delete_user() {
    let (pool, _c) = fresh_pg().await;
    let (admin, cookie) = signed_in_user(&pool).await;
    // …
}
```

Factories + fixtures keep test setup minimal. See Lesson 5.5.

## Why this matters

- **Real Postgres in tests** catches behaviour SQLite hides: `RETURNING` semantics, advisory locks, JSONB, `tsvector`, NUMERIC math.
- **Hermetic per-test DBs** make parallelism free — tests can't step on each other.
- **The exact same library code is tested against the production engine.** No more "works on SQLite, fails on Postgres at 3 AM."

## Green-bar checkpoint

- You can write `fresh_pg()` from memory.
- You can articulate when per-test container is right vs per-suite + schema reset.
- You can explain why mocking the DB in unit tests is *worse than* in-memory SQLite for our purposes.

Next: `lessons/04-snapshot-and-property.md`.
