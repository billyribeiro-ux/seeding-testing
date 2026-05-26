# Lesson 2.3 — `tokio::spawn` and `JoinHandle`

> **Concept first:** `tokio::spawn` hands a future to the runtime *and returns immediately*. The handle it gives you (`JoinHandle`) lets you wait for the task or abort it.
> **Time:** 30 minutes.

## `spawn` vs `await`

| Operation | What it does |
|---|---|
| `f.await` | Run this future to completion *now*, in the current task. |
| `tokio::spawn(f)` | Detach this future into a new task. Returns a `JoinHandle`. |

Spawning gives you concurrency. Awaiting does not.

```rust
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let start = Instant::now();

    let a = tokio::spawn(async { sleep(Duration::from_secs(1)).await; 1 });
    let b = tokio::spawn(async { sleep(Duration::from_secs(1)).await; 2 });
    let c = tokio::spawn(async { sleep(Duration::from_secs(1)).await; 3 });

    let (ra, rb, rc) = tokio::join!(a, b, c);
    println!("{:?} {:?} {:?}", ra.unwrap(), rb.unwrap(), rc.unwrap());
    println!("elapsed: {:?}", start.elapsed());     // ~1 second, not 3
}
```

Three tasks ran on the same runtime, sleeping in parallel.

## `JoinHandle<T>`

`tokio::spawn` returns `JoinHandle<T>`. You can:

- **`.await` it** to wait for the task and get its result (`Result<T, JoinError>`).
- **`.abort()` it** to cancel the task (the task's next `.await` returns immediately, with a cancellation error).
- **Drop it** to detach — the task keeps running, but you can't observe it.

`JoinError` reports two failure modes: the task *panicked* (`is_panic()`) or was *cancelled* (`is_cancelled()`).

## `JoinSet<T>` — spawn many, collect as they finish

The right tool when you spawn an unknown or large number of tasks and want results in completion order:

```rust
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    let mut set: JoinSet<u32> = JoinSet::new();
    for i in 1..=5 {
        set.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100 * i as u64)).await;
            i
        });
    }
    while let Some(res) = set.join_next().await {
        match res {
            Ok(v)  => println!("done: {v}"),
            Err(e) => eprintln!("task failed: {e}"),
        }
    }
}
```

Output (completion order):

```
done: 1
done: 2
done: 3
done: 4
done: 5
```

This is the foundation we'll use in the quote-generator.

## `JoinSet` vs `FuturesUnordered`

Two options for "collect many results in completion order":

- **`JoinSet<T>`** — spawns each future on the runtime (every task gets a Tokio task). Use when each unit of work is meaningful: an HTTP request, a DB query, a long computation.
- **`FuturesUnordered<F>`** (from `futures` crate) — polls futures on the *current* task. Cheaper if you have many small futures and don't need a separate task per future.

Default to `JoinSet`. Reach for `FuturesUnordered` when profiling shows task overhead.

## Moving data into spawned tasks

The future passed to `spawn` must be `'static` (no borrowed references to anything that could go away) and `Send` (movable across threads). Practical implication:

```rust
let name = String::from("alice");
tokio::spawn(async move {
    println!("hi, {name}");          // `move` transfers ownership into the task
});
```

If you need to share data, wrap it in `Arc<T>` (Phase 2.6).

## `select!` — wait for the *first* of many futures

When you want "whichever finishes first, give me that":

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let fast = sleep(Duration::from_millis(100));
    let slow = sleep(Duration::from_secs(10));

    tokio::select! {
        _ = fast => println!("fast won"),
        _ = slow => println!("slow won"),
    }
}
```

We use `select!` constantly for racing a real task against a timeout, a cancellation signal, or a shutdown channel.

## Graceful shutdown — the canonical pattern

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = tokio::spawn(async {
        // ... run an Axum server here ...
    });

    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::info!("ctrl-C received, shutting down");
            // server.abort() or a graceful-shutdown channel here
        }
        _ = server => {
            tracing::info!("server exited");
        }
    }
    Ok(())
}
```

You'll see this exact pattern in Axum services we ship.

## Why this matters

- **`spawn` is how you turn sequential async code into concurrent async code.** Without `spawn` (or `join!`/`JoinSet`), your `.await`s are sequential.
- **Tasks are lightweight.** Spawning a million tasks is fine. Spawning a million OS threads is not.
- **`JoinSet` is the right default for fan-out.** When you have "do N HTTP calls and gather results," reach for it first.

## Green-bar checkpoint

- You can rewrite a sequential loop into a `JoinSet`-based concurrent version.
- You can articulate the difference between `spawn`, `await`, and `select!`.
- You can use `tokio::signal::ctrl_c()` + `select!` to shut down a service cleanly.

Next: `lessons/04-channels-and-select.md`.
