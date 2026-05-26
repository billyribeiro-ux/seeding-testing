# Lesson 8.5 — One-Time Payments via Checkout

> **Concept first:** for one-off purchases (a "Lifetime" plan, an add-on workshop pass), Stripe Checkout in `mode: "payment"` is the lowest-friction path. The webhook tells us when it succeeds.
> **Time:** 20 minutes.

## The flow

```
1. User clicks "Buy the Workshop Pass — $99 one time".
2. Server creates a Checkout Session via Stripe API:
     POST /v1/checkout/sessions
     {
       "mode": "payment",
       "line_items": [{ "price": "price_workshop_pass", "quantity": 1 }],
       "success_url": "https://memberclub.test/billing/success?session_id={CHECKOUT_SESSION_ID}",
       "cancel_url":  "https://memberclub.test/billing/cancel",
       "client_reference_id": "<our_user_id>",
       "customer": "<stripe_customer_id>"    (optional, links to existing customer)
     }
     Idempotency-Key: <our_ulid>
   → Stripe returns { id, url, ... }
3. Server stores a row in `pending_purchases (user_id, session_id, product, amount_cents)`.
4. Server returns 303 redirect to the Stripe-hosted checkout URL.
5. User pays on Stripe's domain.
6. Stripe redirects to success_url (UX only).
7. Stripe POSTs a webhook `checkout.session.completed`.
8. Our handler:
   - Verifies signature.
   - Looks up the pending_purchase by session_id.
   - Inserts a row in `payments` with the Charge id.
   - Marks the user entitled to the product.
   - Sends a receipt email.
   - Logs an audit row.
```

The redirect is convenience; the webhook is truth.

## Why `client_reference_id` and not embedding the user id in URL params

The `success_url` is shown to the user; URL params end up in browser
history, server logs, and analytics. *Don't* put internal user ids there.
Stripe's `client_reference_id` is a field on the Session that we get back
in webhook events — internal only.

## Idempotency key — *before* the call

```rust
let idem_key = format!("checkout-create:{user_id}:{product}:{ulid}");

// 1. Persist the key + intent BEFORE calling Stripe
sqlx::query(
    "INSERT INTO pending_purchases (idempotency_key, user_id, product, amount_cents)
     VALUES ($1, $2, $3, $4)"
).bind(&idem_key).bind(user_id).bind(product).bind(amount.cents).execute(&tx).await?;
tx.commit().await?;

// 2. Now call Stripe with the key
let session = stripe_client.create_checkout_session(&body, &idem_key).await?;
```

Why persist first?

- If we crash *between* the DB insert and the Stripe call, on retry Stripe
  refuses the duplicate key. We never double-charge.
- If the Stripe call returns but the response is lost, the retry returns
  the same Session. We never create two sessions.

The order matters. *Persist intent, then act.*

## The minimum table shape

```sql
CREATE TABLE pending_purchases (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    product         TEXT NOT NULL,                          -- 'workshop_pass'
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2_100_000_000_00),
    currency        CHAR(3) NOT NULL,
    stripe_session_id TEXT UNIQUE,
    status          TEXT NOT NULL DEFAULT 'pending',         -- 'pending'|'completed'|'expired'
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE payments (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    stripe_charge_id TEXT NOT NULL UNIQUE,
    stripe_payment_intent_id TEXT NOT NULL,
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0 AND amount_cents < 2_100_000_000_00),
    currency        CHAR(3) NOT NULL,
    succeeded_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## In Rust with `async-stripe`

```rust
use stripe::{Client, CreateCheckoutSession, CheckoutSession};

let client = Client::new(env::var("STRIPE_API_KEY")?);
let session = CheckoutSession::create(&client, CreateCheckoutSession {
    mode: Some(stripe::CheckoutSessionMode::Payment),
    line_items: Some(vec![ stripe::CreateCheckoutSessionLineItems {
        price: Some("price_workshop_pass".into()),
        quantity: Some(1),
        ..Default::default()
    }]),
    success_url: Some("https://memberclub.test/billing/success?session_id={CHECKOUT_SESSION_ID}".into()),
    cancel_url: Some("https://memberclub.test/billing/cancel".into()),
    client_reference_id: Some(user_id.to_string()),
    customer: Some(customer_id),
    // Idempotency-Key header is set elsewhere on the client
    ..Default::default()
}).await?;
```

`async-stripe`'s API is type-safe and matches Stripe's REST API one-for-one.

## What `success_url` should actually do

```html
<!-- /billing/success page -->
<h1>Thank you for your purchase!</h1>
<p>Your receipt will arrive by email shortly.</p>
<p>(If you have any questions, reply to that email.)</p>
```

That's it. The page does *not*:

- Mark the purchase as completed (the webhook does that).
- Grant entitlement (the webhook does that).
- Trust the URL parameters (a malicious user can hand-craft them).

If the user lands here and *no webhook has arrived yet*, the receipt email
will be the next clear signal. The page is a friendly placeholder.

## Why this matters

- **Persist intent before the network call** is the universal pattern for
  any third-party write. Not specific to Stripe.
- **The webhook is the source of truth.** The redirect is decoration.
- **Idempotency keys + UNIQUE constraints = no double-charges.** This pair
  protects you from every retry, network blip, and client double-click.

## Green-bar checkpoint

- You can sketch the persist-then-call pattern with a transaction.
- You can articulate why `success_url` is *not* trusted.
- You can name three reasons we use Stripe's `client_reference_id`.

Next: `lessons/06-recurring-subscriptions.md`.
