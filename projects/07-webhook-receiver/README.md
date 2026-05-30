# projects/07-webhook-receiver

Phase 8, capstone 2 — a Stripe-compatible webhook receiver with the three
defenses every reliable handler needs:

1. **HMAC-SHA-256 signature verification.** Constant-time comparison; rejects
   forgeries and tampered bodies.
2. **Timestamp-bounded replay rejection.** Refuses signatures older than
   5 minutes; resists replay of captured legitimate webhooks.
3. **Idempotent storage** via `INSERT ... ON CONFLICT DO NOTHING RETURNING id`
   on a UNIQUE constraint. Duplicate deliveries are no-ops.

## Run it

```bash
STRIPE_WEBHOOK_SECRET=whsec_... cargo run -p webhook-receiver
# listens on 127.0.0.1:3002

stripe listen --forward-to localhost:3002/webhooks/stripe
stripe trigger checkout.session.completed
stripe trigger checkout.session.completed     # 2nd time — duplicate, ignored
stripe trigger checkout.session.completed     # 3rd time — still ignored
```

## Test it

```bash
cargo test -p webhook-receiver
cargo clippy -p webhook-receiver -- -D warnings
```

The integration tests craft a signature with a known secret and verify each
defense:

- Missing header → 400.
- Tampered signature → 400.
- Timestamp > 5 min old → 400.
- Valid signature → 200 + row inserted + `processed_at` set.
- Duplicate delivery (same `id`) → 200, no double-insert. Verified by `SELECT COUNT(*)`.
- Malformed JSON / missing fields → 400.

## File map

| File | Purpose |
|---|---|
| `migrations/...` | `stripe_events` table with UNIQUE on `stripe_event_id` |
| `src/lib.rs` | Router, `AppState`, `parse_signature_header`, `compute_signature`, `verify_signature`, `store_event`, `pending_event_id`, `mark_processed`, the `webhook` handler |
| `src/main.rs` | Binary boots the router with `STRIPE_WEBHOOK_SECRET` from env |
| `tests/webhook.rs` | 9 integration tests covering every defense, incl. crash-then-resume |

## How the signature is verified

Stripe's format is `t=<timestamp>,v1=<hex>`. To verify:

```
expected = hex( HMAC-SHA-256( secret, "{timestamp}.{raw_body}" ) )
allow if expected == provided && |now - timestamp| < 5 minutes
```

The comparison is constant-time to defeat timing attacks.

## The idempotency contract

```sql
INSERT INTO stripe_events (stripe_event_id, event_type, created_at_stripe, payload)
VALUES (?, ?, ?, ?)
ON CONFLICT (stripe_event_id) DO NOTHING
RETURNING id;
```

If the event id already exists, the `INSERT` is a no-op and `RETURNING id`
yields zero rows. The handler observes the empty result and returns 200
without processing — Stripe stops retrying.

## Why no event dispatching here?

Phase 8 is about *primitives*. The lessons describe dispatching by event
type (`checkout.session.completed`, `invoice.paid`, etc.); the actual
domain handlers ship in the MemberClub capstone from Phase 9 onward.

This project's job is to prove that the *foundation* — signature +
idempotency + storage — is bulletproof. Everything above sits on top.
