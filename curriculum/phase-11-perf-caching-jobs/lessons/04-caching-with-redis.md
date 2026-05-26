# Lesson 11.4 — Caching with Redis

> **Concept first:** caching trades freshness for speed. Pick the wrong
> staleness window and you serve stale invoices; pick the right one and
> you serve sub-millisecond reads.
> **Time:** 20 minutes.

## Three patterns

| Pattern | Read | Write | Use |
|---|---|---|---|
| **Cache-aside** (lazy) | Try cache; on miss, fetch + store + return | Invalidate or skip | Default. Read-heavy data. |
| **Write-through** | Always cache | Write goes to cache *and* DB | Latency-sensitive writes; small data |
| **Write-behind** | Always cache | Write to cache; DB later | Counters, batched updates |

We use **cache-aside** in MemberClub. Simple, correct, gives us a clear
"how stale can this be?" boundary per cache entry.

## Cache-aside in Rust

```rust
async fn get_user(redis: &redis::Client, pool: &PgPool, id: i64) -> Result<User, AppError> {
    let key = format!("user:{id}");
    let mut conn = redis.get_multiplexed_async_connection().await?;

    // 1. Try cache.
    if let Ok(Some(s)) = redis::cmd("GET").arg(&key).query_async::<Option<String>>(&mut conn).await {
        if let Ok(u) = serde_json::from_str::<User>(&s) {
            return Ok(u);
        }
    }

    // 2. Miss — read from DB.
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id).fetch_one(pool).await?;

    // 3. Populate cache with a 5-minute TTL.
    let _: Result<(), _> = redis::cmd("SET")
        .arg(&key).arg(serde_json::to_string(&user)?).arg("EX").arg(300)
        .query_async(&mut conn).await;

    Ok(user)
}
```

Three details worth dwelling on:

- **TTL is mandatory.** Without it the cache fills until you run out of
  RAM.
- **JSON serialization** is convenient; consider `bincode` or `postcard`
  for hot paths.
- **Ignore Redis errors gracefully.** A Redis blip should not fail the
  request — fall through to the DB.

## Invalidation strategies

The two hard problems in computer science are naming things and cache
invalidation. Pick the easiest one for your use case:

| Strategy | When |
|---|---|
| **TTL** (time to live) | Acceptable staleness window known up front. Default. |
| **Delete-on-write** | Mutating endpoint deletes the relevant key. Strong consistency, complex. |
| **Versioned keys** | `user:42:v3` with a bumped version on writes. Stale entries expire naturally. |
| **Tag invalidation** | Group keys by tag; flush by tag on write. Redis 7.4 supports this via `FUNCTION` or via an application-level convention. |

For MemberClub's `users` cache: **TTL of 5 minutes**. The tier projection
is acceptable to be 5-minutes stale (it's used for content gating, not
billing).

For the `sessions` cache: **delete-on-revoke**. Logout must be immediate.

For `subscriptions` mirror: **TTL of 1 minute + delete on webhook**. Fast
recovery from staleness, immediate response to Stripe state changes.

## Single-flight (stampede protection)

When 1000 simultaneous requests hit a cache miss for the same key, you
don't want 1000 simultaneous DB queries. Use a *single-flight* pattern:

```rust
// First request acquires a lock; the rest wait and read the populated cache.
let lock_key = format!("lock:{key}");
let acquired: Option<String> = redis::cmd("SET").arg(&lock_key).arg(&request_id)
    .arg("NX").arg("EX").arg(5).query_async(&mut conn).await?;
if acquired.is_some() {
    let value = compute_expensive_thing().await?;
    redis::cmd("SET").arg(&key).arg(&value).arg("EX").arg(300).query_async(&mut conn).await?;
    redis::cmd("DEL").arg(&lock_key).query_async(&mut conn).await?;
    Ok(value)
} else {
    // Brief sleep + retry the cache.
    tokio::time::sleep(Duration::from_millis(50)).await;
    get_user(redis, pool, id).await
}
```

For hot keys this is the difference between "10 ms" and "the DB melted."

## Counters and atomic ops

Redis is a *terrible* cache and a *fantastic* counter store. The
`INCR`/`DECR` ops are atomic; race conditions can't exist.

Use cases:

- **Rate limits.** `INCR ratelimit:user:42:1716800000`, check value.
- **Active session counts.** `SADD sessions:user:42 <session_id>`.
- **Job queue depth gauges.** `LLEN job_queue`.
- **Idempotency counters.** Already in our webhook receiver via DB; Redis
  is faster if you can lose data on restart.

## Cache key naming

Boring but important:

```
user:42                   - user record
user:42:tier              - tier projection (denormalized)
session:<token_hash>      - session lookup
sub:<stripe_id>           - subscription mirror
ratelimit:ip:198.51.100.1:/auth/login:1716800000  - rate-limit bucket
```

Three rules:

- **Colon as separator.** Universal Redis convention.
- **Smallest unit first.** `user:42` not `42:user`.
- **Time-bucketed keys** for rate limits use floor-divided timestamps so
  they age out naturally.

## Why this matters

- **A well-cached service handles 10× the load on the same hardware.**
- **Cache invalidation discipline keeps staleness bounded.** TTL is the
  cheapest control; delete-on-write is the strongest.
- **Single-flight prevents cache stampedes** — a 2-line pattern that
  saves the DB.

## Green-bar checkpoint

- You can write a cache-aside helper for "get user by id."
- You can pick TTL vs delete-on-write for a given resource.
- You can sketch the single-flight pattern.

Next: `lessons/05-background-jobs.md`.
