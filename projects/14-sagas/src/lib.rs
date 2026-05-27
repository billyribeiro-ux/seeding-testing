//! sagas — a tiny saga orchestrator.
//!
//! A *saga* is a sequence of local transactions in different stores (a
//! Stripe charge, a Postgres row, an email send) glued together with
//! *compensations*. There is no global transaction. When step *k*
//! fails, the orchestrator runs the compensation of every earlier step
//! in reverse (LIFO) order; the effect is that the system ends up in a
//! semantically-undone state even though no single transaction crossed
//! all three stores.
//!
//! The shape here is the simple, readable one:
//!
//! ```text
//! trait Step {
//!     async fn execute(&self, ctx: &mut SagaContext) -> Result<(), StepError>;
//!     async fn compensate(&self, ctx: &mut SagaContext) -> Result<(), StepError>;
//! }
//!
//! let mut saga = Saga::new("upgrade_subscription");
//! saga.add_step(ChargeCard::new(...));
//! saga.add_step(UpdateSubscriptionRow::new(...));
//! saga.add_step(SendWelcomeEmail::new(...));
//! saga.run(&mut ctx).await?;
//! ```
//!
//! We use `Box<dyn Step>` rather than generics. The whole orchestrator
//! fits on one screen; you can read it once and remember it.
//!
//! ## Trade-offs (documented loudly because they are real)
//!
//! 1. **Semantic vs syntactic compensation.** A "refund" is not the
//!    bit-for-bit inverse of a "charge"; it leaves a charge AND a refund
//!    on the customer's statement. That is correct — sagas restore
//!    *business* invariants, not byte-equality. Pick compensations the
//!    business is happy to explain.
//!
//! 2. **What if a compensation itself fails?** This is the real-world
//!    nightmare. Three options, all worse than 2PC:
//!    * **Retry with backoff** (this orchestrator does it: see
//!      `CompensationPolicy`). Idempotent compensations make this safe.
//!    * **Park** the saga in a "needs human" state and page on-call.
//!    * **Persist** the saga so a worker can pick it up after a crash
//!      (we do not persist here; the design is sketched in lesson
//!      `docs/distributed-systems/03-sagas-vs-2pc.md`).
//!
//! 3. **Compensations must be idempotent.** A retried "refund" should
//!    not double-refund. Use idempotency keys; see
//!    `apps/memberclub/api/src/billing.rs` for the production pattern.
//!
//! 4. **No isolation.** Mid-saga, other readers see partial state. If
//!    that is unacceptable, you need 2PC (rare) or to model the saga
//!    state in your reads ("pending upgrade").
//!
//! ## Why not 2PC?
//!
//! Two-phase commit needs every participant to support a `PREPARE`
//! verb and to honor it indefinitely. Stripe and SendGrid do not.
//! See `docs/distributed-systems/03-sagas-vs-2pc.md`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SagaError {
    /// A `Step::execute` returned an error; we attempted compensations of
    /// the prior successful steps. The first field is the original
    /// failure; the second is the list of compensation results (in the
    /// order they ran — i.e. LIFO from the failing step).
    #[error(
        "saga `{saga}` failed at step `{step}` ({step_index}); ran {compensations_run} compensation(s)"
    )]
    StepFailed {
        saga: String,
        step: String,
        step_index: usize,
        source: StepError,
        compensations_run: usize,
        /// Did every compensation succeed? If `false`, the system is
        /// in a partially-compensated state and needs human attention.
        compensations_clean: bool,
    },
}

#[derive(Debug, Error)]
pub enum StepError {
    /// Retryable — transient. The orchestrator may retry depending on
    /// policy. For `execute` we never retry automatically (that is a
    /// step-level concern); for `compensate` we do (see policy).
    #[error("transient: {0}")]
    Transient(String),
    /// Permanent — never retry; escalate.
    #[error("permanent: {0}")]
    Permanent(String),
}

impl StepError {
    pub fn transient(msg: impl Into<String>) -> Self {
        Self::Transient(msg.into())
    }
    pub fn permanent(msg: impl Into<String>) -> Self {
        Self::Permanent(msg.into())
    }

    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_))
    }
}

// ---------------------------------------------------------------------------
// SagaContext — a typed key/value bag that flows through every step.
// ---------------------------------------------------------------------------

