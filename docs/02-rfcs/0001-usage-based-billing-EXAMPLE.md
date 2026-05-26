# RFC 0001 — Usage-Based Billing (Worked Example)

> **This RFC is a worked example, not an actual proposal.** Phase 12
> learners read it to see what "good" looks like. To author your own,
> copy `0000-TEMPLATE.md`.

- Author: @alice
- Status: Adopted (example)
- Reviewers: @bob (billing), @carol (product), @dave (data)
- Date: 2026-05-26
- Related: ADR 0003 (i64 cents), ADR 0006 (outbox), ADR 0008 (Stripe is the rail)

## Summary

Add a usage-based billing dimension to MemberClub: customers on the Pro
and Elite tiers can purchase additional storage past their plan limit, at
$0.10 / GB / month, billed monthly at the end of the period (post-paid).

## Motivation

Three signals:

1. **Customer requests.** 14 support tickets in the last 60 days asking
   for "buy more storage" rather than upgrading to the next tier
   ($30 → $99 is too big a jump for users hitting just their limit).
2. **Competitive pressure.** Dropbox, iCloud, and Notion all expose this
   model; we lose Pro customers who hit the cap to alternatives.
3. **Margin opportunity.** Storage cost per GB is $0.02 on our infra at
   current volume; $0.10 is 5× margin.

Data: 8.2% of Pro customers used > 90% of their quota in Q1; 1.4% hit
the cap and were blocked. Estimated incremental ARR if 50% adopt:
~$160K/year.

## Goals

- **G1.** Customers can opt-in to usage-based storage post their plan
  limit, with real-time consumption visibility and a soft cap.
- **G2.** Billing happens monthly at the end of the period; the new
  charge shows on the regular invoice as an "Overage" line item.
- **G3.** p99 latency on the usage-recording endpoint < 50 ms; p99 of
  the customer dashboard < 200 ms.
- **G4.** Existing customers see no behavior change unless they opt in.

## Non-goals

- **N1.** We are not implementing per-second metering.
- **N2.** We are not switching the base subscription model to usage-only.
- **N3.** We are not building a generic metering framework — this is
  specifically for storage.

## Proposed Design

### Data model

```sql
CREATE TABLE usage_records (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    customer_id     BIGINT NOT NULL REFERENCES users(id),
    kind            TEXT NOT NULL CHECK (kind IN ('storage_bytes')),
    amount          BIGINT NOT NULL CHECK (amount >= 0),
    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
)
PARTITION BY RANGE (recorded_at);                       -- monthly partitions

CREATE TABLE usage_periods (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    customer_id     BIGINT NOT NULL REFERENCES users(id),
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    peak_bytes      BIGINT NOT NULL DEFAULT 0,
    overage_bytes   BIGINT NOT NULL DEFAULT 0,
    overage_cents   BIGINT NOT NULL CHECK (overage_cents >= 0 AND overage_cents < 2_100_000_000_00),
    stripe_invoice_id TEXT,
    UNIQUE (customer_id, period_start, period_end)
);
```

Partitioning by month keeps the hot partition small (~5 GB / month at
current volume).

### Flow

```
Real-time write path:
1. App writes a usage_records row on each storage event (upload, delete, etc.).
   ~10 events / customer / day.
2. A trigger updates `usage_periods.peak_bytes` if the running total exceeds it.

End-of-month aggregation (cron via outbox):
1. For each customer, compute overage = max(0, peak_bytes - plan_limit).
2. Compute overage_cents = (overage_bytes / 2^30) * 10 cents, rounded down.
3. Insert outbox job ('stripe.add_invoice_item') with the value.
4. The worker calls Stripe Add Invoice Item via async-stripe.
5. The next subscription invoice includes the line item.

Customer dashboard:
- Reads from `usage_periods` for the current period (eventually consistent).
- Real-time consumption from a Redis counter updated synchronously.
```

### Failure modes

| Downstream | If it's down | What we do |
|---|---|---|
| Stripe API | Outbox accumulates; nightly cron retries | Customers see "billing temporarily unavailable"; charge still happens later |
| Redis (counter) | Falls back to last-known-good value from DB | Dashboard staleness up to ~5 minutes; no billing impact |
| Postgres (hot partition) | Cannot record usage | Usage events get retried client-side (we already do this for note saves); the rest of the app degrades but does not fail |

### Alternatives considered

#### A — Stripe Metered Billing

Pros:
- Native integration; Stripe stores the usage record.
- Less code.

Cons:
- Per-event API calls hit Stripe rate limits at our volume
  (we'd need to batch anyway).
- Invoice rendering is locked to Stripe's format; harder to surface
  per-day breakdowns in our UI.
- Refunds and disputes against metered items are clunky.

#### B — Pre-paid credits

Pros:
- Cash up front; no surprise invoices.
- Simpler accounting.

Cons:
- Customer feedback (5 interviews) strongly preferred post-paid.
- Refund of unused credits is its own UX headache.

#### C — Switch the entire model to usage-based

Pros: predictable for cost-conscious users.
Cons: way out of scope for this RFC; tier-based fits 91% of users.

## Decision Matrix

| Option | Implementation | Latency | Customer fit | Risk |
|---|---|---|---|---|
| A — Stripe Metered Billing | 2 weeks | Medium | Medium | Low |
| **B — Custom + Stripe invoice items (chosen)** | 5 weeks | High | High | Medium |
| C — Pre-paid credits | 3 weeks | High | Low | Low |
| D — Status quo | 0 weeks | — | Low | Low (but losing customers) |

## Implementation plan

| Week | Owner | Work |
|---|---|---|
| 1 | @alice | Schema + migrations + tests |
| 2 | @bob | Real-time write path + tests |
| 2 | @alice | Redis counter; dashboard read path |
| 3 | @alice | Aggregation cron + outbox handler |
| 4 | @bob | Stripe invoice item integration |
| 4 | @carol | Customer dashboard UI + e2e tests |
| 5 | All | Grafana dashboard, alerts, runbook, docs |

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Customers surprised by overage charges | Soft cap at $10; email at $5, $8, $10 |
| Customer hits hard cap, can't upload | Soft cap pauses new uploads but doesn't delete; clear UX |
| Stripe invoice item creation fails silently | Outbox retries; nightly reconciliation alerts on discrepancy |
| Hot partition gets too big | Monthly partitions cap each at < 10 GB; quarterly archive job |

## Migration

- Existing customers: no behavior change. Toggle off by default.
- Opt-in via the billing settings page; takes effect at the start of
  the next period.
- Communicate via in-product banner for 30 days plus an email at
  launch + 7d + 14d + 28d.

## Open questions

1. **Refunds.** How do customer-initiated refunds interact with
   overage? Decision: prorated refund of the overage portion only.
2. **Multi-region storage.** Some customers want EU-only data
   residency, which costs us more. Treat as a separate ADR.
3. **Salesforce CPQ integration.** Out of scope; will be a follow-up
   RFC if Enterprise adopts.

## Decision

- **Outcome:** Adopted.
- **Decider:** @cto.
- **Date:** 2026-05-30.
- **Follow-up ADR:** `docs/01-architecture-decisions/0009-overage-billing.md` (to be written).
- **Ticket:** #2150 (Epic).
