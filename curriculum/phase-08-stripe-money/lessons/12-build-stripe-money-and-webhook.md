# Lesson 8.12 — Build `stripe-money-lab` and `webhook-receiver`

> **The two capstones of Phase 8.** Take a slow walk through both projects.
> **Time:** 90 minutes.

## Capstone 1 — `projects/06-stripe-money-lab`

A pure-Rust money primitive library. 25 tests (19 unit + 6 proptest).

### Files

```
src/lib.rs          ← Money, Currency, MoneyError, MONEY_CEILING_CENTS
                       new, zero, checked_add/sub/mul, split_proportional
                       Display impl + #[cfg(test)] modules
```

### The four invariants worth memorizing

1. **`Money::new` refuses values >= the ceiling.** No way to construct an
   out-of-bounds value.
2. **No `Add`/`Sub` operator impls.** Callers use `checked_add` etc., so
   overflow can't be silently ignored.
3. **Currency mismatch is a typed error** — distinct from overflow.
4. **`split_proportional` sums *exactly* to the original.** Proven by
   proptest for thousands of inputs.

### The largest-remainder split, in code

```rust
let cents = i128::from(self.cents);
let abs = cents.unsigned_abs();
let sign: i64 = if cents < 0 { -1 } else { 1 };

let mut shares: Vec<i64>      = Vec::with_capacity(weights.len());
let mut remainders: Vec<(usize, u128)> = Vec::with_capacity(weights.len());
let mut allocated_abs: u128 = 0;
for (i, &w) in weights.iter().enumerate() {
    let num = abs * u128::from(w);
    let q = num / total_weight;
    let r = num % total_weight;
    allocated_abs += q;
    shares.push((q as i64) * sign);
    remainders.push((i, r));
}

// Distribute the residue, one cent at a time, to the largest remainders.
let residue: u128 = abs - allocated_abs;
remainders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));   // stable
for k in 0..residue as usize {
    let (idx, _) = remainders[k % remainders.len()];
    shares[idx] = shares[idx].saturating_add(sign);
}
```

Three details worth dwelling on:

- **`u128` for intermediate products.** `i64 * u64` can overflow `i64`;
  promoting to `u128` is safe and fast.
- **`sort_by` with a tiebreaker (`a.0.cmp(&b.0)`)** keeps splits
  deterministic — same inputs produce same shares every run.
- **`saturating_add(sign)`** instead of `+= sign` makes the inner loop
  panic-free.

### The proptest that catches the corner cases

```rust
proptest! {
    #[test]
    fn split_sums_to_original(
        cents in 1i64..1_000_000_000,
        ws in proptest::collection::vec(1u64..1000, 1..10),
    ) {
        let m = Money::new(cents, Currency::USD).unwrap();
        let parts = m.split_proportional(&ws);
        let total: i64 = parts.iter().map(|p| p.cents).sum();
        prop_assert_eq!(total, cents);
    }
}
```

Run this with 10,000 random inputs. If any one fails, you have an off-by-one
in the split. Mine eventually surfaced via this exact test — the original
code had a subtle bug when `weights.len() < residue`.

## Capstone 2 — `projects/07-webhook-receiver`

An Axum service receiving Stripe-style signed webhooks. 8 integration tests.

### Files

```
src/lib.rs                Router, signature, idempotency, handler
src/main.rs               binary boots with STRIPE_WEBHOOK_SECRET
migrations/*.sql          stripe_events table with UNIQUE on event id
tests/webhook.rs          8 tests verifying each defense
```

### The three defenses, in order

1. **Verify signature** — HMAC-SHA-256 of `"{timestamp}.{raw_body}"` against
   the secret. Constant-time comparison. Reject if mismatch.
2. **Verify timestamp** — reject if older than 5 minutes (replay window).
3. **Idempotent insert** — `INSERT ... ON CONFLICT DO NOTHING RETURNING id`.
   `Some(id)` → new; process. `None` → duplicate; 200.

### The signature compute

```rust
pub fn compute_signature(secret: &[u8], timestamp: u64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}
```

The signed payload is *the timestamp dot the raw body* — not the parsed
JSON. This matters: any JSON pretty-printing in the middle would break the
signature.

### The "verify before parse" rule, again

```rust
async fn webhook(State(s): State<AppState>, headers: HeaderMap, body: Bytes)
    -> Result<StatusCode, WebhookError>
{
    verify_signature(&headers, &body, &s.secret, s.max_skew)?;   // FIRST
    let meta = parse_event_meta(&body)?;                          // THEN
    /* ...idempotent insert + dispatch... */
}
```

If we parsed first, malformed JSON would tell an attacker we *attempted* to
parse — leaking that the endpoint is "interesting." Verify, then parse.

### The duplicate-delivery test that's the soul of this project

```rust
#[tokio::test]
async fn duplicate_delivery_is_idempotent() {
    /* register event evt_5 once */
    /* deliver the same payload again — still 200 */
    /* deliver with a different timestamp (signature) but same event id — still 200 */
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stripe_events WHERE stripe_event_id = 'evt_5'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "exactly one row regardless of retries");
}
```

If this test passes, your webhook pipeline survives the chaos Stripe will
throw at it.

## How they fit together (in MemberClub)

```
stripe.com  ─POST→  webhook-receiver
                     ├── verify_signature (Phase 8.7)
                     ├── store_event (idempotency row)
                     ├── dispatch by type:
                     │   - checkout.session.completed → grant_entitlement(money_lab::Money)
                     │   - invoice.paid               → update mirror
                     │   - customer.subscription.updated → state machine
                     │   - ... etc.
                     └── mark_processed

(All money values from Stripe events go through Money::new before touching our domain.)
```

`stripe-money-lab` provides the *vocabulary* for money. `webhook-receiver`
provides the *door*. The MemberClub capstone in Phase 9 onward fills in the
domain handlers that connect the two.

## Why this matters

- **Money safety + webhook reliability** are the two non-negotiable
  properties of any subscription system.
- **Both projects ship as libraries** — they're reused in MemberClub, in
  internal admin tools, anywhere money or Stripe shows up.
- **The pattern transfers.** Any third-party that sends webhooks (Slack,
  GitHub, GitLab, Linear, …) gets the same three-defense treatment.

Phase 8 is complete. Phase 9 — **Svelte 5 / SvelteKit 2** — builds the
MemberClub web app. The backend (auth + RBAC + money + webhooks) is ready;
now we put the human face on it.
