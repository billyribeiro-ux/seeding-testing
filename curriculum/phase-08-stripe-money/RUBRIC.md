# Phase 8 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Money representation** | `f64` somewhere in the money path | `i64` cents, dedicated newtype | `Money(i64, Currency)`, no operator overloads, type-system-enforced |
| **Ceiling enforcement** | No bound | App-level check at constructor | App + DB `CHECK` constraint; constants centralized |
| **Arithmetic** | `+`/`-`/`*` silently overflows | `checked_*` returns `Result`; caller handles | All math goes through one library; proptest proves invariants |
| **Proportional split** | `total / n` floor-rounded; off-by-one | Banker's rounding | Largest-remainder method with proptest of `sum == total` |
| **Stripe data model** | Don't mirror; query Stripe live | Mirror Customer/Subscription/Invoice/Charge | Mirror is the source of truth; Stripe is the rail; nightly reconciliation |
| **Idempotency keys** | None | Pass to Stripe API | Persist BEFORE the call; deterministic key generation |
| **Webhook signature** | Trust the body | Verify HMAC after parsing | Verify *before* parsing; constant-time compare; timestamp window |
| **Webhook idempotency** | Process every delivery | UNIQUE on `stripe_event_id`; 200 duplicates | Insert + return id; mark processed after handler; test triple-replay |
| **Out-of-order tolerance** | Last write wins blindly | Compare `created` timestamps | `WHERE updated_at_stripe < event.created` guard on every UPDATE |
| **Dunning** | Immediate downgrade on `past_due` | Grace period + email + portal redirect | Smart-retries-aware; banner UX; reservation accounting |
| **Reconciliation** | None | Nightly diff + alert | Diff + sum check + run history + advisory lock |
| **PII / receipts** | Hand-rolled invoice rendering | Use Stripe-hosted PDF URL | Mirror PDF URLs; store subtotal/tax/total separately; integrity CHECKs |

## Self-check before moving to Phase 9

- [ ] `make verify` passes locally.
- [ ] You completed Exercises E8.1 – E8.5.
- [ ] You can write `Money::split_proportional` from memory and explain why
      it sums exactly.
- [ ] You can sketch the three webhook defenses and articulate which bug
      each prevents.
- [ ] You can pick `i64` cents vs `rust_decimal` vs `f64` for a given math
      operation and justify the choice.
- [ ] You can explain "Stripe is the rail; our DB is the source of truth."
- [ ] CI is green on your branch.

Phase 9 — **Svelte 5 / SvelteKit 2** — builds the MemberClub web app on top
of every backend primitive shipped so far.