/// Shared mutable scratch space for a single saga run.
///
/// We use `String -> String` for the demo. Production sagas often hand
/// down a typed struct instead; the choice does not affect the
/// orchestrator. The values are JSON-y on purpose so any step (whether
/// it talks to Stripe, Postgres, or SendGrid) can read what earlier
/// steps wrote.
#[derive(Debug, Default, Clone)]
pub struct SagaContext {
    data: HashMap<String, String>,
}

impl SagaContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Step trait
// ---------------------------------------------------------------------------

/// The unit of work a saga executes.
///
/// `execute` runs the *forward* action and may write to `ctx`. If the
/// step succeeds, the orchestrator remembers it so a later failure can
/// call `compensate` to undo it.
///
/// `compensate` undoes the forward action. It must be idempotent:
/// retries are normal, and a crash + restart should be safe.
#[async_trait]
pub trait Step: Send + Sync {
    /// A short, stable identifier for logs and errors (e.g.
    /// `"charge_card"`). Use snake_case.
    fn name(&self) -> &'static str;

    /// Perform the forward action. Mutate `ctx` to pass results to
    /// later steps (e.g. write the resulting `charge_id`).
    async fn execute(&self, ctx: &mut SagaContext) -> Result<(), StepError>;

    /// Undo the forward action. Called only if `execute` previously
    /// returned `Ok`. Must be idempotent; the orchestrator may retry
    /// transient failures (see `CompensationPolicy`).
    async fn compensate(&self, ctx: &mut SagaContext) -> Result<(), StepError>;
}

// ---------------------------------------------------------------------------
// CompensationPolicy
// ---------------------------------------------------------------------------

/// How the orchestrator handles compensation failures.
///
/// In production you usually want `RetryWithBackoff` with a small cap,
/// then escalate. We default to a 3-attempt retry with 10 ms backoff
/// (kept short so tests stay fast).
#[derive(Debug, Clone, Copy)]
pub struct CompensationPolicy {
    pub max_attempts: u32,
    pub backoff: Duration,
}

impl Default for CompensationPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: Duration::from_millis(10),
        }
    }
}

// ---------------------------------------------------------------------------
// Saga
// ---------------------------------------------------------------------------

/// A saga is a named, ordered list of steps with a compensation policy.
pub struct Saga {
    name: String,
    steps: Vec<Box<dyn Step>>,
    policy: CompensationPolicy,
}

impl Saga {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            policy: CompensationPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: CompensationPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn add_step<S: Step + 'static>(&mut self, step: S) -> &mut Self {
        self.steps.push(Box::new(step));
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Run the saga. On any `execute` failure, the prior successful
    /// steps' `compensate` methods are called in **reverse** (LIFO)
    /// order. Compensation errors are retried per `policy`.
    ///
    /// # Errors
    /// Returns [`SagaError::StepFailed`] if any `execute` returns an
    /// error. The `compensations_clean` flag tells you whether the
    /// rollback completed; if `false`, the system is in a half-undone
    /// state and a human needs to look at it.
    pub async fn run(&self, ctx: &mut SagaContext) -> Result<(), SagaError> {
        info!(saga = %self.name, steps = self.steps.len(), "saga.start");

        for (idx, step) in self.steps.iter().enumerate() {
            debug!(saga = %self.name, step = step.name(), idx, "saga.step.start");
            match step.execute(ctx).await {
                Ok(()) => {
                    debug!(saga = %self.name, step = step.name(), idx, "saga.step.ok");
                }
                Err(e) => {
                    warn!(
                        saga = %self.name,
                        step = step.name(),
                        idx,
                        error = %e,
                        "saga.step.failed; beginning compensation",
                    );
                    let (ran, clean) = self.compensate_through(idx, ctx).await;
                    return Err(SagaError::StepFailed {
                        saga: self.name.clone(),
                        step: step.name().to_string(),
                        step_index: idx,
                        source: e,
                        compensations_run: ran,
                        compensations_clean: clean,
                    });
                }
            }
        }

        info!(saga = %self.name, "saga.commit");
        Ok(())
    }

    /// Compensate every step strictly before `failed_idx`, in reverse.
    ///
    /// Returns `(compensations_run, clean)`. `clean` is `true` iff every
    /// compensation eventually succeeded. We *do not* stop on a
    /// compensation failure — partial undo is better than no undo.
    async fn compensate_through(&self, failed_idx: usize, ctx: &mut SagaContext) -> (usize, bool) {
        let mut clean = true;
        let mut ran = 0_usize;
        for i in (0..failed_idx).rev() {
            let step = &self.steps[i];
            debug!(saga = %self.name, step = step.name(), idx = i, "saga.compensate.start");
            let ok = self.try_compensate(step.as_ref(), ctx).await;
            ran += 1;
            if ok {
                debug!(saga = %self.name, step = step.name(), idx = i, "saga.compensate.ok");
            } else {
                error!(
                    saga = %self.name,
                    step = step.name(),
                    idx = i,
                    "saga.compensate.giveup",
                );
                clean = false;
            }
        }
        (ran, clean)
    }

