//! `capacity-planner` — the binary half.
//!
//! Reads the inputs from CLI flags, runs the planner, and prints both
//! a JSON object (machine-friendly; pipe to `jq`) and a small ASCII
//! table (human-friendly; paste into a design doc).
//!
//! ```bash
//! cargo run -p capacity-planner -- \
//!     --dau 100000 --requests-per-user 50 \
//!     --p99-budget-ms 200 --observed-p99-ms 80
//! ```

use anyhow::Result;
use capacity_planner::{Inputs, plan, render_table};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "capacity-planner",
    version,
    about = "Back-of-envelope capacity + cost calculator. Given DAU, \
             reqs/user, peak multiplier, and a measured per-core RPS, \
             prints cores, instances, monthly $$$, and p99 health."
)]
struct Cli {
    /// Daily active users.
    #[arg(long)]
    dau: u64,

    /// Average requests per user per day.
    #[arg(long)]
    requests_per_user: u64,

    /// Peak QPS = avg QPS × this multiplier.
    #[arg(long, default_value_t = 3.0)]
    peak_multiplier: f64,

    /// Allowable p99 latency in ms.
    #[arg(long)]
    p99_budget_ms: u64,

    /// Observed p99 latency in ms on a reference machine.
    #[arg(long)]
    observed_p99_ms: u64,

    /// CPU cores per VM/container.
    #[arg(long, default_value_t = 4)]
    cores_per_instance: u32,

    /// Requests-per-second a single core can hold within budget.
    #[arg(long, default_value_t = 200)]
    reqs_per_core_per_sec: u32,

    /// Spare-capacity percent (e.g. 30 → plan for 1.30× peak).
    #[arg(long, default_value_t = 30)]
    headroom_pct: u32,

    /// Hourly USD per instance.
    #[arg(long, default_value_t = 0.05)]
    cost_per_instance_hr: f64,

    /// Replica count for HA (≥ 1).
    #[arg(long, default_value_t = 2)]
    replication: u32,

    /// Emit only the JSON (suppress the table). Handy when piping
    /// into `jq` or another tool.
    #[arg(long)]
    json_only: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let inputs = Inputs {
        dau: cli.dau,
        requests_per_user: cli.requests_per_user,
        peak_multiplier: cli.peak_multiplier,
        p99_budget_ms: cli.p99_budget_ms,
        observed_p99_ms: cli.observed_p99_ms,
        cores_per_instance: cli.cores_per_instance,
        reqs_per_core_per_sec: cli.reqs_per_core_per_sec,
        headroom_pct: cli.headroom_pct,
        cost_per_instance_hr: cli.cost_per_instance_hr,
        replication: cli.replication,
    };

    let result = plan(&inputs);
    let json = serde_json::to_string_pretty(&result)?;
    println!("{json}");
    if !cli.json_only {
        println!();
        print!("{}", render_table(&result));
    }
    Ok(())
}
