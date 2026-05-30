# 11-redis-cache

Phase 11 caching lab. Two patterns from the lessons:

  * **Single-flight `get_or_compute`** — many concurrent callers ask
    for the same key; only one runs the compute; the rest wait. Stops
    thundering-herd hammering of slow upstreams.
  * **Fixed-window counter rate limiter** — at most N per window per
    key, atomic `INCR` + first-call-only TTL.

Both are written against a `CacheBackend` trait. The provided
`InMemoryBackend` powers the tests and runs in a single-process
deployment; swapping it for a Redis-backed implementation is the
intended exercise.

## Why a trait?

Because we don't want every test to need a running Redis. The same
policy code (rate limit, single-flight) executes against:

  * `InMemoryBackend` — fast, in-process, perfect for `cargo nextest`.
  * `RedisBackend` — production. Uses `SETNX` + `EXPIRE` for locks
    and `INCR` + first-time `EXPIRE` for counters. A reference
    implementation lives in `docs/redis.md` (not committed here so the
    workspace stays Redis-dep-free).

## Tests (10)

Backend: `set_get_round_trip`, `set_with_ttl_expires`,
`incr_starts_at_by_and_keeps_counting`,
`incr_only_applies_ttl_on_first_call`.

Single-flight: `cache_hit_skips_compute`,
`concurrent_misses_only_compute_once` (10 concurrent callers,
exactly 1 compute fires).

Rate limit: `first_n_calls_allowed_n_plus_one_is_limited`,
`separate_keys_have_independent_buckets`,
`window_expiry_resets_the_bucket`.

`cargo test -p redis-cache` — 10 passing, no Redis required.
