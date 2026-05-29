# Lesson 8.10 — Refunds and Disputes (Chargebacks)

> **Concept first:** a refund is *us* returning money. A dispute is *the customer's bank* taking it back. Different events, different obligations, both audit-logged.
> **Time:** 20 minutes.

## Refunds

Issued via the admin dashboard or an automated flow. Stripe API:

```rust
stripe::Refund::create(&client, stripe::CreateRefund {
    charge: Some(charge_id),
    amount: Some(amount_cents),         // None = full refund
    reason: Some(stripe::RefundReason::RequestedByCustomer),
    metadata: Some(meta! { "actor_id" => admin.id, "reason" => "manual: ..." }),
    ..Default::default()
}).await?;
```

Idempotency key: yes. Mirror the resulting `Refund` in our `refunds` table.

Our schema:

```sql
CREATE TABLE refunds (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stripe_id       TEXT NOT NULL UNIQUE,
    payment_id      BIGINT NOT NULL REFERENCES payments(id),
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2100000000000),
    currency        CHAR(3) NOT NULL,
    reason          TEXT NOT NULL,
    actor_id        BIGINT REFERENCES users(id),       -- the admin who issued it (NULL = system)
    status          TEXT NOT NULL,                     -- 'pending'|'succeeded'|'failed'
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

The `actor_id` matters. Every refund is audit-logged with the human
responsible.

## Partial refunds and proportional splits

A multi-line invoice refunded partially needs the Phase 8.3 split. We use
`Money::split_proportional` to allocate the refund across lines so each
line is refunded a deterministic, integer-cent share.

## Disputes (chargebacks)

A dispute is when the customer's bank tells Stripe "this charge is invalid;
return the money." Stripe immediately debits us and notifies us via the
`charge.dispute.created` webhook. We have a short window (usually 7 days)
to submit evidence.

```rust
// charge.dispute.created handler
//   1. Insert disputes row (or update existing).
//   2. Send Slack alert to billing-ops.
//   3. Email the customer ("We see a dispute on your account. Here's how to resolve...").
//   4. Reserve the chargeback amount in our books — until resolution it's ours-and-not-ours.

// charge.dispute.funds_withdrawn
//   1. Update reserved_at.

// charge.dispute.closed
//   1. Outcome = 'won' | 'lost'.
//   2. Reverse the reserve accordingly.
//   3. Audit log.
```

Disputes are *expensive*. Stripe charges a fee per dispute regardless of
outcome. Combat them by:

- Clear receipt emails.
- Easy self-serve cancellation (so users don't dispute as a cancellation
  shortcut).
- A clear, working "contact us" path in every invoice.

## The dispute reserve

When a dispute opens, the disputed amount is in flux. In our ledger:

```sql
CREATE TABLE disputes (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stripe_id       TEXT NOT NULL UNIQUE,
    payment_id      BIGINT NOT NULL REFERENCES payments(id),
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2100000000000),
    currency        CHAR(3) NOT NULL,
    reason          TEXT,                                          -- 'fraudulent', 'product_not_received', ...
    status          TEXT NOT NULL,                                 -- 'needs_response'|'under_review'|'won'|'lost'
    funds_withdrawn_at TIMESTAMPTZ,
    closed_at       TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Reporting on "MRR net of disputed/refunded" matters — gross MRR can look
healthy while net is bleeding.

## Why this matters

- **Refunds are normal; the audit trail is the differentiator.** Every
  refund has a named human and a reason in the audit log.
- **Disputes are expensive *and* a UX signal.** A high dispute rate is
  customers telling you the cancellation path isn't obvious.
- **Reserves keep your books honest.** "We have $X in cash" must account
  for in-flight disputes.

## Green-bar checkpoint

- You can sketch the refunds + disputes schemas.
- You can describe the four `charge.dispute.*` webhooks and the action
  for each.
- You can name three things that *reduce* disputes proactively.

Next: `lessons/11-reconciliation.md`.
