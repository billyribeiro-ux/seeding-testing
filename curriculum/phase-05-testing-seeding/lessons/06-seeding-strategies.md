# Lesson 5.6 — Seeding Strategies

> **Concept first:** a *seed* is a script that fills a database with data. Different audiences want different data — devs want enough to navigate, demoers want a polished story, load tests want millions of rows. Build three seeds, one per profile.
> **Time:** 20 minutes.

## The three profiles

| Profile | Audience | Data |
|---|---|---|
| `dev` | A developer who just cloned the repo | A few users, a few notes, enough to click around |
| `demo` | A salesperson demoing to a customer | Curated, polished, brand-aware ("MemberClub Demo Inc.") |
| `load` | A load test or perf benchmark | Millions of rows, realistic distribution, deterministic |

A real seed CLI takes `--profile dev|demo|load`. The shape is the same; the data is wildly different.

## Idempotency is non-negotiable

Running the seed twice should not double the data. Three approaches:

### 1. Truncate-and-load (`dev` profile)

```sql
TRUNCATE notes, users, sessions RESTART IDENTITY CASCADE;
```

Then insert. Simple, fast, deterministic — fine for the dev DB where data is disposable.

### 2. Upsert (`demo` profile)

```sql
INSERT INTO users (email, password_hash, is_admin)
VALUES ('alice@example.com', '$argon2id...', false)
ON CONFLICT (email) DO UPDATE SET
    password_hash = EXCLUDED.password_hash,
    is_admin      = EXCLUDED.is_admin;
```

Running twice updates to the desired state, never duplicates. The demo set always converges to the curated story.

### 3. Watermark (`load` profile)

```rust
let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes").fetch_one(&pool).await?;
let want = 1_000_000;
if existing < want {
    insert_many(want - existing as usize).await;
}
```

For load profiles, only top-up; never wipe an existing load dataset to avoid re-paying the multi-minute insert cost.

## Structure: one file per profile, one runner

```
apps/memberclub/db/seeds/
├── dev.rs      # pub async fn seed(pool: &PgPool) -> Result<()>
├── demo.rs
└── load.rs

apps/memberclub/api/src/bin/notes-seed.rs   # CLI dispatcher
```

CLI shape:

```bash
notes-seed --profile dev
notes-seed --profile demo --database-url postgres://...
notes-seed --profile load --count 1000000
```

Each profile is a single function. The CLI is a thin clap dispatcher.

## Safety: refuse to run on prod

```rust
let url = std::env::var("DATABASE_URL")?;
if profile != Profile::Demo && url.contains("prod") {
    anyhow::bail!("refusing to run profile {:?} against a URL that contains 'prod'", profile);
}
```

You'll thank yourself once.

For the *demo* profile, sometimes you *do* run against a production-like environment (a customer-facing demo site). Make that an explicit `--allow-prod` flag with a confirmation prompt.

## Determinism

If you use `fake` in seeds, seed the RNG explicitly:

```rust
let mut rng = StdRng::seed_from_u64(42);
for _ in 0..count {
    let email: String = SafeEmail().fake_with_rng(&mut rng);
    // …
}
```

Same input → same output. Critical for performance regression testing (the seed creates the same dataset every time, so timing diffs are signal, not noise).

## Transactions and batching

For the `load` profile, **wrap inserts in batches** of ~1000 rows per transaction. Otherwise:

- Single transaction for 1M inserts = giant WAL, long lock holds.
- Single autocommit per row = 1M fsyncs.

```rust
let mut tx = pool.begin().await?;
for (i, row) in rows.enumerate() {
    sqlx::query!(...).execute(&mut *tx).await?;
    if i % 1000 == 0 { tx.commit().await?; tx = pool.begin().await?; }
}
tx.commit().await?;
```

`COPY ... FROM STDIN` is even faster for huge inserts; reach for it when batching isn't enough.

## Audit logging from the seed

Even seeds write audit log entries (with `actor_id = NULL`, action = `"seed.dev"`). It's a real event. Audit logs are append-only; the seed shouldn't be exempt.

## A worked dev seed for `notes-api`

```rust
pub async fn seed_dev(pool: &SqlitePool, count: usize) -> anyhow::Result<()> {
    // 1. Reset
    sqlx::query("DELETE FROM notes").execute(pool).await?;
    sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'notes'").execute(pool).await.ok();

    // 2. Load
    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    use fake::Fake;
    use fake::faker::lorem::en::Sentence;
    for _ in 0..count {
        let body: String = Sentence(1..4).fake_with_rng(&mut rng);
        sqlx_notes::add(pool, &body).await?;
    }

    tracing::info!(count, "dev seed complete");
    Ok(())
}
```

## Why this matters

- **A seed CLI is a runbook.** New hires bootstrap their environment in 30 seconds, every time.
- **Different audiences need different data.** One profile can't serve devs *and* demoers *and* load tests.
- **Idempotency means the seed is safe to run any time.** No "did I run this already?" hesitation.

## Green-bar checkpoint

- You can sketch the three profiles and one example of each kind of data.
- You can write an upsert that converges to a fixed state.
- You can explain why a load profile uses watermarks instead of truncating.

Next: `lessons/07-coverage-as-leading-indicator.md`.
