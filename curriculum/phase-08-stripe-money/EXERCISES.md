# Phase 8 — Exercises

Eight drills across the Money primitive and the webhook receiver.

---

## E8.1 — `Money::checked_div` (Easy)

Add a `checked_div(self, by: i64) -> Result<Money, MoneyError>` that performs
integer division. Document the rounding behavior (truncation toward zero) in
the doc comment.

---

## E8.2 — Display tests for every currency (Easy)

For each of the four currencies, write a unit test asserting Display output
for $0.00, $0.05, $1.50, -$10.00, the ceiling-minus-1.

---

## E8.3 — Round-trip JSON (Medium)

Round-trip `Money` through `serde_json::to_string` and back; assert equality
via a proptest.

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

## E8.4 — Dispatch by event type in `webhook-receiver` (Medium)

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

---

## E8.5 — Out-of-order subscription updates (Medium)

Extend `webhook-receiver` with a `subscriptions` table and a handler for
`customer.subscription.updated` that *only* applies the update if the event's
`created` is newer than the row's `updated_at_stripe`. Add a test that
delivers two events for the same sub out of order and asserts the *newer*
state wins regardless of arrival order.

---

## E8.6 — Send-then-process pattern (Stretch)

Refactor the webhook handler so it returns 200 *as soon as the event is
persisted* and processes the dispatch in a background `tokio::spawn`. Use a
small in-memory queue or `tokio::sync::mpsc`. Add a test that asserts the
HTTP response arrives before the dispatch completes.

---

## E8.7 — Proptest currency-mismatch is symmetric (Easy)

Property: for any two currencies A and B with A != B, `a.checked_add(b)` and
`b.checked_add(a)` both return `Err(MoneyError::CurrencyMismatch { .. })`.

---

## E8.8 — Reconciliation dry-run (Stretch)

Add a `dry_run` binary to `stripe-money-lab` that takes two CSV files
("stripe_charges.csv", "our_payments.csv"), each with columns
`stripe_charge_id, amount_cents, currency`, and prints the diff: charges in
one not the other, and the dollar discrepancy. Demonstrates the
reconciliation logic without a live Stripe.