    async fn try_compensate(&self, step: &dyn Step, ctx: &mut SagaContext) -> bool {
        let mut last_err: Option<StepError> = None;
        for attempt in 1..=self.policy.max_attempts {
            match step.compensate(ctx).await {
                Ok(()) => return true,
                Err(e) => {
                    let transient = e.is_transient();
                    warn!(
                        step = step.name(),
                        attempt,
                        max = self.policy.max_attempts,
                        error = %e,
                        "saga.compensate.error",
                    );
                    last_err = Some(e);
                    if !transient {
                        return false; // permanent error → no point retrying
                    }
                    if attempt < self.policy.max_attempts {
                        tokio::time::sleep(self.policy.backoff).await;
                    }
                }
            }
        }
        debug!(step = step.name(), ?last_err, "saga.compensate.exhausted");
        false
    }
}

// ===========================================================================
// Worked example: MemberClub `upgrade_subscription`.
//
// The three local transactions:
//   1. Charge the customer's card via Stripe.
//   2. Update the local `subscriptions` row.
//   3. Send a welcome email.
//
// Each step writes its result into the `SagaContext` so compensations
// can find what to undo. The example uses an in-process `FakeWorld` so
// the tests don't need a real Stripe/DB/SMTP. The same shape works
// against real services; only the I/O bodies change.
// ===========================================================================

/// In-process stand-in for "the world" — a fake Stripe API + fake DB +
/// fake email outbox. We track every effect so tests can assert that
/// compensations actually undid them.
#[derive(Debug, Default)]
pub struct FakeWorldInner {
    pub charges: HashMap<String, ChargeRecord>,
    pub refunds: Vec<String>,
    pub subscriptions: HashMap<u64, SubRecord>,
    pub emails_sent: Vec<String>,
    pub email_failures_remaining: u32,
    pub charge_failures_remaining: u32,
    pub sub_failures_remaining: u32,
}

#[derive(Debug, Clone)]
pub struct ChargeRecord {
    pub amount_cents: i64,
    pub refunded: bool,
}

#[derive(Debug, Clone)]
pub struct SubRecord {
    pub user_id: u64,
    pub plan: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FakeWorld {
    pub inner: Arc<Mutex<FakeWorldInner>>,
}

impl FakeWorld {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience: how many charges are still live (not refunded)?
    pub fn live_charges(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .charges
            .values()
            .filter(|c| !c.refunded)
            .count()
    }

    /// Convenience: how many active subscription rows?
    pub fn active_subs(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .subscriptions
            .values()
            .filter(|s| s.active)
            .count()
    }

    pub fn emails_sent(&self) -> usize {
        self.inner.lock().unwrap().emails_sent.len()
    }

    pub fn arm_email_failures(&self, n: u32) {
        self.inner.lock().unwrap().email_failures_remaining = n;
    }

    pub fn arm_charge_failures(&self, n: u32) {
        self.inner.lock().unwrap().charge_failures_remaining = n;
    }

    pub fn arm_sub_failures(&self, n: u32) {
        self.inner.lock().unwrap().sub_failures_remaining = n;
    }
}

// --- Step 1: ChargeCard ----------------------------------------------------

/// Charge `amount_cents` to `user_id`. Mirrors the i64-cents convention
/// from `projects/06-stripe-money-lab`.
pub struct ChargeCard {
    pub world: FakeWorld,
    pub user_id: u64,
    pub amount_cents: i64,
    /// Idempotency key — the same value must reach `compensate` so a
    /// refund retry does not double-refund. We synthesize a charge id
    /// from this in `execute` and stash it on `ctx`.
    pub idempotency_key: String,
}

#[async_trait]
impl Step for ChargeCard {
    fn name(&self) -> &'static str {
        "charge_card"
    }

