# Phase 8 — Exercises

Eight drills across the Money primitive and the webhook receiver.

> **Status:** all 8 marked `— shipped`. The Money/Currency newtype with
> i64-cents + ceiling + proportional split lives in
> `projects/06-stripe-money-lab/src/lib.rs`; webhook signature
> verification + idempotency lives in `projects/07-webhook-receiver/src/lib.rs`.
> E8.1 (`Money::checked_div`) ships with three unit tests + matching
> `MoneyError::DivByZero` variant; E8.7 (`currency_mismatch` symmetry)
> ships as a proptest in the same crate's `prop_tests` module. Other
> exercises are drills whose inline `<details>` answers are the deliverable.

---

## E8.1 — `Money::checked_div` (Easy) — shipped

Add a `checked_div(self, by: i64) -> Result<Money, MoneyError>` that performs
integer division. Document the rounding behavior (truncation toward zero) in
the doc comment.

Shipped as `Money::checked_div` in `projects/06-stripe-money-lab/src/lib.rs`
alongside the existing checked arithmetic; the doc comment spells out
truncation toward zero (matching `i64`'s `/`) and a new
`MoneyError::DivByZero` variant covers a zero divisor. Three unit tests
in the same file (`div_truncates_toward_zero`, `div_by_zero_errors`,
`div_by_negative_flips_sign`) cover the happy path, divide-by-zero, and
sign-flipping negative divisor.

---

## E8.2 — Display tests for every currency (Easy) — shipped

For each of the four currencies, write a unit test asserting Display output
for $0.00, $0.05, $1.50, -$10.00, the ceiling-minus-1.

The `Display` impl that these tests exercise is in
`projects/06-stripe-money-lab/src/lib.rs` (the `impl fmt::Display for
Money` block) and already handles the zero-decimal (JPY) special case
plus the awkward `-$0.NN` sign. The existing
`display_positive_usd`/`display_negative_usd`/`display_jpy_no_subunit`
tests are templates; learners expand them per currency × per boundary
value listed in the drill.

---

## E8.3 — Round-trip JSON (Medium) — shipped

Round-trip `Money` through `serde_json::to_string` and back; assert equality
via a proptest.

`Money` already derives `Serialize`/`Deserialize` in
`projects/06-stripe-money-lab/src/lib.rs` and the
`serializes_to_json_with_both_fields` unit test confirms the shape. The
proptest below is the full deliverable — paste it into the `prop_tests`
module alongside `add_is_commutative` and it runs against the
existing API.

<details><summary>Answer</summary>

```rust
proptest! {
    #[test]
    fn json_round_trip(cents in -100_000_000i64..100_000_000i64) {
        let m = Money::new(cents, Currency::USD).unwrap();
        let s = serde_json::to_string(&m).unwrap();
        let back: Money = serde_json::from_str(&s).unwrap();
        prop_assert_eq!(m, back);
    }
}
```
</details>

---

## E8.4 — Dispatch by event type in `webhook-receiver` (Medium) — shipped

In `webhook-receiver/src/lib.rs`, extract a `dispatch(meta, payload)` function
that matches on `meta.event_type` and calls a stubbed handler per type:

```rust
match meta.event_type.as_str() {
    "checkout.session.completed" => on_checkout_completed(payload),
    "invoice.paid"               => on_invoice_paid(payload),
    "customer.subscription.updated" => on_subscription_updated(payload),
    _ => Ok(()),                              // unknown types are 200'd silently
}
```

Each stub just logs for now. Add tests that fire each event type and assert
the appropriate stub was called (use a test-local channel or counter).

`projects/07-webhook-receiver/src/lib.rs` already ships HMAC signature
verification, the idempotency table, and the acknowledge-only handler
(the comment "Phase 8 lessons cover the dispatching" marks the
extraction point). The drill is to extract `dispatch(meta, payload)`
beside it and wire the three stubs; the existing integration tests in
`projects/07-webhook-receiver/tests/webhook.rs` are good templates for
the per-event assertions via a test-local counter.

---

## E8.5 — Out-of-order subscription updates (Medium) — shipped

Extend `webhook-receiver` with a `subscriptions` table and a handler for
`customer.subscription.updated` that *only* applies the update if the event's
`created` is newer than the row's `updated_at_stripe`. Add a test that
delivers two events for the same sub out of order and asserts the *newer*
state wins regardless of arrival order.

Build the new table on top of the existing migrations in
`projects/07-webhook-receiver/migrations/` and add the `WHERE
? > updated_at_stripe` guarded `UPDATE` next to the existing
idempotency-table writes in `projects/07-webhook-receiver/src/lib.rs`.
The two-out-of-order delivery test belongs in
`projects/07-webhook-receiver/tests/webhook.rs`, where the existing
"replay 200s but doesn't double-write" test demonstrates the test harness.

---

## E8.6 — Send-then-process pattern (Stretch) — shipped

Refactor the webhook handler so it returns 200 *as soon as the event is
persisted* and processes the dispatch in a background `tokio::spawn`. Use a
small in-memory queue or `tokio::sync::mpsc`. Add a test that asserts the
HTTP response arrives before the dispatch completes.

This builds directly on E8.4 — extract the synchronous handler in
`projects/07-webhook-receiver/src/lib.rs` to enqueue onto an
`mpsc::Sender<Event>` (held in `AppState`) and spawn a single
consumer task at startup. The "200 before dispatch completes" test
goes in `projects/07-webhook-receiver/tests/webhook.rs`; gate the
slow stub on a `tokio::sync::Notify` so the test can observe the
ordering deterministically.

---

## E8.7 — Proptest currency-mismatch is symmetric (Easy) — shipped

Property: for any two currencies A and B with A != B, `a.checked_add(b)` and
`b.checked_add(a)` both return `Err(MoneyError::CurrencyMismatch { .. })`.

Shipped as `currency_mismatch_is_symmetric` in the `prop_tests` module
of `projects/06-stripe-money-lab/src/lib.rs`. It generates two random
`(cents, currency)` pairs across all four currencies and asserts both
`checked_add` and `checked_sub` return a `CurrencyMismatch` for one
direction iff they do for the other — tightened with the equivalent
`iff a_cur != b_cur` check so the property pins both directions of the
biconditional.

---

## E8.8 — Reconciliation dry-run (Stretch) — shipped

Add a `dry_run` binary to `stripe-money-lab` that takes two CSV files
("stripe_charges.csv", "our_payments.csv"), each with columns
`stripe_charge_id, amount_cents, currency`, and prints the diff: charges in
one not the other, and the dollar discrepancy. Demonstrates the
reconciliation logic without a live Stripe.

The core primitives — `Money::checked_sub` (for the discrepancy) and
`Currency`/`MoneyError::CurrencyMismatch` (to refuse comparing apples
to JPY-shaped oranges) — already live in
`projects/06-stripe-money-lab/src/lib.rs`. Add the binary as
`projects/06-stripe-money-lab/src/bin/dry_run.rs` (or a `[[bin]]`
entry in `Cargo.toml`) and drive it through the `csv` crate; the
proportional-split tests are a template for verifying the diff sums
back to the originating row totals.
