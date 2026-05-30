//! Fixed-window counter rate limiter, KV-backed.
//!
//! Algorithm: every key has a counter incremented per request. On the
//! first increment of a fresh window, the TTL is set to the window
//! length — the counter dies on its own and the window resets at the
//! next request after the TTL elapses. Returns `Allowed { remaining }`
//! on success or `Limited { retry_after }` once the budget is spent.
//!
//! (This is the fixed-window counter, not a token bucket: tokens do not
//! refill continuously, so a caller can fire up to `2 × max_requests`
//! across a window boundary. That burst tolerance is acceptable for the
//! login/abuse-control use here; a true token or leaky bucket is the
//! upgrade when you need smooth pacing.)
//!
//! Lesson 6.7 ("Rate Limiting and Account Lockout") reaches for
//! `tower-governor` (a GCRA / token-bucket-family limiter) for the
//! per-IP login case; this module is the simpler distributed-friendly
//! cousin — a counter + TTL is the canonical Redis-backed shape because
//! `INCR` + first-call `EXPIRE` is a two-command, lock-free recipe. The
//! trait lets the same code run against Redis in production and
//! `InMemoryBackend` in tests.

use std::time::Duration;

use crate::{CacheBackend, CacheError};

/// One outcome of an `enforce` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The request is within the budget. `remaining` is how many calls
    /// the same key can still make in this window.
    Allowed { remaining: u64 },
    /// The request would exceed the budget. The caller should respond
    /// with 429 + `Retry-After: retry_after.as_secs()`.
    Limited { retry_after: Duration },
}

/// Configuration for one rate-limit policy.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub max_requests: u64,
    pub window: Duration,
}

impl Policy {
    #[must_use]
    pub const fn per_minute(max: u64) -> Self {
        Self {
            max_requests: max,
            window: Duration::from_secs(60),
        }
    }

    #[must_use]
    pub const fn per_hour(max: u64) -> Self {
        Self {
            max_requests: max,
            window: Duration::from_secs(60 * 60),
        }
    }
}

/// Count one request against the bucket identified by `key` and the
/// `policy`'s window length.
pub async fn enforce(
    backend: &dyn CacheBackend,
    key: &str,
    policy: Policy,
) -> Result<Decision, CacheError> {
    let count = backend.incr(key, 1, Some(policy.window)).await?;
    if count <= policy.max_requests {
        Ok(Decision::Allowed {
            remaining: policy.max_requests - count,
        })
    } else {
        Ok(Decision::Limited {
            // Real Redis exposes `TTL` so the retry_after is exact; in
            // this layer the window is the conservative upper bound.
            retry_after: policy.window,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryBackend;

    #[tokio::test]
    async fn first_n_calls_allowed_n_plus_one_is_limited() {
        let b = InMemoryBackend::new();
        let p = Policy {
            max_requests: 3,
            window: Duration::from_secs(60),
        };
        for expected_remaining in (0..3).rev() {
            assert_eq!(
                enforce(&b, "user:42", p).await.unwrap(),
                Decision::Allowed {
                    remaining: expected_remaining,
                }
            );
        }
        // Fourth call trips the limit.
        assert!(matches!(
            enforce(&b, "user:42", p).await.unwrap(),
            Decision::Limited { .. }
        ));
    }

    #[tokio::test]
    async fn separate_keys_have_independent_buckets() {
        let b = InMemoryBackend::new();
        let p = Policy {
            max_requests: 1,
            window: Duration::from_secs(60),
        };
        // user:1 burns its single token.
        assert_eq!(
            enforce(&b, "user:1", p).await.unwrap(),
            Decision::Allowed { remaining: 0 }
        );
        // user:2 still has a fresh bucket.
        assert_eq!(
            enforce(&b, "user:2", p).await.unwrap(),
            Decision::Allowed { remaining: 0 }
        );
        // user:1's next call is limited.
        assert!(matches!(
            enforce(&b, "user:1", p).await.unwrap(),
            Decision::Limited { .. }
        ));
    }

    #[tokio::test]
    async fn window_expiry_resets_the_bucket() {
        let b = InMemoryBackend::new();
        let p = Policy {
            max_requests: 1,
            window: Duration::from_millis(30),
        };
        assert!(matches!(
            enforce(&b, "k", p).await.unwrap(),
            Decision::Allowed { .. }
        ));
        assert!(matches!(
            enforce(&b, "k", p).await.unwrap(),
            Decision::Limited { .. }
        ));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            matches!(enforce(&b, "k", p).await.unwrap(), Decision::Allowed { .. }),
            "after the window, the bucket must reset"
        );
    }
}
