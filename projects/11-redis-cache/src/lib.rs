//! redis-cache — Phase 11 caching primitives.
//!
//! Two patterns the lessons name explicitly:
//!
//!   1. **Single-flight `get_or_compute`** — many concurrent callers
//!      ask for the same key; only ONE actually computes the value;
//!      the rest wait for it. Stops "stampede" — a thundering herd
//!      hammering a slow upstream when a hot cache entry expires.
//!
//!   2. **Fixed-window counter rate limiter** — at most N requests per
//!      window per key, the counter expiring with the window. Postgres
//!      + Redis both implement this; here we keep the policy logic
//!      agnostic and store via a
//!      `CacheBackend` trait so the same code runs against an
//!      in-memory map in tests and Redis (or any KV) in production.
//!
//! ## The `CacheBackend` trait
//!
//! Anything that can:
//!   * `get(key) -> Option<value>`
//!   * `set(key, value, ttl)`
//!   * atomic `incr(key, by, ttl) -> count`
//! satisfies the trait. The `InMemoryBackend` provided here is good
//! enough for tests and single-process deployments; switching to
//! Redis is "swap the type at construction" without any other code
//! change — the production wire-up sketch lives in `docs/redis.md`.

pub mod rate_limit;
pub mod single_flight;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

/// The KV operations the lab depends on. Storage layer.
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// Read a value. `Ok(None)` means "key not present or expired".
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    /// Write a value. `ttl == None` means "no expiration".
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>)
    -> Result<(), CacheError>;

    /// Atomically increment a counter, returning the post-increment
    /// value. Sets the TTL only on the first call (when the key is
    /// created); subsequent INCRs don't extend it.
    async fn incr(
        &self,
        key: &str,
        by: u64,
        first_call_ttl: Option<Duration>,
    ) -> Result<u64, CacheError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cache backend I/O: {0}")]
    Io(String),
}

/// Process-local KV with TTL — good enough for tests, and good enough
/// as the single-instance default backend.
#[derive(Default, Clone)]
pub struct InMemoryBackend {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
}

#[derive(Clone)]
struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_alive(&self, now: Instant) -> bool {
        self.expires_at.is_none_or(|t| t > now)
    }
}

impl InMemoryBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn purge_expired(map: &mut HashMap<String, Entry>) {
        let now = Instant::now();
        map.retain(|_, e| e.is_alive(now));
    }
}

#[async_trait]
impl CacheBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut map = self.inner.lock().unwrap();
        Self::purge_expired(&mut map);
        Ok(map.get(key).map(|e| e.value.clone()))
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let mut map = self.inner.lock().unwrap();
        let expires_at = ttl.map(|d| Instant::now() + d);
        map.insert(key.to_string(), Entry { value, expires_at });
        Ok(())
    }

    async fn incr(
        &self,
        key: &str,
        by: u64,
        first_call_ttl: Option<Duration>,
    ) -> Result<u64, CacheError> {
        let mut map = self.inner.lock().unwrap();
        Self::purge_expired(&mut map);
        let now = Instant::now();
        if let Some(entry) = map.get_mut(key) {
            let n: u64 = std::str::from_utf8(&entry.value)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let new = n.saturating_add(by);
            entry.value = new.to_string().into_bytes();
            Ok(new)
        } else {
            let new = by;
            map.insert(
                key.to_string(),
                Entry {
                    value: new.to_string().into_bytes(),
                    expires_at: first_call_ttl.map(|d| now + d),
                },
            );
            Ok(new)
        }
    }
}

#[cfg(test)]
mod backend_tests {
    use super::*;

    #[tokio::test]
    async fn set_get_round_trip() {
        let b = InMemoryBackend::new();
        b.set("k", b"hello".to_vec(), None).await.unwrap();
        assert_eq!(b.get("k").await.unwrap().as_deref(), Some(&b"hello"[..]));
    }

    #[tokio::test]
    async fn set_with_ttl_expires() {
        let b = InMemoryBackend::new();
        b.set("k", b"v".to_vec(), Some(Duration::from_millis(20)))
            .await
            .unwrap();
        assert!(b.get("k").await.unwrap().is_some());
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(b.get("k").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn incr_starts_at_by_and_keeps_counting() {
        let b = InMemoryBackend::new();
        assert_eq!(b.incr("c", 1, None).await.unwrap(), 1);
        assert_eq!(b.incr("c", 1, None).await.unwrap(), 2);
        assert_eq!(b.incr("c", 5, None).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn incr_only_applies_ttl_on_first_call() {
        let b = InMemoryBackend::new();
        b.incr("c", 1, Some(Duration::from_millis(30)))
            .await
            .unwrap();
        // Subsequent incrs must NOT extend the TTL.
        tokio::time::sleep(Duration::from_millis(15)).await;
        b.incr("c", 1, Some(Duration::from_secs(3600)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await;
        // ~40 ms total — past the original 30 ms TTL.
        assert!(
            b.get("c").await.unwrap().is_none(),
            "second incr must not extend the TTL set by the first"
        );
    }
}
