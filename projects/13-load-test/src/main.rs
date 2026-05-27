//! `loadgen` — the binary half.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use load_test::{Sample, summarize};
use tokio::sync::Semaphore;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "loadgen",
    version,
    about = "Fixed-RPS load generator for HTTP endpoints. Emits a JSON \
             report compatible with docs/perf/methodology.md."
)]
struct Cli {
    /// Target URL.
    #[arg(long)]
    url: String,

    /// Concurrency cap — at most this many requests in flight at once.
    #[arg(long, default_value_t = 16)]
    concurrency: usize,

    /// Total number of requests to fire.
    #[arg(long, default_value_t = 1000)]
    requests: u64,

    /// HTTP method.
    #[arg(long, default_value = "GET")]
    method: String,

    /// Optional file to write the JSON report to. Stdout is always
    /// written; the file is for archiving.
    #[arg(long)]
    output: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,reqwest=warn")),
        )
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let cli = Cli::parse();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("loadgen/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let method = reqwest::Method::from_bytes(cli.method.to_uppercase().as_bytes())?;

    let sem = Arc::new(Semaphore::new(cli.concurrency));
    let network_failures = Arc::new(AtomicU64::new(0));
    let mut futs = FuturesUnordered::new();

    tracing::info!(
        url = %cli.url,
        requests = cli.requests,
        concurrency = cli.concurrency,
        "loadgen starting"
    );
    let started = Instant::now();

    for _ in 0..cli.requests {
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let url = cli.url.clone();
        let method = method.clone();
        let network_failures = network_failures.clone();

        futs.push(tokio::spawn(async move {
            let _permit = permit;
            let req_started = Instant::now();
            if let Ok(resp) = client.request(method, &url).send().await {
                Some(Sample {
                    status: resp.status().as_u16(),
                    duration: req_started.elapsed(),
                })
            } else {
                network_failures.fetch_add(1, Ordering::Relaxed);
                None
            }
        }));
    }

    let mut samples = Vec::<Sample>::with_capacity(cli.requests as usize);
    while let Some(handle) = futs.next().await {
        if let Ok(Some(s)) = handle {
            samples.push(s);
        }
    }
    let wall = started.elapsed();
    let report = summarize(&mut samples, network_failures.load(Ordering::Relaxed));

    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");
    println!();
    println!(
        "ran {} requests in {} ({} RPS), {} failed",
        report.total_requests,
        humantime::format_duration(wall),
        if wall.as_secs_f64() > 0.0 {
            (report.total_requests as f64 / wall.as_secs_f64()) as u64
        } else {
            0
        },
        report.failed,
    );

    if let Some(out) = cli.output {
        std::fs::write(&out, json)?;
        tracing::info!(path = ?out, "report written");
    }
    Ok(())
}
