//! Single-flight `get_or_compute`.
//!
//! The thundering-herd problem: a hot cache entry expires while a
//! thousand requests are in flight. Naïvely, every one of them sees
//! "cache miss" and calls the slow upstream. The smart fix is "only
//! one runs; the rest wait for its result."
//!
//! Implementation: an in-process map of `key -> tokio::sync::watch`.
//! The first caller for a missing key inserts a fresh watch sender,
//! runs the upstream, sends the result. Subsequent callers (within
//! the same process) clone the receiver and await.
//!
//! For multi-process deployments, the equivalent in Redis is `SETNX
//! lock-key 1 PX 5000` — only the writer that wins the lock fetches;
//! the rest poll for the cache key. The trait stays the same.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::watch;

use crate::{CacheBackend, CacheError};

/// In-process registry of in-flight computations.
#[derive(Default)]
pub struct SingleFlight {
    inflight: Mutex<HashMap<String, watch::Receiver<Option<Vec<u8>>>>>,
}

impl SingleFlight {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `get_or_compute(backend, key, ttl, compute)`:
    ///   1. Try to read `key` from the backend; return early on hit.
    ///   2. On miss, try to claim the in-flight slot. If we win,
    ///      `compute()` runs to produce the value, we write it to
    ///      the backend, and we broadcast it to anyone waiting.
    ///   3. If we lose (someone else is already computing), we wait
    ///      on their broadcast.
    pub async fn get_or_compute<F, Fut>(
        &self,
        backend: &dyn CacheBackend,
        key: &str,
        ttl: Option<Duration>,
        compute: F,
    ) -> Result<Vec<u8>, CacheError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<Vec<u8>, CacheError>> + Send,
    {
        // Step 1: cache hit?
        if let Some(v) = backend.get(key).await? {
            return Ok(v);
        }

        // Step 2: try to claim the in-flight slot.
        let claim = {
            let mut map = self.inflight.lock().unwrap();
            if let Some(rx) = map.get(key) {
                // Someone else is computing — clone their receiver and
                // wait outside the lock.
                Some(WaitKind::Wait(rx.clone()))
            } else {
                let (tx, rx) = watch::channel(None);
                map.insert(key.to_string(), rx);
                Some(WaitKind::Compute(tx))
            }
        };

        match claim.expect("we always pick a branch") {
            WaitKind::Compute(tx) => {
                let result = compute().await;
                // Always remove the slot, even on error, so callers
                // don't deadlock waiting on a watch that never fills.
                let _ = self.inflight.lock().unwrap().remove(key);
                let value = result?;
                backend.set(key, value.clone(), ttl).await?;
                let _ = tx.send(Some(value.clone()));
                Ok(value)
            }
            WaitKind::Wait(mut rx) => {
                // Loop until the sender publishes a value or drops.
                loop {
                    if let Some(v) = rx.borrow().clone() {
                        return Ok(v);
                    }
                    if rx.changed().await.is_err() {
                        // Sender dropped without publishing — fall back
                        // to reading the cache (the writer must have
                        // succeeded but lost the channel) or recursing.
                        if let Some(v) = backend.get(key).await? {
                            return Ok(v);
                        }
                        // No value materialized; pretend we never got
                        // the slot and try again from scratch.
                        return Err(CacheError::Io(
                            "single-flight peer dropped before publishing".into(),
                        ));
                    }
                }
            }
        }
    }
}

enum WaitKind {
    Compute(watch::Sender<Option<Vec<u8>>>),
    Wait(watch::Receiver<Option<Vec<u8>>>),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::InMemoryBackend;

    #[tokio::test]
    async fn cache_hit_skips_compute() {
        let backend = InMemoryBackend::new();
        backend.set("k", b"cached".to_vec(), None).await.unwrap();
        let sf = SingleFlight::new();

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_closure = calls.clone();
        let v = sf
            .get_or_compute(&backend, "k", None, move || {
                let calls = calls_for_closure.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(b"freshly-computed".to_vec())
                }
            })
            .await
            .unwrap();
        assert_eq!(v, b"cached");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn concurrent_misses_only_compute_once() {
        let backend = InMemoryBackend::new();
        let sf = Arc::new(SingleFlight::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(10);
        for _ in 0..10 {
            let sf = sf.clone();
            let backend = backend.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                sf.get_or_compute(&backend, "hot-key", None, move || {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        // Simulate a slow upstream so all the concurrent
                        // callers actually pile up.
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(b"computed-once".to_vec())
                    }
                })
                .await
            }));
        }

        for h in handles {
            assert_eq!(h.await.unwrap().unwrap(), b"computed-once");
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "stampede protection failed — compute ran more than once"
        );
    }
}
