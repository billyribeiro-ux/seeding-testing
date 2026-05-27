//! # capacity-planner
//!
//! Back-of-envelope capacity + cost math for a stateless HTTP service.
//!
//! Given daily-active users, requests per user per day, a peak multiplier,
//! a p99 budget, and a measured per-core throughput, the planner returns
//! the number of cores + instances the team should provision (with
//! headroom and HA replicas) and the monthly cost that buys.
//!
//! The math is deliberately simple — it's a *planning* tool, not a
//! capacity guarantee. The point is for an engineer to type their best
//! guesses for DAU + observed p99 + per-core throughput and get a
//! defensible answer they can paste into a design doc, in seconds.
//!
//! ## The model
//!
//! ```text
//! avg_qps              = DAU × reqs_per_user / 86 400
//! peak_qps             = avg_qps × peak_multiplier
//! target_qps           = peak_qps × (1 + headroom_pct/100)
//! cores_needed         = ceil(target_qps / reqs_per_core_per_sec)
//! instances_needed     = ceil(cores_needed / cores_per_instance) × replication
//! monthly_cost_usd     = instances_needed × cost_per_instance_hr × 730
//! p99_health           = ok | tight | over_budget
//! ```
//!
//! `p99_health` compares the *observed* p99 to the budget:
//!
//! - `ok` — observed ≤ 70% of budget (comfortable margin)
//! - `tight` — observed ≤ budget (within budget but no room)
//! - `over_budget` — observed > budget (capacity won't save you; the
//!   service is too slow per request)
//!
//! ## Rounding
//!
//! Every ceiling step always rounds **up**. Under-provisioning a
//! capacity plan is a worse failure mode than over-paying by one
//! instance for a quarter.
//!
//! ## Example
//!
//! ```
//! use capacity_planner::{plan, Inputs};
//!
//! let plan = plan(&Inputs {
//!     dau: 100_000,
//!     requests_per_user: 50,
//!     peak_multiplier: 3.0,
//!     p99_budget_ms: 200,
//!     observed_p99_ms: 80,
//!     cores_per_instance: 4,
//!     reqs_per_core_per_sec: 200,
//!     headroom_pct: 30,
//!     cost_per_instance_hr: 0.05,
//!     replication: 2,
//! });
//! assert!(plan.instances_needed >= 2); // at minimum, replication copies
//! ```

use serde::{Deserialize, Serialize};

/// Hours per month for cost math. 365.25 × 24 / 12 ≈ 730. Industry-
/// standard for hourly-billed VM pricing (AWS, GCP, Fly, etc).
pub const HOURS_PER_MONTH: f64 = 730.0;

/// Health classification for the observed p99 vs the budget. The
/// boundary at 70% is the "comfort" threshold — once you're past 70%
/// of budget at a quiet hour you have no room for a traffic spike
/// or a noisy neighbour.
pub const HEALTHY_FRACTION_OF_BUDGET: f64 = 0.70;

/// Inputs to a capacity plan. All numeric so callers can build them
/// from CLI args, a YAML file, or a UI without dependency on `clap`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Inputs {
    /// Daily active users.
    pub dau: u64,
    /// Average requests per user per day.
    pub requests_per_user: u64,
    /// Peak QPS = average QPS × this multiplier. A flat-traffic
    /// service might use 1.5; a consumer app commonly sees 3–5.
    pub peak_multiplier: f64,
    /// Allowable p99 latency, in milliseconds.
    pub p99_budget_ms: u64,
    /// Observed p99 on a reference machine, in milliseconds.
    pub observed_p99_ms: u64,
    /// CPU cores per VM/container.
    pub cores_per_instance: u32,
    /// Requests-per-second a single core can hold while staying
    /// inside the p99 budget. Measured, not assumed.
    pub reqs_per_core_per_sec: u32,
    /// Spare capacity, percent (e.g. 30 → plan for 1.30× peak).
    pub headroom_pct: u32,
    /// Hourly USD cost per instance.
    pub cost_per_instance_hr: f64,
    /// Replica count for HA (≥ 1). 2 = N+1 redundancy.
    pub replication: u32,
}

/// Output of the planner. `serde::Serialize` so the binary can emit
/// it as JSON without any glue code.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct Plan {
    pub avg_qps: f64,
    pub peak_qps: f64,
    pub target_qps_with_headroom: f64,
    pub cores_needed: u64,
    pub instances_needed: u64,
    pub monthly_cost_usd: f64,
    pub monthly_cost_per_dau_cents: f64,
    pub p99_health: P99Health,
}

