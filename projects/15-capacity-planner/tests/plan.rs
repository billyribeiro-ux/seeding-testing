//! Integration tests for the capacity-planner library.
//!
//! Each test pairs a "back-of-envelope" scenario with the answer an
//! engineer should get if they did the math on a whiteboard. Goal:
//! catch any silent regression in the formulas (rounding the wrong
//! way, dropping the peak multiplier, etc.).

use capacity_planner::{Inputs, P99Health, classify_p99, plan};

/// A reasonable defaults builder; tests override the fields they
/// care about. Keeps every test under ten lines.
fn base() -> Inputs {
    Inputs {
        dau: 100_000,
        requests_per_user: 50,
        peak_multiplier: 3.0,
        p99_budget_ms: 200,
        observed_p99_ms: 80,
        cores_per_instance: 4,
        reqs_per_core_per_sec: 200,
        headroom_pct: 30,
        cost_per_instance_hr: 0.05,
        replication: 2,
    }
}

#[test]
fn small_app_one_thousand_dau_fits_on_minimum_fleet() {
    // 1 000 DAU × 50 reqs/day = 50 000 reqs/day = 0.58 QPS avg.
    // Peak × 3 = 1.74 QPS — comfortably under one core. We should
    // still end up with the HA minimum (one instance per replica).
    let inputs = Inputs {
        dau: 1_000,
        ..base()
    };
    let p = plan(&inputs);
    assert!(p.avg_qps < 1.0, "avg_qps={}", p.avg_qps);
    assert!(p.peak_qps < 5.0);
    assert_eq!(
        p.cores_needed, 1,
        "even tiny apps need at least one core after ceil"
    );
    // 2 replicas × 1 instance/copy = 2 instances minimum.
    assert_eq!(p.instances_needed, 2);
}

#[test]
fn mid_size_one_hundred_thousand_dau_baseline() {
    // 100 000 DAU × 50 = 5 000 000 reqs/day → ~57.87 avg QPS.
    // Peak × 3 = ~173.6 QPS. With 30% headroom → ~225.7 target QPS.
    // At 200 RPS/core → 2 cores (ceil(225.7/200) = 2).
    // 2 cores / 4 per instance → 1 instance × 2 replicas = 2.
    let p = plan(&base());
    assert!((p.avg_qps - 57.87).abs() < 0.5, "avg_qps={}", p.avg_qps);
    assert!((p.peak_qps - 173.6).abs() < 1.0, "peak_qps={}", p.peak_qps);
    assert!(
        (p.target_qps_with_headroom - 225.7).abs() < 1.0,
        "target_qps={}",
        p.target_qps_with_headroom
    );
    assert_eq!(p.cores_needed, 2);
    assert_eq!(p.instances_needed, 2);
}

#[test]
fn large_app_ten_million_dau_scales_to_many_instances() {
    // 10M DAU × 50 = 500M reqs/day → ~5 787 avg QPS.
    // Peak × 3 = ~17 361 QPS. With 30% headroom → ~22 569.
    // At 200 RPS/core → ceil(22569/200) = 113 cores.
    // 113 cores / 4 per instance = ceil(113/4) = 29 instances per copy.
    // × 2 replicas = 58 instances.
    let inputs = Inputs {
        dau: 10_000_000,
        ..base()
    };
    let p = plan(&inputs);
    assert_eq!(p.cores_needed, 113);
    assert_eq!(p.instances_needed, 58);
    // Cost: 58 × $0.05/hr × 730 hr/mo = $2 117.
    assert!(
        (p.monthly_cost_usd - 2_117.0).abs() < 1.0,
        "monthly_cost_usd={}",
        p.monthly_cost_usd
    );
    // Per DAU: $2117 × 100 / 10M = 0.02117 cents/DAU/mo.
    assert!(p.monthly_cost_per_dau_cents < 0.05);
}

#[test]
fn peak_multiplier_is_actually_applied() {
    // Two identical plans except for peak_multiplier — the one with
    // 5× should land on strictly more (or equal) cores/instances.
    let low = plan(&Inputs {
        dau: 500_000,
        peak_multiplier: 1.0,
        ..base()
    });
    let high = plan(&Inputs {
        dau: 500_000,
        peak_multiplier: 5.0,
        ..base()
    });
    assert!(high.peak_qps > low.peak_qps * 4.0);
    assert!(high.cores_needed > low.cores_needed);
    assert!(high.instances_needed >= low.instances_needed);
}