    async fn execute(&self, ctx: &mut SagaContext) -> Result<(), StepError> {
        if self.amount_cents <= 0 {
            return Err(StepError::permanent("amount must be positive"));
        }
        let mut g = self.world.inner.lock().unwrap();
        if g.charge_failures_remaining > 0 {
            g.charge_failures_remaining -= 1;
            return Err(StepError::transient("stripe declined (injected)"));
        }
        let charge_id = format!("ch_{}", self.idempotency_key);
        g.charges.insert(
            charge_id.clone(),
            ChargeRecord {
                amount_cents: self.amount_cents,
                refunded: false,
            },
        );
        drop(g);
        ctx.set("charge_id", charge_id);
        Ok(())
    }

    async fn compensate(&self, ctx: &mut SagaContext) -> Result<(), StepError> {
        let Some(charge_id) = ctx.get("charge_id").map(ToString::to_string) else {
            // No charge_id means the forward action did not commit; nothing to undo.
            return Ok(());
        };
        let mut g = self.world.inner.lock().unwrap();
        match g.charges.get_mut(&charge_id) {
            Some(c) if !c.refunded => {
                c.refunded = true;
                g.refunds.push(charge_id);
                Ok(())
            }
            // Already refunded, or charge id not found — idempotent no-op.
            _ => Ok(()),
        }
    }
}

// --- Step 2: UpdateSubscriptionRow -----------------------------------------

pub struct UpdateSubscriptionRow {
    pub world: FakeWorld,
    pub user_id: u64,
    pub plan: String,
}

#[async_trait]
impl Step for UpdateSubscriptionRow {
    fn name(&self) -> &'static str {
        "update_subscription_row"
    }

    async fn execute(&self, ctx: &mut SagaContext) -> Result<(), StepError> {
        let mut g = self.world.inner.lock().unwrap();
        if g.sub_failures_remaining > 0 {
            g.sub_failures_remaining -= 1;
            return Err(StepError::transient("db deadlock (injected)"));
        }
        g.subscriptions.insert(
            self.user_id,
            SubRecord {
                user_id: self.user_id,
                plan: self.plan.clone(),
                active: true,
            },
        );
        drop(g);
        ctx.set("subscription_user_id", self.user_id.to_string());
        ctx.set("subscription_plan", self.plan.clone());
        Ok(())
    }

    async fn compensate(&self, _ctx: &mut SagaContext) -> Result<(), StepError> {
        let mut g = self.world.inner.lock().unwrap();
        if let Some(s) = g.subscriptions.get_mut(&self.user_id) {
            s.active = false;
        }
        Ok(())
    }
}

// --- Step 3: SendWelcomeEmail ----------------------------------------------

/// Send a welcome email. Sometimes flaky — that's the point of the
/// example: it's the most-likely-to-fail step *and* the step nobody
/// wants to compensate (you cannot un-send an email, only send a
/// follow-up). In our model we treat the welcome email as "best effort"
/// — its compensation is a no-op, but the SAGA still rolls back the
/// preceding steps if it fails.
pub struct SendWelcomeEmail {
    pub world: FakeWorld,
    pub user_email: String,
}