/// p99 health vs the budget. Serializes as a lower-case string so
/// the JSON output stays human-readable.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum P99Health {
    /// Observed p99 ≤ 70% of budget.
    Ok,
    /// Observed p99 between 70% and 100% of budget (no margin).
    Tight,
    /// Observed p99 above budget. Adding cores does **not** fix this —
    /// you need to make each request faster.
    OverBudget,
}

/// Compute a capacity plan from a set of inputs. Pure — no I/O — so
/// it's trivial to unit-test.
#[must_use]
pub fn plan(inp: &Inputs) -> Plan {
    let seconds_per_day: f64 = 86_400.0;
    let avg_qps = (inp.dau as f64 * inp.requests_per_user as f64) / seconds_per_day;
    let peak_qps = avg_qps * inp.peak_multiplier;

    // Headroom multiplier — e.g. 30% → 1.30. Negative headroom
    // would be a foot-gun; floor at 0.
    let headroom_mult = 1.0 + (inp.headroom_pct as f64 / 100.0).max(0.0);
    let target_qps_with_headroom = peak_qps * headroom_mult;

    // Cores needed to hold target QPS. Always round up — under-
    // provisioning is the worse failure mode.
    let cores_needed_f = target_qps_with_headroom / inp.reqs_per_core_per_sec.max(1) as f64;
    let cores_needed = cores_needed_f.ceil() as u64;

    // Instances: ceil(cores / cores_per_instance), then multiply by
    // replication for HA copies. `replication` of 2 means "two full
    // copies of the fleet" — a common N+1 pattern.
    let cores_per_instance = u64::from(inp.cores_per_instance.max(1));
    let instances_per_copy = cores_needed.div_ceil(cores_per_instance);
    // Always need at least one instance per replica, even at zero traffic.
    let replication = u64::from(inp.replication.max(1));
    let instances_needed = instances_per_copy.max(1) * replication;

    let monthly_cost_usd =
        instances_needed as f64 * inp.cost_per_instance_hr * HOURS_PER_MONTH;

    // Cost per DAU per month, in cents. Useful for unit-economics
    // sanity checks ("does this fit inside our $5/user/mo plan?").
    let monthly_cost_per_dau_cents = if inp.dau == 0 {
        0.0
    } else {
        (monthly_cost_usd * 100.0) / inp.dau as f64
    };

    let p99_health = classify_p99(inp.observed_p99_ms, inp.p99_budget_ms);

    Plan {
        avg_qps,
        peak_qps,
        target_qps_with_headroom,
        cores_needed,
        instances_needed,
        monthly_cost_usd,
        monthly_cost_per_dau_cents,
        p99_health,
    }
}

/// Bucket the observed p99 into ok / tight / over-budget. Public so
/// tests + integrations can use the same thresholds.
#[must_use]
pub fn classify_p99(observed_ms: u64, budget_ms: u64) -> P99Health {
    if observed_ms > budget_ms {
        P99Health::OverBudget
    } else if (observed_ms as f64) > (budget_ms as f64) * HEALTHY_FRACTION_OF_BUDGET {
        P99Health::Tight
    } else {
        P99Health::Ok
    }
}

/// Render a `Plan` as a small ASCII table. The binary prints this
/// alongside the JSON; the table is the human-friendly half.
#[must_use]
pub fn render_table(plan: &Plan) -> String {
    let health = match plan.p99_health {
        P99Health::Ok => "ok",
        P99Health::Tight => "tight",
        P99Health::OverBudget => "over_budget",
    };
    let mut out = String::new();
    out.push_str("metric                          value\n");
    out.push_str("------------------------------  ---------------\n");
    out.push_str(&format!(
        "avg_qps                         {:>15.2}\n",
        plan.avg_qps
    ));
    out.push_str(&format!(
        "peak_qps                        {:>15.2}\n",
        plan.peak_qps
    ));
    out.push_str(&format!(
        "target_qps_with_headroom        {:>15.2}\n",
        plan.target_qps_with_headroom
    ));
    out.push_str(&format!(
        "cores_needed                    {:>15}\n",
        plan.cores_needed
    ));
    out.push_str(&format!(
        "instances_needed                {:>15}\n",
        plan.instances_needed
    ));
    out.push_str(&format!(
        "monthly_cost_usd                {:>15.2}\n",
        plan.monthly_cost_usd
    ));
    out.push_str(&format!(
        "monthly_cost_per_dau_cents      {:>15.4}\n",
        plan.monthly_cost_per_dau_cents
    ));
    out.push_str(&format!("p99_health                      {health:>15}\n"));
    out
}
