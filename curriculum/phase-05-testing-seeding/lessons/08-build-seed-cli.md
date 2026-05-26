# Lesson 5.8 — Build the Seed CLI

> **The capstone of Phase 5.** Build a `notes-seed` binary that takes a `--profile dev|demo|load --count N` and idempotently seeds the database.
> **Time:** 60 minutes.

## The shape

We extend `projects/03-notes-api/` with a *second* binary:

```toml
# notes-api/Cargo.toml
[[bin]]
name = "notes-api"
path = "src/main.rs"

[[bin]]
name = "notes-seed"
path = "src/bin/notes-seed.rs"
```

Run it:

```bash
cargo run -p notes-api --bin notes-seed -- --profile dev --count 20
cargo run -p notes-api --bin notes-seed -- --profile demo
cargo run -p notes-api --bin notes-seed -- --profile load --count 1000000
```

## The CLI

```rust
// src/bin/notes-seed.rs
use anyhow::Context;
use clap::{Parser, ValueEnum};
use sqlx::sqlite::SqlitePoolOptions;

use notes_api::seed;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Profile { Dev, Demo, Load }

#[derive(Debug, Parser)]
#[command(name = "notes-seed", version, about)]
struct Cli {
    #[arg(long, value_enum)]
    profile: Profile,

    #[arg(long, default_value_t = 20)]
    count: usize,

    #[arg(long)]
    allow_prod: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().compact().init();
    let cli = Cli::parse();

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::dev.sqlite".into());
    if !cli.allow_prod && url.contains("prod") {
        anyhow::bail!("URL contains 'prod'; pass --allow-prod to override");
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(if url == "sqlite::memory:" { 1 } else { 4 })
        .connect(&url).await.context("connect")?;
    sqlx_notes::migrate(&pool).await.context("apply migrations")?;

    let started = std::time::Instant::now();
    match cli.profile {
        Profile::Dev  => seed::seed_dev(&pool, cli.count).await?,
        Profile::Demo => seed::seed_demo(&pool).await?,
        Profile::Load => seed::seed_load(&pool, cli.count).await?,
    }
    tracing::info!(?cli.profile, elapsed=?started.elapsed(), "seed complete");
    Ok(())
}
```

Three habits:

1. **Pull `DATABASE_URL` from env.** The CLI is the same on dev, demo, and load environments.
2. **Refuse to run against a 'prod' URL unless explicitly overridden.** Safety net.
3. **Print timings.** Operators love knowing how long a 1M-row load takes on their hardware.

## The library

```rust
// src/lib.rs (notes-api)
pub mod seed {
    use sqlx::SqlitePool;
    use fake::Fake;
    use fake::faker::lorem::en::Sentence;
    use rand::SeedableRng;

    pub async fn seed_dev(pool: &SqlitePool, count: usize) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM notes").execute(pool).await?;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for _ in 0..count {
            let body: String = Sentence(1..4).fake_with_rng(&mut rng);
            sqlx_notes::add(pool, &body).await?;
        }
        Ok(())
    }

    pub async fn seed_demo(pool: &SqlitePool) -> anyhow::Result<()> {
        let bodies = [
            "Welcome to MemberClub!",
            "Q3 retro — what worked, what didn't",
            "Reminder: incident runbook lives at docs/runbooks/",
        ];
        for body in bodies {
            // Upsert by body — not realistic for production but fine for demo.
            sqlx_notes::add(pool, body).await.ok();
        }
        Ok(())
    }

    pub async fn seed_load(pool: &SqlitePool, count: usize) -> anyhow::Result<()> {
        let mut tx = pool.begin().await?;
        for i in 0..count {
            sqlx::query("INSERT INTO notes (body) VALUES (?)")
                .bind(format!("load-{i}"))
                .execute(&mut *tx).await?;
            if i % 1000 == 999 {
                tx.commit().await?;
                tx = pool.begin().await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
```

The three profiles. Each is a single function. Add tests for each (especially that re-running `seed_dev` doesn't double the count).

## Tests for the seed

```rust
#[tokio::test]
async fn seed_dev_is_idempotent() {
    let pool = fresh_pool().await;
    seed::seed_dev(&pool, 5).await.unwrap();
    let first = sqlx_notes::list(&pool).await.unwrap().len();
    seed::seed_dev(&pool, 5).await.unwrap();
    let second = sqlx_notes::list(&pool).await.unwrap().len();
    assert_eq!(first, second, "running seed twice should converge");
}
```

## Snapshot test for the demo

```rust
#[tokio::test]
async fn demo_set_is_stable() {
    let pool = fresh_pool().await;
    seed::seed_demo(&pool).await.unwrap();
    let bodies: Vec<String> = sqlx_notes::list(&pool).await.unwrap()
        .into_iter().map(|n| n.body).collect();
    insta::assert_json_snapshot!(bodies);
}
```

If a future commit adds a demo note, this snapshot fails — forcing the author to *accept* the new demo set as a deliberate change.

## CI integration

Add a `seed` job that runs each profile against a fresh in-memory DB, capping the load count:

```yaml
seed-smoke:
  steps:
    - run: cargo run -p notes-api --bin notes-seed -- --profile dev  --count 5
    - run: cargo run -p notes-api --bin notes-seed -- --profile demo
    - run: cargo run -p notes-api --bin notes-seed -- --profile load --count 1000
```

This catches "the load seed broke after the schema change" before it ruins a perf run.

## Why this matters

- **A seed CLI is the first script any new hire runs.** Make it perfect.
- **Three profiles ≠ three forks of the same script.** They share infrastructure (pool, migration, env-var safety) and differ in the data they produce.
- **Idempotency + safety guards turn "scary" scripts into routine.** Anyone on the team can run them without anxiety.

## Green-bar checkpoint

- You can sketch the CLI dispatch and the three profile functions.
- You can write an idempotency test for `seed_dev`.
- You can articulate why `seed_load` batches in transactions of ~1000.

Phase 5 is complete. Phase 6 — **Auth** — adds password hashing, sessions, JWT, and 2FA.
