# Lesson 8.7 — Webhooks: Signature, Idempotency, Replay Safety

> **Concept first:** every webhook request might be a forgery, a duplicate, or out of order. Three defenses, all required, in this exact order.
> **Time:** 35 minutes.

## The threat model

A webhook endpoint is a public URL. Without protection:

- **Forgery** — anyone can POST to it; an attacker could "upgrade" a user
  by fabricating a `checkout.session.completed`.
- **Duplication** — Stripe (and any reliable delivery system) retries on
  errors. The same event arrives 3, 5, 30 times until you 200 it.
- **Out-of-order delivery** — `subscription.deleted` can arrive *before*
  `subscription.created` if the network reordered packets.

Three defenses:

1. **Verify the signature** to prove the request is from Stripe.
2. **Persist event ids** and reject duplicates.
3. **Use Stripe event timestamps**, not arrival order, for state machine
   logic.

## Defense 1 — signature verification

Stripe signs every webhook with HMAC-SHA-256 using a *webhook secret*
unique to your endpoint. The signature lives in the `Stripe-Signature`
header:

```
Stripe-Signature: t=1748275200,v1=5257a869e7ecebeda32affa62cdca3fa51cad7e77a0e56ff536d0ce8e108d8bd
```

Parse: `t=<timestamp>`, `v1=<signature>`.

Verify:

```
expected = hex(hmac_sha256(secret, "{t}.{raw_body}"))
allow if expected == provided AND |now - t| < 5 minutes
```

The 5-minute window prevents replay of *captured* legitimate webhooks
days later.

### The "verify *before* parsing" rule

```rust
async fn webhook(headers: HeaderMap, body: Bytes) -> Result<StatusCode, ApiError> {
    let signature = headers.get("Stripe-Signature").ok_or(ApiError::Unauthorized)?;
    verify_signature(&body, signature, &state.stripe_secret)?;  // verify FIRST
    let event: stripe::Event = serde_json::from_slice(&body)?;  // parse AFTER
    handle_event(event).await
}
```

If we parse first, an attacker who sends malformed JSON learns that we
*tried* to parse it before checking the signature. Don't leak that.
Always verify, then parse.

### `Bytes`, not `Json<T>`, as the extractor

Axum's `Json<T>` consumes the body to deserialize. We need the *raw* body
for signature verification. Use `Bytes`:

```rust
async fn webhook(headers: HeaderMap, body: Bytes) -> ...
```

## Defense 2 — idempotent event storage

The pattern: insert the event id with a UNIQUE constraint *before* doing
any work. If the insert fails (duplicate), the event was already
processed.

```sql
CREATE TABLE stripe_events (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stripe_event_id TEXT NOT NULL UNIQUE,
    event_type      TEXT NOT NULL,
    created_at_stripe TIMESTAMPTZ NOT NULL,
    received_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at    TIMESTAMPTZ,
    payload         JSONB NOT NULL
);
```

```rust
// Try to insert. If it conflicts, the event is a duplicate.
let inserted: Option<i64> = sqlx::query_scalar(
    "INSERT INTO stripe_events (stripe_event_id, event_type, created_at_stripe, payload)
     VALUES ($1, $2, $3, $4)
     ON CONFLICT (stripe_event_id) DO NOTHING
     RETURNING id"
).bind(event.id).bind(event.type_str()).bind(event.created).bind(payload_json)
 .fetch_optional(&pool).await?;

let Some(row_id) = inserted else {
    return Ok(StatusCode::OK);  // already processed; ack the retry
};

handle(event).await?;

sqlx::query("UPDATE stripe_events SET processed_at = NOW() WHERE id = $1")
    .bind(row_id).execute(&pool).await?;
Ok(StatusCode::OK)
```

Three rules:

1. **`INSERT ... ON CONFLICT DO NOTHING RETURNING id`** is the atomic
   "insert if new" primitive. A second call returns `None`; only the first
   call gets a row id.
2. **Mark `processed_at` *after* the handler succeeds.** If we crash mid-handler,
   the next delivery re-runs us — and that's *fine* because the handler is
   itself idempotent (see "handler design" below).
3. **Always 200 a duplicate.** Stripe stops retrying on 200. Returning an
   error on duplicates causes endless retries.

## Defense 3 — out-of-order tolerance

Two webhooks for the same subscription:

```
A: customer.subscription.updated   created_at = 12:01:00   status: active
B: customer.subscription.updated   created_at = 12:01:30   status: past_due
```

If A arrives *after* B due to network reordering, naive code would
overwrite `past_due` with `active`. Bad.

Fix: compare timestamps and only apply if newer.

```rust
sqlx::query(
    "UPDATE subscriptions
     SET status = $1, current_period_end = $2, updated_at_stripe = $3
     WHERE stripe_id = $4 AND COALESCE(updated_at_stripe, 'epoch') < $3"
).bind(...).execute(&pool).await?;
```

The `WHERE updated_at_stripe < $3` guard makes the UPDATE a no-op for
out-of-order deliveries. Same row, same query, deterministic outcome.

## Handler design — idempotent at the unit level

A good webhook handler is idempotent *even without* the `stripe_events`
table. The table is belt-and-braces.

For `customer.subscription.created`:

```sql
INSERT INTO subscriptions (...) VALUES (...)
ON CONFLICT (stripe_id) DO UPDATE SET
    status = EXCLUDED.status,
    current_period_end = EXCLUDED.current_period_end,
    updated_at_stripe = EXCLUDED.updated_at_stripe;
```

Running it twice converges. Running it once works. The
`UPDATE updated_at_stripe < $3` guard in defense 3 covers the ordering bit.

## Acking long handlers

Stripe times out webhook requests after 30 seconds and retries. If your
handler is slow (sending email, calling an LLM, etc.), do this:

1. **Verify signature.**
2. **Insert into `stripe_events`** (the idempotency row).
3. **Return 200 immediately.**
4. **Process the event in a background task** (with retries on failure).

The DB row is the queue.

This is essentially the *outbox* pattern. Phase 11 covers it in depth.

## Local testing — Stripe CLI

```bash
stripe listen --forward-to localhost:3000/webhooks/stripe
# Prints: webhook signing secret is whsec_...
# Use that in your .env as STRIPE_WEBHOOK_SECRET.

stripe trigger checkout.session.completed
stripe trigger invoice.paid
stripe trigger customer.subscription.updated
```

Each `trigger` synthesizes a realistic event signed with the listen
secret. Trigger the same event 3 times; verify your handler processes it
*once*.

## Why this matters

- **Each defense is necessary; none is sufficient.** Signature without
  idempotency = double-applied events. Idempotency without signature =
  forgery vector. Both without timestamp guards = race-condition state
  corruption.
- **At-least-once delivery is the *only* delivery you'll ever get.** Plan
  for it.
- **Test mode + Stripe CLI** let you simulate everything; you don't need a
  real card to validate end-to-end.

## Green-bar checkpoint

- You can write the HMAC-SHA-256 verification routine.
- You can sketch the `stripe_events` table + the
  `INSERT ... ON CONFLICT DO NOTHING RETURNING id` pattern.
- You can articulate the difference between "the table makes the handler
  idempotent" and "the handler is idempotent on its own."

Next: `lessons/08-dunning-and-failures.md`.
