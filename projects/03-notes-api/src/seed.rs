//! Seed profiles for the notes domain.
//!
//! Three audiences: dev (random plausible), demo (curated story), load (millions).
//! Each profile is idempotent — running twice converges, never duplicates.

use anyhow::Context;
use clap::ValueEnum;
use fake::Fake;
use fake::faker::lorem::en::Sentence;
use rand::SeedableRng;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Profile {
    Dev,
    Demo,
    Load,
}

/// Run the given profile. Returns the number of rows present in `notes` after seeding.
pub async fn run(pool: &SqlitePool, profile: Profile, count: usize) -> anyhow::Result<usize> {
    match profile {
        Profile::Dev => seed_dev(pool, count).await?,
        Profile::Demo => seed_demo(pool).await?,
        Profile::Load => seed_load(pool, count).await?,
    }
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .context("count notes")?;
    let count = usize::try_from(rows).unwrap_or(0);
    Ok(count)
}

/// Wipes the notes table and inserts `count` random plausible notes.
pub async fn seed_dev(pool: &SqlitePool, count: usize) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM notes").execute(pool).await?;
    // Reset autoincrement so IDs start from 1 again (sqlite-specific).
    sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'notes'")
        .execute(pool)
        .await
        .ok();

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for _ in 0..count {
        let body: String = Sentence(1..4).fake_with_rng(&mut rng);
        sqlx_notes::add(pool, &body).await?;
    }
    Ok(())
}

/// Inserts a fixed curated demo set; idempotent via dedup on body.
pub async fn seed_demo(pool: &SqlitePool) -> anyhow::Result<()> {
    let demo: &[&str] = &[
        "Welcome to MemberClub — your members-only community.",
        "Q3 retro — what worked, what didn't",
        "Reminder: incident runbook lives at docs/runbooks/",
        "New feature: subscription gifting (coming next sprint)",
        "Operating philosophy — small commits, green CI, no merging on Fridays",
    ];

    for body in demo {
        // Check existence first; idempotent insert.
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM notes WHERE body = ?")
            .bind(body)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            sqlx_notes::add(pool, body).await?;
        }
    }
    Ok(())
}

/// Batched insert for performance datasets.
pub async fn seed_load(pool: &SqlitePool, count: usize) -> anyhow::Result<()> {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await?;
    let want = i64::try_from(count).unwrap_or(i64::MAX);
    if existing >= want {
        return Ok(()); // watermark — already at or past the target
    }
    let mut tx = pool.begin().await?;
    let mut inserted = 0i64;
    for i in existing..want {
        sqlx::query("INSERT INTO notes (body) VALUES (?)")
            .bind(format!("load-{i}"))
            .execute(&mut *tx)
            .await?;
        inserted += 1;
        if inserted % 1000 == 0 {
            tx.commit().await?;
            tx = pool.begin().await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx_notes::migrate(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn dev_inserts_requested_count() {
        let pool = fresh_pool().await;
        let total = run(&pool, Profile::Dev, 7).await.unwrap();
        assert_eq!(total, 7);
    }

    #[tokio::test]
    async fn dev_is_idempotent() {
        let pool = fresh_pool().await;
        let a = run(&pool, Profile::Dev, 5).await.unwrap();
        let b = run(&pool, Profile::Dev, 5).await.unwrap();
        assert_eq!(a, b, "rerunning dev seed should converge");
    }

    #[tokio::test]
    async fn dev_is_deterministic() {
        let p1 = fresh_pool().await;
        run(&p1, Profile::Dev, 3).await.unwrap();
        let bodies1 = sqlx_notes::list(&p1)
            .await
            .unwrap()
            .into_iter()
            .map(|n| n.body)
            .collect::<Vec<_>>();

        let p2 = fresh_pool().await;
        run(&p2, Profile::Dev, 3).await.unwrap();
        let bodies2 = sqlx_notes::list(&p2)
            .await
            .unwrap()
            .into_iter()
            .map(|n| n.body)
            .collect::<Vec<_>>();

        assert_eq!(
            bodies1, bodies2,
            "deterministic seed must produce same data"
        );
    }

    #[tokio::test]
    async fn demo_is_idempotent() {
        let pool = fresh_pool().await;
        let a = run(&pool, Profile::Demo, 0).await.unwrap();
        let b = run(&pool, Profile::Demo, 0).await.unwrap();
        assert_eq!(a, b);
        assert!(a >= 5, "demo set should have at least 5 notes; got {a}");
    }

    #[tokio::test]
    async fn load_only_tops_up() {
        let pool = fresh_pool().await;
        run(&pool, Profile::Load, 10).await.unwrap();
        // Second call with a smaller target must not delete anything.
        let total = run(&pool, Profile::Load, 5).await.unwrap();
        assert_eq!(total, 10);
    }

    #[tokio::test]
    async fn load_inserts_to_watermark() {
        let pool = fresh_pool().await;
        let total = run(&pool, Profile::Load, 50).await.unwrap();
        assert_eq!(total, 50);
    }
}
