//! outbox-demo CLI: enqueue, worker, status.
//!
//! Three subcommands:
//!   record   produce a transfer (writes business + outbox atomically)
//!   worker   run the worker loop until ctrl-C
//!   stats    print queue stats and exit

use anyhow::Context;
use clap::{Parser, Subcommand};
use sqlx::sqlite::SqlitePoolOptions;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

use outbox_demo::{Dispatcher, OutboxRow, migrate, record_transfer, run_loop, stats};

#[derive(Debug, Parser)]
#[command(name = "outbox-demo", version, about)]
struct Cli {
    /// Database URL. Defaults to a local file so state survives between runs.
    #[arg(long, default_value = "sqlite:./outbox-demo.sqlite?mode=rwc")]
    database_url: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Record a transfer (writes business row + outbox row atomically).
    Record {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount_cents: i64,
    },
    /// Run the outbox worker loop. Stops on ctrl-C.
    Worker {
        #[arg(long, default_value_t = 5)]
        max_attempts: i64,
        #[arg(long, default_value = "500ms", value_parser = humantime_parse)]
        idle_poll: Duration,
    },
    /// Print queue stats (pending / processing / done / failed) and exit.
    Stats,
}

fn humantime_parse(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| e.to_string())
}

struct PrintDispatcher;
impl Dispatcher for PrintDispatcher {
    fn handle(&self, row: &OutboxRow) -> Result<(), String> {
        println!(
            "processed outbox#{:>5} {} : {}",
            row.id, row.kind, row.payload
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,outbox_demo=debug")),
        )
        .compact()
        .init();

    let cli = Cli::parse();

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&cli.database_url)
        .await
        .context("connect to sqlite")?;
    migrate(&pool).await?;

    match cli.cmd {
        Cmd::Record {
            from,
            to,
            amount_cents,
        } => {
            let (t, outbox_id) = record_transfer(&pool, &from, &to, amount_cents).await?;
            println!(
                "recorded transfer #{} (outbox #{}): {} → {} of {} cents",
                t.id, outbox_id, t.from_account, t.to_account, t.amount_cents
            );
        }
        Cmd::Worker {
            max_attempts,
            idle_poll,
        } => {
            println!(
                "outbox worker: max_attempts={max_attempts} idle_poll={idle_poll:?} (ctrl-C to stop)"
            );
            run_loop(
                pool.clone(),
                PrintDispatcher,
                idle_poll,
                max_attempts,
                async {
                    let _ = tokio::signal::ctrl_c().await;
                },
            )
            .await?;
        }
        Cmd::Stats => {
            let s = stats(&pool).await?;
            println!(
                "pending={} processing={} done={} failed={}",
                s.pending, s.processing, s.done, s.failed
            );
        }
    }
    Ok(())
}
