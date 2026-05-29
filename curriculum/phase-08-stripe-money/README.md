# Phase 8 — Stripe + Money (the Enterprise Way)

> **Audience:** you finished Phase 7.
> **Outcome:** you can represent money correctly, integrate Stripe safely (Checkout + subscriptions + webhooks), survive at-least-once delivery via idempotency keys, and reconcile your ledger against Stripe nightly.
> **Time:** 3–4 weeks.

## The mental model

> **Money is not a float. Ever.**
>
> Money is an integer count of the smallest currency unit (cents for USD) plus
> a currency code. A `Money(i64 cents, Currency)` newtype is the only shape
> that touches business logic.

Three corollaries that decide every Stripe argument:

1. **Storage: `i64` cents only.** No `f64`. The Postgres column type is
   `BIGINT NOT NULL`, with a `CHECK` constraint binding values to the
   **$21 billion ceiling** (`MONEY_CEILING_CENTS = 2_100_000_000_000`).
2. **Stripe is not the source of truth.** Our DB owns customers,
   subscriptions, invoices, payments. Stripe is the *rail*. We mirror Stripe
   events; we never read Stripe synchronously in a user-facing path.
3. **Every Stripe write is idempotent.** Every state-change endpoint accepts
   an idempotency key persisted *before* the call. Every webhook handler is
   replay-safe.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-money-not-a-float.md` | Why floats fail, history of "off by a penny" disasters |
| `lessons/02-the-money-newtype.md` | `Money(i64, Currency)`, ceiling, checked arithmetic, proptest |
| `lessons/03-proportional-split.md` | Largest-remainder method, sums-exactly invariant |
| `lessons/04-stripe-data-model.md` | Customer / PaymentMethod / Product / Price / Subscription / Invoice / Charge |
| `lessons/05-one-time-payments.md` | Checkout Session → PaymentIntent flow |
| `lessons/06-recurring-subscriptions.md` | Trials, proration, plan change, cancel-at-period-end |
| `lessons/07-webhooks.md` | Signature verification, idempotency, replay safety, out-of-order |
| `lessons/08-dunning-and-failures.md` | Failed payments, smart retries, grace period, downgrade |
| `lessons/09-tax-and-invoices.md` | Stripe Tax vs manual rates, tax-inclusive/exclusive |
| `lessons/10-refunds-and-disputes.md` | Chargeback flow, reserves |
| `lessons/11-reconciliation.md` | Nightly diff: our DB vs Stripe Balance/Payouts |
| `lessons/12-build-stripe-money-and-webhook.md` | The two capstone projects |

## The capstones

- **`projects/06-stripe-money-lab`** — Pure Rust. The `Money` newtype, the
  ceiling enforcement, the proportional split via the largest-remainder
  method. Proptest
  proves invariants ("sum of split parts equals original," "checked arithmetic
  never silently overflows"). No I/O — fast, deterministic, mathematically
  rigorous.
- **`projects/07-webhook-receiver`** — Axum service that receives signed
  Stripe-style webhooks, verifies the signature, stores events idempotently,
  and processes them once. Replay-safe: the same webhook payload arriving
  three times produces *one* side effect.

Stripe integration *itself* (creating real charges via async-stripe) lives in
the MemberClub capstone from Phase 9 onward. This phase locks down the
*primitives* — the patterns that make Stripe usage safe.

## Green-bar checkpoint

```bash
cargo test  -p stripe-money-lab    # all green; proptests pass
cargo test  -p webhook-receiver    # signature + idempotency tests green
make verify                        # workspace green
```

## What's next

Phase 9 — **Svelte 5 / SvelteKit 2**. We build the MemberClub web app. The
backend (auth, RBAC, Stripe) is ready; we ship the user-facing piece.