#[test]
fn headroom_is_enforced() {
    // With 0% headroom we need fewer cores than with 100% headroom.
    // Specifically: target_qps_with_headroom = peak_qps × (1 + H/100).
    let none = plan(&Inputs {
        headroom_pct: 0,
        ..base()
    });
    let lots = plan(&Inputs {
        headroom_pct: 100,
        ..base()
    });
    assert!(
        (none.target_qps_with_headroom - none.peak_qps).abs() < 1e-6,
        "headroom 0 should equal peak; got target={} peak={}",
        none.target_qps_with_headroom,
        none.peak_qps
    );
    assert!(
        (lots.target_qps_with_headroom - none.peak_qps * 2.0).abs() < 1e-6,
        "headroom 100 should double target; got target={}",
        lots.target_qps_with_headroom
    );
    assert!(lots.cores_needed >= none.cores_needed);
}

#[test]
fn p99_health_three_classes() {
    // Budget 200ms; under 70% (=140ms) → ok; between 140 and 200 → tight;
    // above 200 → over_budget.
    assert_eq!(classify_p99(50, 200), P99Health::Ok);
    assert_eq!(classify_p99(140, 200), P99Health::Ok);
    assert_eq!(classify_p99(160, 200), P99Health::Tight);
    assert_eq!(classify_p99(200, 200), P99Health::Tight);
    assert_eq!(classify_p99(201, 200), P99Health::OverBudget);
    assert_eq!(classify_p99(2_000, 200), P99Health::OverBudget);
}

#[test]
fn p99_over_budget_shows_up_in_plan() {
    // The "over budget" signal must propagate from the inputs to
    // the plan output, regardless of how many cores we end up with.
    let over = plan(&Inputs {
        observed_p99_ms: 350,
        p99_budget_ms: 200,
        ..base()
    });
    assert_eq!(over.p99_health, P99Health::OverBudget);
    // Capacity alone doesn't rescue a slow request path — but the
    // plan still emits a number; it's the engineer's job to read
    // the health field, not assume cores fix everything.
    assert!(over.instances_needed > 0);
}

#[test]
fn rounding_always_rounds_up_so_we_never_underprovision() {
    // Craft a scenario where the math comes out to a fractional core
    // (e.g. 2.01 cores) and confirm we round up to 3, not down to 2.
    // 100k DAU × 50 = 57.87 avg QPS; peak × 3 = 173.6; headroom 30%
    // = 225.7 target. At 100 RPS/core → 2.257 → must ceil to 3.
    let p = plan(&Inputs {
        reqs_per_core_per_sec: 100,
        ..base()
    });
    assert_eq!(p.cores_needed, 3, "fractional cores must round up");

    // Same idea for instances: 5 cores ÷ 4 per instance = 1.25 → 2.
    let p2 = plan(&Inputs {
        dau: 500_000, // bigger -> forces > 4 cores
        ..base()
    });
    let cores_per_instance: u64 = 4;
    let expected_per_copy = p2.cores_needed.div_ceil(cores_per_instance);
    assert_eq!(
        p2.instances_needed,
        expected_per_copy * u64::from(base().replication),
    );
}

#[test]
fn replication_multiplies_instance_count() {
    let one = plan(&Inputs {
        replication: 1,
        dau: 1_000_000,
        ..base()
    });
    let three = plan(&Inputs {
        replication: 3,
        dau: 1_000_000,
        ..base()
    });
    assert_eq!(three.instances_needed, one.instances_needed * 3);
    assert!(
        (three.monthly_cost_usd - one.monthly_cost_usd * 3.0).abs() < 1e-6,
        "tripling replicas should triple cost",
    );
}

#[test]
fn zero_dau_is_handled_gracefully() {
    // Don't divide by zero on the cost-per-DAU calc, even at zero
    // traffic. Still provisions one instance per replica.
    let p = plan(&Inputs { dau: 0, ..base() });
    assert_eq!(p.avg_qps, 0.0);
    assert_eq!(p.peak_qps, 0.0);
    assert_eq!(p.monthly_cost_per_dau_cents, 0.0);
    assert!(p.instances_needed >= u64::from(base().replication));
}
