# Lesson 2.6 — Shared State with `Arc`, `Mutex`, `RwLock`, `Semaphore`

> **Concept first:** channels are the *first* answer to shared state. Sometimes you really do need shared mutable data; `Arc<Mutex<T>>` is the standard cure.
> **Time:** 30 minutes.

## `Arc<T>` — shared ownership

`Arc<T>` (Atomically-Reference-Counted) is a pointer multiple tasks can hold. Cloning it bumps an atomic refcount; dropping the last clone frees the underlying `T`.

```rust
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);
let a = data.clone();
let b = data.clone();
// data, a, b all point to the same Vec on the heap
```

`Arc` gives you **shared *read* access** to `T`. You can't mutate through an `Arc<T>` directly — that's where the synchronization primitives come in.

> `Rc<T>` (without the "A") is the single-threaded version. We never use it in async code because tasks may move across threads.

## `tokio::sync::Mutex<T>` — async exclusion

A `Mutex` lets *one* task hold a mutable reference at a time. `tokio::sync::Mutex` is the async version: `.lock().await` yields to the runtime if the mutex is contended, rather than blocking the OS thread.

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();
    for _ in 0..10 {
        let counter = counter.clone();
        handles.push(tokio::spawn(async move {
            let mut n = counter.lock().await;
            *n += 1;
        }));
    }
    for h in handles { let _ = h.await; }
    println!("{}", *counter.lock().await);          // 10
}
```

Two rules:

1. **Drop the guard as soon as possible.** The `MutexGuard` returned by `.lock().await` deref-coerces to `&mut T`. Don't hold it across more `.await`s than absolutely necessary.
2. **Never hold a guard across an `await` that could call back into the same lock.** Deadlock.

When the critical section is short (a single field assignment), prefer **`std::sync::Mutex`** — it's faster, and the cost of "blocking" the runtime thread for ~10 ns is negligible. Use `tokio::sync::Mutex` when the critical section actually contains awaits (e.g. database queries inside the lock).

## `tokio::sync::RwLock<T>` — many readers, one writer

Same idea as the Rust borrow rule, at runtime: many `read()` guards can coexist, but `write()` is exclusive.

```rust
use tokio::sync::RwLock;
let cfg = Arc::new(RwLock::new(Config::default()));

// reader (many concurrent)
let r = cfg.read().await;
println!("{}", r.timeout_ms);

// writer (exclusive)
let mut w = cfg.write().await;
w.timeout_ms = 1500;
```

Default to `Mutex`. Reach for `RwLock` when *reads massively dominate writes* and the critical section is non-trivial. Otherwise the extra overhead doesn't pay off.

## `tokio::sync::Semaphore` — bounded concurrency

A `Semaphore` issues N "permits." Tasks acquire one, do work, release on drop.

```rust
use tokio::sync::Semaphore;
let limit = Arc::new(Semaphore::new(8));            // at most 8 in flight
for url in urls {
    let permit = limit.clone().acquire_owned().await.unwrap();
    tokio::spawn(async move {
        let _permit = permit;                       // released on drop
        let _ = reqwest::get(&url).await;
    });
}
```

We use semaphores constantly:

- **Concurrent HTTP calls** with a fan-out cap (e.g. "don't hammer Stripe with more than 4 in-flight requests").
- **Database pool back-pressure** (sqlx pools already include this internally, but you might layer one on top for *application*-level limits).
- **Rate limits** combined with a clock.

## Atomics — when you don't need a lock

For simple counters and flags, atomics in `std::sync::atomic` are lock-free and fast:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

let count = Arc::new(AtomicU64::new(0));
let c = count.clone();
tokio::spawn(async move { c.fetch_add(1, Ordering::Relaxed); });
```

`Ordering::Relaxed` is fine for counters. Bring `Ordering::Acquire`/`Release` only when you need ordering guarantees between threads — and read a memory-model primer before you do.

## `OnceCell` and `Lazy` — initialized once

```rust
use tokio::sync::OnceCell;
static POOL: OnceCell<sqlx::PgPool> = OnceCell::const_new();

async fn pool() -> &'static sqlx::PgPool {
    POOL.get_or_init(|| async {
        sqlx::PgPool::connect("postgres://…").await.unwrap()
    }).await
}
```

The standard pattern for "one-time async initialization of an expensive resource."

## `Send` and `Sync` — the markers behind the scenes

- **`Send`** — safe to *move* across thread boundaries. Almost every type is `Send`.
- **`Sync`** — safe to *share by reference* across threads (i.e. `&T` is `Send`). Types behind `Arc` need to be `Sync`.

`Rc<T>`, `RefCell<T>`, raw pointers — not `Send`/`Sync`. The compiler will reject them in `tokio::spawn` immediately. Listen to it.

## A worked pattern: shared state inside an Axum handler

```rust
#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,                  // Pool is Clone + Send + Sync internally
    metrics: Arc<Metrics>,             // counters
}

async fn handler(State(s): State<AppState>) -> impl IntoResponse {
    s.metrics.requests.fetch_add(1, Ordering::Relaxed);
    let _row = sqlx::query!("SELECT 1 as v").fetch_one(&s.db).await.unwrap();
    "ok"
}
```

`AppState` is cheap to clone (just an `Arc`/`Pool`); each handler call gets its own clone. No locking, because everything inside is either internally synchronized (`PgPool`) or atomic (`AtomicU64`).

## Why this matters

- **Most "shared state" turns out to be cache + atomics**, not a lock-heavy data structure.
- **Channels eliminate locks where you don't need them.** Reach for `mpsc` before `Mutex`.
- **`Send`/`Sync` are guarantees, not hassles.** When the compiler complains, it's stopping a real bug.

## Green-bar checkpoint

- You can write `Arc<Mutex<T>>` to share a counter across spawned tasks.
- You can pick `Mutex` vs `RwLock` vs `AtomicU64` for a given workload.
- You can read a `Send`/`Sync` compiler error and explain what it means.

Next: `lessons/07-build-quote-generator.md` — the Phase 2 capstone walk-through.
