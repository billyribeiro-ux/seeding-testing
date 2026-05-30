# Lesson 6.7 — Rate Limiting and Account Lockout

> **Concept first:** every public endpoint is a candidate for abuse. Rate limiting buys time; lockout closes the door after repeated failure.
> **Time:** 20 minutes.

## Three flavors of throttling

| Flavor | Granularity | Tool |
|---|---|---|
| **Per-IP** | Cheap; catches simple botnets | `tower-governor` (in-memory token bucket) |
| **Per-account** | Targets credential-stuffing | Redis counters per user_id |
| **Global / per-endpoint** | Protects against thundering herd | Single Redis key or queue with a max in-flight |

You typically run all three.

## Per-IP with `tower-governor`

```toml
[dependencies]
tower_governor = "0.8"
```

```rust
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;

let governor_conf = GovernorConfigBuilder::default()
    .per_second(10)              // 10 req/s sustained
    .burst_size(20)              // bursts up to 20
    .finish().unwrap();

// In tower_governor 0.8 the layer is built with `::new(...)`; the
// config is wrapped in an `Arc` for you.
let app = Router::new().route(...)
    .layer(GovernorLayer::new(governor_conf));
```

Returns `429 Too Many Requests` with a `Retry-After` header when exceeded. The default key is the client IP (via `X-Forwarded-For` behind a trusted proxy; configure carefully).

**Apply more aggressive limits to `/auth/login` and `/auth/forgot-password`** — those are the abuse targets.

## Per-account counters with Redis

```rust
// Pseudocode for a Redis-backed counter
async fn record_failed_login(redis: &Redis, account_id: i64) -> u32 {
    let key = format!("auth:fail:{account_id}");
    let count: u32 = redis.incr(&key).await?;
    if count == 1 {
        redis.expire(&key, 60 * 15).await?;          // 15-minute window
    }
    count
}
```

Then in the login handler:

```rust
let failures = record_failed_login(&redis, account_id).await;
if failures > 5 {
    return Err(ApiError::TooManyAttempts);    // 429
}
```

A 15-minute sliding window keeps an attacker to 5 attempts per quarter-hour per account — credential stuffing is dead.

## Exponential backoff per account

For lockout that doesn't permanently lock real users out:

| Consecutive failures | Lockout duration |
|---|---|
| 5 | 15 seconds |
| 6 | 1 minute |
| 7 | 5 minutes |
| 8 | 15 minutes |
| 9 | 1 hour |
| 10+ | 24 hours |

Reset the counter on successful login.

A locked-out account still *succeeds* on a correct password+TOTP — it just doesn't issue a token until the lockout expires. (Or you can be strict and reject even correct credentials during lockout; depends on your threat model.)

## Defending the verification + reset endpoints

These are spam vectors (each call may send an email). Rate-limit aggressively:

| Endpoint | Per-IP | Per-account |
|---|---|---|
| `POST /auth/forgot-password` | 10/h | 3/day |
| `POST /auth/verify-email/resend` | 10/h | 1/min |
| `POST /auth/register` | 5/h | n/a (account doesn't exist yet) |

Email providers count abusive sends against your reputation. A rate-limit failure here is *cheaper than* sending the email.

## What `429` should return

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
Content-Type: application/problem+json

{
  "type":   "https://memberclub.test/problems/rate-limited",
  "title":  "Too Many Requests",
  "status": 429,
  "detail": "Try again in 60 seconds."
}
```

Three rules:

- **`Retry-After` in seconds** — clients can wait the right amount, not guess.
- **Don't expose the limit value to the wire.** Don't say "you can do 100/min"; just "try again later." The exact value is internal.
- **Log the rate-limit hit with the actor's IP, route, and the count.** Sustained 429s are signal for an attack.

## What about distributed rate limits?

When you have multiple API instances behind a load balancer, in-memory token buckets don't share state. Two options:

- **Redis (single source of truth).** Slightly more latency per request; accurate across instances.
- **Best-effort per-instance.** Each instance enforces locally; total burst is N×limit if you have N instances. Fine if you over-provision.

We use Redis from MemberClub's Phase 11 onward.

## Why this matters

- **Login endpoints are the #1 attack target on any service that has them.** Rate limiting is the cheapest defense.
- **Exponential backoff per account beats hard lockouts** for keeping legit users in.
- **Email sending is expensive and has reputation impact** — protect those endpoints first.

## Green-bar checkpoint

- You can pick per-IP vs per-account for a given endpoint and justify the choice.
- You can sketch a Redis-backed counter with a sliding window.
- You can write a `429` problem-details body.

Next: `lessons/08-extractors-and-policies.md`.
