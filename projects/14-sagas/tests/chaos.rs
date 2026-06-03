//! Chaos test for the saga orchestrator.
//!
//! Run 50 `upgrade_subscription` sagas concurrently. For each saga,
//! randomly inject 0+ transient failures at any of the three steps.
//! After every saga has resolved (Ok or Err), assert the orchestrator's
//! central invariant:
//!
//!   **No partial state lands.** For each saga either:
//!     * Saga returned Ok → exactly one live charge AND one active sub
//!       AND one email sent for that user; or
//!     * Saga returned Err → no live charge AND no active sub for that
//!       user (the email may have been sent if step 3 hadn't failed yet;
//!       in this model the welcome email is best-effort).
//!
//! This is the property that justifies sagas at all: if the rollback is
//! wrong, money goes missing or service entitlements get out of sync.
//!
//! Two parallel runs:
//!   1. `chaos_isolated_worlds_each_saga_is_atomic` — every saga in its
//!      own FakeWorld, so each scenario's injected failures only affect
//!      that saga. This is the deterministic invariant check.
//!   2. `chaos_shared_world_concurrency` — every saga shares one
//!      FakeWorld so the Mutex around `FakeWorldInner` is actually
//!      contested. Asserts the *global* invariant: live charges set ==
//!      active subscriptions set.
//!   3. `chaos_refund_log_matches_failed_post_charge_sagas` — proves
//!      compensation actually ran (rather than the charge happening to
//!      not land).

use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;
use sagas::{FakeWorld, SagaContext, upgrade_subscription_saga};

const N_SAGAS: usize = 50;

#[derive(Clone, Copy, Debug)]
struct Scenario {
    user_id: u64,
    charge_failures: u32,
    sub_failures: u32,
    email_failures: u32,
}

fn random_scenarios(seed: u64) -> Vec<Scenario> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..N_SAGAS)
        .map(|i| Scenario {
            user_id: (i as u64) + 1,
            charge_failures: rng.random_range(0..=1),
            sub_failures: rng.random_range(0..=1),
            email_failures: rng.random_range(0..=1),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chaos_isolated_worlds_each_saga_is_atomic() {
    let scenarios = random_scenarios(0xDEAD_BEEF_u64);

    let mut handles = Vec::with_capacity(N_SAGAS);
    for s in scenarios {
        handles.push(tokio::spawn(async move {
            // Each saga gets its own FakeWorld so its injected failures
            // are isolated from every other saga.
            let w = FakeWorld::new();
            w.arm_charge_failures(s.charge_failures);
            w.arm_sub_failures(s.sub_failures);
            w.arm_email_failures(s.email_failures);
            let saga = upgrade_subscription_saga(
                w.clone(),
                s.user_id,
                format!("user{}@test.local", s.user_id),
                "pro",
                9_99,
                format!("idemp-{}", s.user_id),
            );
            let mut ctx = SagaContext::new();
            let res = saga.run(&mut ctx).await;
            (s, res, w)
        }));
    }

    let mut ok_count = 0_usize;
    let mut err_count = 0_usize;
    for h in handles {
        let (s, res, w) = h.await.unwrap();
        if res.is_ok() {
            ok_count += 1;
            assert_eq!(w.live_charges(), 1, "{s:?}: ok must leave one live charge");
            assert_eq!(w.active_subs(), 1, "{s:?}: ok must leave one active sub");
            assert_eq!(w.emails_sent(), 1, "{s:?}: ok must have sent one email");
        } else {
            err_count += 1;
            assert_eq!(
                w.live_charges(),
                0,
                "{s:?}: err must leave zero live charges"
            );
            assert_eq!(w.active_subs(), 0, "{s:?}: err must leave zero active subs");
        }
    }

    // With the seed above the random scenarios produce a mix of Ok and
    // Err; that is the point of the chaos test. If this ever fails,
    // pick a different seed or widen the range — it's a smoke check
    // that the test exercises both branches.
    assert!(
        ok_count > 0 && err_count > 0,
        "chaos test must exercise both Ok and Err paths (ok={ok_count}, err={err_count})",
    );
}

/// Shared `FakeWorld` across 50 concurrent sagas. The injected-failure
/// pools are summed across all scenarios (so any failure injected by
/// any scenario can affect any saga's matching step). This makes the
/// per-saga outcome non-deterministic; what we assert is the
/// post-condition the system reconciliation job would assert nightly:
///
///   { user_id : sub is active }  ==  { user_id : charge is live }
async fn chaos_shared_world_inner() {
    let scenarios = random_scenarios(0x00C0_FFEE_u64);
    let world = FakeWorld::new();
    // Sum every scenario's quota into one shared pool.
    {
        let mut g = world.inner.lock().unwrap();
        for s in &scenarios {
            g.charge_failures_remaining += s.charge_failures;
            g.sub_failures_remaining += s.sub_failures;
            g.email_failures_remaining += s.email_failures;
        }
    }

    let mut handles = Vec::with_capacity(N_SAGAS);
    for s in scenarios {
        let w = world.clone();
        handles.push(tokio::spawn(async move {
            let saga = upgrade_subscription_saga(
                w,
                s.user_id,
                format!("user{}@test.local", s.user_id),
                "pro",
                9_99,
                format!("idemp-{}", s.user_id),
            );
            let mut ctx = SagaContext::new();
            saga.run(&mut ctx).await
        }));
    }
    for h in handles {
        let _ = h.await.unwrap();
    }

    // Global invariant: live charges set ≡ active subs set.
    let inner = world.inner.lock().unwrap();
    let active_user_ids: std::collections::HashSet<u64> = inner
        .subscriptions
        .values()
        .filter(|s| s.active)
        .map(|s| s.user_id)
        .collect();
    let charged_user_ids: std::collections::HashSet<u64> = inner
        .charges
        .iter()
        .filter(|(_, c)| !c.refunded)
        .filter_map(|(id, _)| id.strip_prefix("ch_idemp-").and_then(|s| s.parse().ok()))
        .collect();
    assert_eq!(
        active_user_ids, charged_user_ids,
        "global invariant: active subs == live charges",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chaos_shared_world_concurrency() {
    chaos_shared_world_inner().await;
}

/// Specifically prove: a refund is recorded for every saga that
/// charged-then-failed-later. (Compensation actually ran, not just
/// "live_charges() happens to be 0 because the charge never landed.")
#[tokio::test]
async fn chaos_refund_log_matches_failed_post_charge_sagas() {
    let w = FakeWorld::new();
    let mut count_should_refund = 0_usize;
    for i in 0..20_u64 {
        w.arm_email_failures(1); // force step 3 to fail
        count_should_refund += 1;
        let saga = upgrade_subscription_saga(
            w.clone(),
            100 + i,
            format!("u{i}@t.local"),
            "pro",
            5_00,
            format!("idemp-r{i}"),
        );
        let mut ctx = SagaContext::new();
        let _ = saga.run(&mut ctx).await;
    }
    let inner = w.inner.lock().unwrap();
    assert_eq!(
        inner.refunds.len(),
        count_should_refund,
        "every failed-after-charge saga must produce one refund row",
    );
}
