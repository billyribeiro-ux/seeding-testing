# Lesson 8.4 — Stripe's Data Model

> **Concept first:** Stripe's API has ~30 entity types. Twelve of them carry the weight; learning those is 80% of the integration.
> **Time:** 25 minutes.

## The twelve entities that matter

```
Customer          a person/company (one per signup)
PaymentMethod     a credit card, bank account, wallet attached to a Customer
Product           a thing you sell ("MemberClub Pro")
Price             a specific price for a Product ($10/mo, $100/yr, $200 lifetime)
Subscription      a recurring billing cycle binding a Customer to one or more Prices
Invoice           a periodic bill — drafted, finalized, paid, voided
InvoiceItem       a line on an invoice (proration, one-off add-ons)
PaymentIntent     an attempt to charge a payment method (one per try)
Charge            the actual successful payment (a captured PaymentIntent)
Refund            a partial or full reversal of a Charge
SetupIntent       collect a payment method without charging (e.g. trial signup)
Webhook Event     "something happened" — the only way you'll hear about state changes
```

## The relationships

```
Customer ──< PaymentMethod
   │
   ├──< Subscription ──< Invoice ──< InvoiceItem
   │                       │
   │                       └── PaymentIntent ──> Charge ──< Refund
   │
   └──< Charge (for one-off purchases)
```

A Customer has many PaymentMethods. A Customer has many Subscriptions
(usually one). A Subscription has many Invoices over time. Each Invoice has
many InvoiceItems and one PaymentIntent (which becomes a Charge on success).
Refunds attach to Charges.

## What's in *our* database vs Stripe's

| In our DB | In Stripe |
|---|---|
| `users` (with `stripe_customer_id`) | the Customer object |
| `subscriptions` (mirror with `stripe_subscription_id`) | the Subscription object |
| `invoices` (mirror) | the Invoice object |
| `payments` (one row per Charge attempt) | PaymentIntent + Charge |
| `audit_logs` for every privileged action | — |
| `stripe_events` (one row per delivered webhook for idempotency) | their event log |

We *mirror* Stripe — we never depend on it being reachable during a
user-facing request. When a user opens their billing page, the rendering
reads our DB; if Stripe is down, the page still loads.

## The flow for a typical upgrade

```
1. User clicks "Upgrade to Pro" on MemberClub.
2. Server: POST to Stripe Checkout Session, with success_url + cancel_url.
   - Pass `mode: "subscription"` and `line_items: [{ price: $price_id, quantity: 1 }]`
   - Pass `client_reference_id: <our user_id>`
   - Pass `idempotency_key: ulid()` (persisted before the call)
3. Stripe returns a Checkout Session URL.
4. Server returns the URL; client redirects there.
5. User pays at Stripe.
6. Stripe redirects back to our `success_url`.
7. *Independently*, Stripe POSTs a webhook event:
   - `checkout.session.completed`
   - `customer.subscription.created`
   - `invoice.paid`
8. Our webhook handler:
   - Verifies the signature.
   - Idempotently records the event id.
   - Looks up our user via `client_reference_id`.
   - Inserts the `subscriptions` row, the `invoices` row, etc.
   - Updates `users.tier` (the cached projection).
   - Sends a receipt email.
```

The webhook is *the* state-update mechanism. The redirect to `success_url`
is a UX nicety; never trust it as the source of truth.

## The lifecycle of a subscription

| Stripe status | Our `subscriptions.status` | What we do |
|---|---|---|
| `trialing` | `trialing` | Allow access, count days left, send reminder emails |
| `active` | `active` | Normal access |
| `past_due` | `past_due` | Show banner; grace period of N days |
| `incomplete` | `incomplete` | First payment failed; require new payment method |
| `incomplete_expired` | `expired` | Took too long; revert to free tier |
| `canceled` | `canceled` | Will access until period end (if cancel-at-period-end) |
| `unpaid` | `unpaid` | Revert to free tier; keep data |

Stripe sends webhook events for every state change. We mirror them with one
`UPDATE` per event.

## Test mode vs live mode

Stripe has two completely separate environments:

- **Test mode** — `sk_test_*` keys; no real cards; magic card numbers
  (`4242 4242 4242 4242`) test specific outcomes.
- **Live mode** — `sk_live_*` keys; real cards; real money.

Two webhook secrets, two API keys, two dashboard views. **Never mix them.**
Our `.env.example` makes the test/live distinction explicit; we never put
live keys in `.env`.

## The Stripe CLI

```bash
stripe login                         # OAuth-link to your test account
stripe listen --forward-to localhost:3000/webhooks/stripe   # forward to your local server
stripe trigger checkout.session.completed                   # send a synthetic event
stripe trigger invoice.paid
```

`stripe listen` is the bridge between Stripe's webhook delivery and your
laptop. It signs events with a *test webhook secret* (printed when you run
`listen`), which you put in your `.env`.

## What's in MemberClub vs what's in this phase

This phase teaches the *primitives* — the `Money` newtype and the webhook
reliability machinery. The MemberClub capstone (Phase 9 onward) is where the
real subscription flows ship:

- Checkout Session endpoint.
- Customer Portal redirect.
- Subscription mirror tables.
- Dunning logic.

We split it that way so each Phase 8 lesson stays focused.

## Why this matters

- **Stripe is the most reliable third-party in your stack.** That doesn't
  mean it's always online. Mirror first; depend never.
- **Test mode is your safety harness.** Never disable it for "convenience."
- **The webhook is the source of truth.** The redirect URL is UX.

## Green-bar checkpoint

- You can name the 12 core Stripe entities and how they relate.
- You can describe the happy-path flow for an upgrade, naming each
  webhook event.
- You can articulate "mirror Stripe; don't read it in user-facing paths."

Next: `lessons/05-one-time-payments.md`.
