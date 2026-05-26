# ADR 0008 — Stripe is the rail; our database is the source of truth

- Status: Accepted
- Date: 2026-05-26
- Deciders: billing-team, api-team
- Tags: billing, stripe, architecture

## Context and Problem Statement

MemberClub depends on Stripe for actual money movement (Checkout
Sessions, Subscriptions, Invoices, Refunds). Stripe is reliable but not
*always* reachable. User-facing pages (login, content gates, account
settings) must remain fast and available even if Stripe is degraded.

## Decision Drivers

- User-facing request latency must not depend on Stripe API latency
  (Stripe's p99 ~300ms; ours target < 200ms).
- Availability: a Stripe outage must not take down our service.
- Correctness: our DB and Stripe must eventually agree, with audit.
- Compliance: receipts and audit history live in our DB, not in Stripe's
  retention policy.

## Considered Options

1. **Stripe as the source of truth.** Always read from Stripe in
   user-facing requests. Simple; couples our availability to theirs.
2. **Local mirror, webhooks as the sync mechanism.** Our DB holds the
   canonical state; Stripe drives updates via webhooks.
3. **Cache Stripe with TTL.** A middle path; still depends on Stripe
   periodically.

## Decision Outcome

Chose **option 2**.

- Our DB owns `customers`, `subscriptions`, `invoices`, `payments`,
  `refunds`, `disputes`. Each row mirrors a Stripe object.
- Writes go *through* Stripe (Checkout, Customer Portal, refunds via
  admin endpoints), with idempotency keys persisted *before* the API
  call.
- Stripe webhooks (Phase 8.7) update our mirror tables. Out-of-order
  updates are guarded by comparing event timestamps.
- User-facing reads (e.g. "what's my tier?") *never* call Stripe; they
  read our cached projection (`users.tier`).
- A nightly reconciliation job (Phase 8.11) diffs our totals against
  Stripe's Balance Reports and alerts on discrepancies.

## Consequences

- **Positive:** user-facing requests stay fast and available regardless
  of Stripe's state. We own our audit trail. Webhook handling discipline
  is well-defined and tested.
- **Negative:** the mirror can be temporarily stale (seconds during
  normal operation, minutes if a webhook retried). Operational
  complexity: we now run a reconciliation job and alert on it.
- **Mitigations:** webhooks must be processed within 30 seconds (Phase
  8.7's `outbox`-driven background processing keeps the response < 100ms);
  customers can refresh their billing page via the Customer Portal to
  trigger a re-sync if the mirror feels stale.

## Notes

This is the standard architecture for SaaS billing (Vercel, Linear,
Notion follow this pattern). The webhook receiver
(`projects/07-webhook-receiver`) is the foundational primitive.

Related: Phase 8 lessons 4–11; project `07-webhook-receiver`.