#[async_trait]
impl Step for SendWelcomeEmail {
    fn name(&self) -> &'static str {
        "send_welcome_email"
    }

    async fn execute(&self, _ctx: &mut SagaContext) -> Result<(), StepError> {
        let mut g = self.world.inner.lock().unwrap();
        if g.email_failures_remaining > 0 {
            g.email_failures_remaining -= 1;
            return Err(StepError::transient("smtp 5xx (injected)"));
        }
        g.emails_sent.push(self.user_email.clone());
        Ok(())
    }

    async fn compensate(&self, _ctx: &mut SagaContext) -> Result<(), StepError> {
        // Can't un-send an email. In production you'd send a corrective
        // "we got ahead of ourselves" follow-up. The orchestrator does
        // not need to know that — the compensation just has to succeed.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Convenience constructor for the worked example.
// ---------------------------------------------------------------------------

/// Build the `upgrade_subscription` saga used by tests and docs.
#[must_use]
pub fn upgrade_subscription_saga(
    world: FakeWorld,
    user_id: u64,
    user_email: impl Into<String>,
    plan: impl Into<String>,
    amount_cents: i64,
    idempotency_key: impl Into<String>,
) -> Saga {
    let mut saga = Saga::new("upgrade_subscription");
    saga.add_step(ChargeCard {
        world: world.clone(),
        user_id,
        amount_cents,
        idempotency_key: idempotency_key.into(),
    });
    saga.add_step(UpdateSubscriptionRow {
        world: world.clone(),
        user_id,
        plan: plan.into(),
    });
    saga.add_step(SendWelcomeEmail {
        world,
        user_email: user_email.into(),
    });
    saga
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> FakeWorld {
        FakeWorld::new()
    }

    #[tokio::test]
    async fn happy_path_commits_all_three_steps() {
        let w = world();
        let saga = upgrade_subscription_saga(w.clone(), 1, "a@b.test", "pro", 9_99, "idemp-1");
        let mut ctx = SagaContext::new();
        saga.run(&mut ctx).await.unwrap();

        assert_eq!(w.live_charges(), 1, "one live charge");
        assert_eq!(w.active_subs(), 1, "one active subscription");
        assert_eq!(w.emails_sent(), 1, "one welcome email sent");
        // Context propagated as expected.
        assert!(ctx.get("charge_id").unwrap().starts_with("ch_"));
        assert_eq!(ctx.get("subscription_plan"), Some("pro"));
    }

    #[tokio::test]
    async fn failure_on_step_3_undoes_steps_2_then_1() {
        let w = world();
        // Fail the email step permanently (1 injected failure is enough
        // because email's execute is not retried by the saga).
        w.arm_email_failures(1);

        let saga = upgrade_subscription_saga(w.clone(), 7, "x@y.test", "pro", 19_99, "idemp-7");
        let mut ctx = SagaContext::new();
        let err = saga.run(&mut ctx).await.unwrap_err();

        match err {
            SagaError::StepFailed {
                step,
                step_index,
                compensations_run,
                compensations_clean,
                ..
            } => {
                assert_eq!(step, "send_welcome_email");
                assert_eq!(step_index, 2);
                assert_eq!(compensations_run, 2, "compensate steps 1 and 0");
                assert!(compensations_clean, "all compensations succeeded");
            }
        }

        assert_eq!(w.live_charges(), 0, "charge refunded");
        assert_eq!(w.active_subs(), 0, "subscription deactivated");
        assert_eq!(w.emails_sent(), 0, "no email sent");
    }

    #[tokio::test]
    async fn failure_on_step_2_only_undoes_step_1() {
        let w = world();
        w.arm_sub_failures(1);

        let saga = upgrade_subscription_saga(w.clone(), 9, "z@y.test", "pro", 4_99, "idemp-9");
        let mut ctx = SagaContext::new();
        let err = saga.run(&mut ctx).await.unwrap_err();

        let SagaError::StepFailed {
            step,
            step_index,
            compensations_run,
            ..
        } = err;
        assert_eq!(step, "update_subscription_row");
        assert_eq!(step_index, 1);
        assert_eq!(compensations_run, 1, "only step 0 needed compensating");

        assert_eq!(w.live_charges(), 0);
        // Subscription was never inserted, so active_subs is 0.
        assert_eq!(w.active_subs(), 0);
    }

    #[tokio::test]
    async fn failure_on_step_1_runs_zero_compensations() {
        let w = world();
        w.arm_charge_failures(1);

        let saga = upgrade_subscription_saga(w.clone(), 11, "a@b.test", "pro", 1_00, "idemp-11");
        let mut ctx = SagaContext::new();
        let err = saga.run(&mut ctx).await.unwrap_err();

        let SagaError::StepFailed {
            step,
            step_index,
            compensations_run,
            ..
        } = err;
        assert_eq!(step, "charge_card");
        assert_eq!(step_index, 0);
        assert_eq!(compensations_run, 0, "no prior steps to compensate");
    }

    /// Verify LIFO ordering of compensations using a custom step that
    /// records its own undo into a shared log.
    #[tokio::test]
    async fn compensations_run_in_lifo_order() {
        use std::sync::Mutex;

        struct Recorder {
            id: &'static str,
            log: Arc<Mutex<Vec<&'static str>>>,
            fail_execute: bool,
        }

        #[async_trait]
        impl Step for Recorder {
            fn name(&self) -> &'static str {
                self.id
            }
            async fn execute(&self, _: &mut SagaContext) -> Result<(), StepError> {
                if self.fail_execute {
                    Err(StepError::permanent("boom"))
                } else {
                    Ok(())
                }
            }
            async fn compensate(&self, _: &mut SagaContext) -> Result<(), StepError> {
                self.log.lock().unwrap().push(self.id);
                Ok(())
            }
        }

        let log = Arc::new(Mutex::new(Vec::new()));
        let mut saga = Saga::new("lifo");
        saga.add_step(Recorder {
            id: "A",
            log: log.clone(),
            fail_execute: false,
        });
        saga.add_step(Recorder {
            id: "B",
            log: log.clone(),
            fail_execute: false,
        });
        saga.add_step(Recorder {
            id: "C",
            log: log.clone(),
            fail_execute: false,
        });
        saga.add_step(Recorder {
            id: "D",
            log: log.clone(),
            fail_execute: true,
        });

        let _ = saga.run(&mut SagaContext::new()).await.unwrap_err();
        let got = log.lock().unwrap().clone();
        assert_eq!(got, vec!["C", "B", "A"], "LIFO from failing step");
    }

    /// A compensation that fails twice then succeeds — we retry.
    #[tokio::test]
    async fn compensation_retries_transient_failures() {
        struct FlakeyComp {
            id: &'static str,
            remaining: Arc<Mutex<u32>>,
        }

        #[async_trait]
        impl Step for FlakeyComp {
            fn name(&self) -> &'static str {
                self.id
            }
            async fn execute(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Ok(())
            }
            async fn compensate(&self, _: &mut SagaContext) -> Result<(), StepError> {
                let mut g = self.remaining.lock().unwrap();
                if *g > 0 {
                    *g -= 1;
                    Err(StepError::transient("flake"))
                } else {
                    Ok(())
                }
            }
        }

        struct Failer;
        #[async_trait]
        impl Step for Failer {
            fn name(&self) -> &'static str {
                "failer"
            }
            async fn execute(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Err(StepError::permanent("nope"))
            }
            async fn compensate(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Ok(())
            }
        }

        let remaining = Arc::new(Mutex::new(2));
        let mut saga = Saga::new("retry-comp");
        saga.add_step(FlakeyComp {
            id: "flakey",
            remaining: remaining.clone(),
        });
        saga.add_step(Failer);

        let err = saga.run(&mut SagaContext::new()).await.unwrap_err();
        let SagaError::StepFailed {
            compensations_run,
            compensations_clean,
            ..
        } = err;
        assert_eq!(compensations_run, 1);
        assert!(
            compensations_clean,
            "transient failures eventually succeeded"
        );
    }

    /// Permanent compensation failure → `compensations_clean = false`,
    /// no retry storm.
    #[tokio::test]
    async fn permanent_compensation_failure_is_not_retried() {
        struct PermFail {
            calls: Arc<Mutex<u32>>,
        }

        #[async_trait]
        impl Step for PermFail {
            fn name(&self) -> &'static str {
                "perm_fail"
            }
            async fn execute(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Ok(())
            }
            async fn compensate(&self, _: &mut SagaContext) -> Result<(), StepError> {
                *self.calls.lock().unwrap() += 1;
                Err(StepError::permanent("data gone"))
            }
        }

        struct Failer;
        #[async_trait]
        impl Step for Failer {
            fn name(&self) -> &'static str {
                "failer"
            }
            async fn execute(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Err(StepError::permanent("nope"))
            }
            async fn compensate(&self, _: &mut SagaContext) -> Result<(), StepError> {
                Ok(())
            }
        }

        let calls = Arc::new(Mutex::new(0));
        let mut saga = Saga::new("perm");
        saga.add_step(PermFail {
            calls: calls.clone(),
        });
        saga.add_step(Failer);
        let err = saga.run(&mut SagaContext::new()).await.unwrap_err();
        let SagaError::StepFailed {
            compensations_clean,
            ..
        } = err;
        assert!(!compensations_clean);
        assert_eq!(*calls.lock().unwrap(), 1, "no retries on permanent");
    }

    #[tokio::test]
    async fn empty_saga_is_a_no_op() {
        let saga = Saga::new("empty");
        let mut ctx = SagaContext::new();
        saga.run(&mut ctx).await.unwrap();
        assert_eq!(saga.step_count(), 0);
    }

    #[tokio::test]
    async fn compensation_is_idempotent_on_charge() {
        // Calling compensate twice should not produce two refunds.
        let w = world();
        let step = ChargeCard {
            world: w.clone(),
            user_id: 1,
            amount_cents: 100,
            idempotency_key: "idemp-x".into(),
        };
        let mut ctx = SagaContext::new();
        step.execute(&mut ctx).await.unwrap();
        step.compensate(&mut ctx).await.unwrap();
        step.compensate(&mut ctx).await.unwrap();
        assert_eq!(w.inner.lock().unwrap().refunds.len(), 1);
    }
}
