//! notes-seed — a CLI that runs one of the seed profiles against the configured database.
//!
//! Exit codes:
//!   0 — success
//!   1 — seed failed (DB error)
//!   2 — refused to run against a URL containing 'prod' without --allow-prod

use anyhow::Context;
use clap::Parser;
use sqlx::sqlite::SqlitePoolOptions;

use notes_api::seed::{self, Profile};

#[derive(Debug, Parser)]
#[command(name = "notes-seed", version, about)]
struct Cli {
    /// Which profile to run.
    #[arg(long, value_enum)]
    profile: Profile,

    /// Target row count (only used by `dev` and `load`).
    #[arg(long, default_value_t = 20)]
    count: usize,

    /// Override the safety guard that refuses URLs containing 'prod'.
    #[arg(long)]
    allow_prod: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().compact().init();
    let cli = Cli::parse();

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    if !cli.allow_prod && url.to_lowercase().contains("prod") {
        eprintln!(
            "notes-seed: refusing to run profile {:?} against {url:?} (URL contains 'prod'). \
             Pass --allow-prod to override.",
            cli.profile
        );
        std::process::exit(2);
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(if url == "sqlite::memory:" { 1 } else { 4 })
        .connect(&url)
        .await
        .context("connect to database")?;

    sqlx_notes::migrate(&pool)
        .await
        .context("apply migrations")?;

    let started = std::time::Instant::now();
    let total = seed::run(&pool, cli.profile, cli.count).await?;
    let elapsed = started.elapsed();

    println!(
        "notes-seed: profile={:?} count={} elapsed={:?} → total_rows={}",
        cli.profile, cli.count, elapsed, total
    );
    Ok(())
}
