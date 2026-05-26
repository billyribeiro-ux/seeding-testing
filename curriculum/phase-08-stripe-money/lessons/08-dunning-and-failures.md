# Lesson 8.8 — Dunning, Failed Payments, Grace Periods

> **Concept first:** payments fail. Customers don't lose access *immediately*. The art of dunning is to recover the payment without alienating the customer.
> **Time:** 20 minutes.

## What "dunning" means

Dunning = the process of recovering money from a customer whose payment
method failed. Done well, you recover 50–70% of failed payments. Done badly,
you alienate customers into churn.

## The Stripe automatic flow

When `invoice.payment_failed` fires, Stripe runs the **Smart Retries**
policy you configured in the dashboard:

```
retry 1: +3 days
retry 2: +5 days
retry 3: +7 days
final:   mark unpaid, transition subscription to `unpaid` (or cancel)
```

You don't have to retry yourself. You *do* have to react.

## What our handler does

```rust
// invoice.payment_failed
//   1. Mark our `invoices` row with attempt_count, next_payment_attempt.
//   2. UPDATE subscriptions.status = 'past_due'.
//   3. UPDATE users.tier — DO NOT immediately downgrade.
//   4. Send an email: "your payment failed; we'll retry in 3 days."
//   5. Audit log.

// invoice.paid (after a retry succeeded)
//   1. UPDATE invoices.status = 'paid'.
//   2. UPDATE subscriptions.status = 'active'.
//   3. Email a thank-you.

// customer.subscription.updated → status = 'unpaid' or 'canceled'
//   1. UPDATE users.tier = 'free' (the downgrade actually happens).
//   2. Send a "we couldn't recover the payment" email.
//   3. Keep the user's data.
```

## The grace period

When status moves to `past_due`, **don't downgrade immediately**. Many
payment failures resolve within hours (card was over the limit; user
topped up). A 3-day grace period is the industry standard.

Implementation: `tier` projection rule changes from "active subscription" to
"active OR past_due subscription with `past_due_since > NOW() - 3 days`":

```sql
UPDATE users SET tier = (
    SELECT CASE
        WHEN status = 'active' THEN price.tier_name
        WHEN status = 'past_due' AND COALESCE(updated_at, created_at) > NOW() - INTERVAL '3 days'
             THEN price.tier_name
        ELSE 'free'
    END
    FROM subscriptions s LEFT JOIN prices p ON p.id = s.primary_price_id
    WHERE s.user_id = users.id
    ORDER BY s.current_period_end DESC LIMIT 1
) WHERE id = $1;
```

(In practice, you compute this projection in code; the SQL above is illustrative.)

## Customer-facing UX

Show a banner the first time the user logs in after a failed payment:

```
⚠️ Your payment failed last night. We'll try again on May 29.
   You still have full Pro access until May 31.
   [Update Card]
```

The "[Update Card]" button redirects to Stripe's Customer Portal — a hosted
page that handles secure card updates without you ever touching card data.

```rust
// POST /v1/billing/portal — redirects to Stripe
let session = stripe::BillingPortal::Session::create(&client, ...).await?;
return Json(json!({ "url": session.url }));
```

## What *not* to do

- **Don't email after every retry.** That's 3+ emails per failure. Spam
  filters notice; customers churn.
- **Don't immediately email after the first failure.** Sometimes Stripe
  recovers within minutes (low-friction issuer).
- **Don't downgrade silently.** A user who logs in and finds their tier
  reverted with no warning is a churn risk.

## Smart-retries vs your own

Stripe's Smart Retries are surprisingly good — ML-driven, account-aware. We
*don't* roll our own. The webhook integration above is enough.

If you have to retry yourself (e.g. you're billing outside Stripe), use
exponential backoff with jitter and cap at ~5 attempts over ~10 days.

## Why this matters

- **Failed payments are not "errors" to fix; they're a *category of business
  event*.** The right reaction is communication + grace period + retry,
  not 5xx page.
- **The Customer Portal is your friend.** Card updates that go through it
  never hit your codebase; PCI scope is reduced.
- **Grace periods are the difference between "your service punishes me for
  bank flakiness" and "your service is reasonable."**

## Green-bar checkpoint

- You can describe the four webhook events that drive dunning.
- You can articulate why we don't downgrade immediately on `past_due`.
- You can sketch the customer-portal redirect endpoint.

Next: `lessons/09-tax-and-invoices.md`.
