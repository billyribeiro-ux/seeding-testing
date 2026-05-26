# Lesson 8.6 — Recurring Subscriptions

> **Concept first:** a subscription is a state machine. Stripe runs it; we mirror it. The interesting bits are trials, proration on plan change, and cancellation policy.
> **Time:** 30 minutes.

## The state machine

```
       trialing ─────► active ─────► past_due ─────► unpaid / canceled
                          ▲              │
                          │              ▼
                       (recovery)    incomplete
```

Each transition is a webhook event:

| Event | Effect |
|---|---|
| `customer.subscription.created` | Insert `subscriptions` row |
| `customer.subscription.updated` | UPDATE the row (status, prices, current_period_end) |
| `customer.subscription.deleted` | Mark canceled |
| `invoice.created` | Insert `invoices` row |
| `invoice.paid` | UPDATE invoice + bump access if needed |
| `invoice.payment_failed` | Move to past_due; start dunning |

## Trials

Pass `trial_period_days: 14` (or `trial_end: unix_ts`) on subscription
creation. Stripe starts the sub as `trialing`; no payment method required if
we set `payment_settings.payment_method_collection: "if_required"`.

During the trial, the user has full access. On day 13 we send a reminder
email. On day 14, Stripe attempts payment; if successful it transitions to
`active`, otherwise `incomplete`.

## Plan change with proration

User on Pro ($10/mo, period 1–30) upgrades to Elite ($30/mo) on day 10.
Two facts:

- They've already paid $10 for 30 days; they used 10 days, so $6.67 is
  unused.
- They now owe Elite for the remaining 20 days: $30 × 20/30 = $20.

Stripe automatically prorates: a *credit* invoice item of −$6.67 plus a
*debit* invoice item of $20 → net $13.33 charged immediately. The next
month they pay the full $30.

To get proration:

```rust
Subscription::update(&client, &sub_id, UpdateSubscription {
    items: Some(vec![ UpdateSubscriptionItems { price: Some(new_price_id), .. } ]),
    proration_behavior: Some(ProrationBehavior::CreateProrations),
    ..Default::default()
}).await?;
```

The math is Stripe's job; ours is to mirror the result.

> Reminder: any time *we* compute proration (e.g. for a custom split), we
> use the largest-remainder method from Lesson 8.3.

## Cancellation policy

Two flavors:

- **Cancel at period end** (`cancel_at_period_end: true`) — the standard.
  User keeps access until period end; no refund.
- **Cancel immediately** (`cancel_at: now`) — usually with a prorated
  refund for the unused portion. Customer-support territory.

Default to cancel-at-period-end. Make immediate cancellation opt-in.

```rust
Subscription::update(&client, &sub_id, UpdateSubscription {
    cancel_at_period_end: Some(true),
    ..Default::default()
}).await?;
```

Stripe emits `customer.subscription.updated`; we set
`subscriptions.cancel_at_period_end = true` and show "your subscription
ends on <period_end>" in the UI.

## What ships in MemberClub

```sql
CREATE TABLE subscriptions (
    id                BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id           BIGINT NOT NULL REFERENCES users(id),
    stripe_id         TEXT NOT NULL UNIQUE,
    status            TEXT NOT NULL,                       -- mirror of Stripe status
    tier              TEXT NOT NULL,                       -- 'free'|'pro'|'elite' projected from prices
    current_period_start TIMESTAMPTZ NOT NULL,
    current_period_end   TIMESTAMPTZ NOT NULL,
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
    trial_end         TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX subscriptions_user_id_active_idx
    ON subscriptions (user_id) WHERE status IN ('trialing','active','past_due');
```

The partial unique index enforces *at most one active subscription per
user*. Duplicate Stripe subs (rare but possible) get caught here.

## The `tier` projection

`tier` is denormalized — it's computed from the user's active subscription's
prices. We update it on every relevant webhook so user-facing reads
(`SELECT tier FROM users WHERE id = $1`) are O(1):

```sql
UPDATE users SET tier = (
    SELECT tier FROM subscriptions
    WHERE user_id = users.id AND status IN ('trialing','active','past_due')
    ORDER BY current_period_end DESC LIMIT 1
) WHERE id = $1;
```

`COALESCE(...)` to `'free'` if no active sub exists.

## Why this matters

- **Stripe owns the state machine; we mirror it.** Don't fight this — the
  worst bugs come from trying to manage subscription state independently.
- **Proration is *math*; let Stripe do it.** Our `Money` newtype only
  reappears when we *split* totals ourselves.
- **The denormalized `tier` field** is what RBAC/ABAC policies read. Keep
  it fresh via webhook handlers; never recompute on every request.

## Green-bar checkpoint

- You can sketch the subscription state machine and name the webhook for
  each transition.
- You can choose between cancel-at-period-end and immediate cancellation
  for a given product policy.
- You can articulate why the `tier` field is a projection of the active
  subscription.

Next: `lessons/07-webhooks.md` — the most consequential lesson.
