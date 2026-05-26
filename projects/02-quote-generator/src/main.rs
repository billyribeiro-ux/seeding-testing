//! quote-generator binary.
//!
//! Exit codes:
//!   0 — every URL succeeded
//!   1 — at least one URL failed (network error, timeout, or non-2xx)
//!   2 — bad input (no URLs provided, file not found, invalid duration)
//!   3 — overall deadline exceeded (some URLs not completed)

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use futures::StreamExt;
use quote_generator::{FetchConfig, fetch_all, render};

#[derive(Debug, Parser)]
#[command(name = "quote-generator", version, about)]
struct Cli {
    /// URLs to fetch (positional). May be combined with --file.
    urls: Vec<String>,

    /// Path to a file with one URL per line.
    #[arg(short = 'f', long)]
    file: Option<PathBuf>,

    /// Maximum number of in-flight requests.
    #[arg(short = 'n', long, default_value_t = 4)]
    concurrency: usize,

    /// Per-request timeout (e.g. "500ms", "2s", "30s").
    #[arg(short = 't', long, default_value = "5s", value_parser = parse_duration)]
    timeout: Duration,

    /// Overall batch deadline (optional).
    #[arg(short = 'd', long, value_parser = parse_duration)]
    deadline: Option<Duration>,

    /// Number of body bytes to include in the preview column.
    #[arg(long, default_value_t = 0)]
    preview_bytes: usize,
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("invalid duration {s:?}: {e}"))
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("warn,quote_generator=info")
            }),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    let urls = match collect_urls(&cli) {
        Ok(v) if v.is_empty() => {
            eprintln!("quote-generator: no URLs provided (pass them as arguments or via --file)");
            return ExitCode::from(2);
        }
        Ok(v) => v,
        Err(msg) => {
            eprintln!("quote-generator: {msg}");
            return ExitCode::from(2);
        }
    };

    let cfg = FetchConfig {
        concurrency: cli.concurrency.max(1),
        per_request_timeout: cli.timeout,
        overall_deadline: cli.deadline,
        preview_bytes: cli.preview_bytes,
    };

    let client = reqwest::Client::builder()
        .user_agent(concat!("quote-generator/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("client builds");

    let total = urls.len();
    let mut stream = fetch_all(client, urls, cfg.clone());

    let consume = async {
        let mut ok = 0usize;
        let mut fail = 0usize;
        while let Some(r) = stream.next().await {
            println!("{}", render(&r));
            match &r {
                Ok(f) if (200..300).contains(&f.status) => ok += 1,
                _ => fail += 1,
            }
        }
        (ok, fail)
    };

    let (ok, fail) = if let Some(dl) = cfg.overall_deadline {
        let Ok(counts) = tokio::time::timeout(dl, consume).await else {
            eprintln!("quote-generator: overall deadline {dl:?} exceeded");
            return ExitCode::from(3);
        };
        counts
    } else {
        consume.await
    };

    eprintln!("quote-generator: completed — total={total} ok={ok} fail={fail}");
    if fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn collect_urls(cli: &Cli) -> Result<Vec<String>, String> {
    let mut urls = cli.urls.clone();
    if let Some(p) = &cli.file {
        let content = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                urls.push(trimmed.to_string());
            }
        }
    }
    Ok(urls)
}
