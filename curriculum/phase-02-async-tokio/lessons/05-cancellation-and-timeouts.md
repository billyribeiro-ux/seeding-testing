# Lesson 2.5 — Cancellation, Timeouts, and Cancellation Safety

> **Concept first:** every `.await` is a potential cancellation point. Code must be safe to interrupt at any `.await`. Tokio gives you timeouts as the canonical bounding mechanism.
> **Time:** 30 minutes.

## What "cancellation" means

Async tasks can be **cancelled** — the future is dropped before completion. Common ways:

- The `JoinHandle` is `.abort()`ed.
- A `tokio::select!` arm wins; the other arms' futures are dropped.
- `tokio::time::timeout` wraps a future and drops it when the deadline hits.

When a future is dropped, execution stops at the last `.await` it reached. Any partially applied work is gone. *The rest of the function does not run.* No `finally` block (Rust doesn't have those), no automatic cleanup of in-memory state — just `Drop` for each owned value.

## Cancellation safety

A future is **cancellation-safe** if dropping it mid-`.await` leaves the system in a consistent state. The contract you should design for:

> *If this future is dropped, no observable side effect should be half-applied.*

Cancellation-safe by construction:

- Reads from a channel (`rx.recv().await`). Drop it: you lose the value-in-transit if any, but the channel is still fine.
- Sleeps (`time::sleep`). Drop it: nothing happened anyway.
- HTTP reads (`reqwest::Response::bytes().await`). Drop it: the connection might close; the *server* may or may not have completed its work, but our process state is clean.

Cancellation-unsafe (be careful):

- A function that performs *two* writes with an `.await` between them — if cancelled between, the first write is committed but the second never happens.
- A `Mutex` taken with `lock().await` *while we are also doing a write*. If the future is cancelled, the lock is released (Drop), but if the write was supposed to clear some state, that state is now half-applied.

**The fix is almost always the same:** keep "do the side effect" calls between two `.await`s as short as possible, and prefer transactions in databases so the DB enforces atomicity.

## `tokio::time::timeout`

```rust
use tokio::time::{timeout, Duration};

let res = timeout(Duration::from_secs(2), slow_thing()).await;
match res {
    Ok(value)  => println!("got: {value:?}"),
    Err(_elapsed) => println!("timed out"),
}
```

Use this for *every* outbound call you don't control: HTTP, DNS lookups, third-party APIs. A 200 ms p99 budget left to drift becomes a 30 s tail-latency disaster.

## `tokio::time::sleep` vs `std::thread::sleep`

| You wrote… | What happens |
|---|---|
| `tokio::time::sleep(d).await` | Yields to the runtime; other tasks run for `d`. |
| `std::thread::sleep(d)` | Holds the OS thread; *every task on that thread* stalls. |

Never use `std::thread::sleep` in an async function. Linters and `tokio::time` will save you, but train your eye to spot it in reviews.

## Cancellation tokens

For larger systems where many tasks must shut down together, Tokio offers `CancellationToken` (in `tokio-util`):

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();
let child = token.clone();

let task = tokio::spawn(async move {
    tokio::select! {
        _ = child.cancelled() => { /* clean up */ }
        _ = work() => {}
    }
});

token.cancel();        // signals all clones
let _ = task.await;
```

Use this in services that have a real graceful-shutdown story (Axum app, background workers).

## Graceful shutdown shape

The pattern we'll use in MemberClub:

```rust
let token = CancellationToken::new();

// 1. Spawn the work
let server = {
    let token = token.clone();
    tokio::spawn(async move { run_axum(token).await })
};

// 2. Wait for either ctrl-C or the server exiting
tokio::select! {
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("ctrl-C; draining…");
        token.cancel();                                  // tell everyone to wrap up
    }
    res = &mut server => {                               // can't use server directly; pin first
        tracing::info!(?res, "server exited");
    }
}

// 3. Give in-flight work a deadline
let _ = tokio::time::timeout(Duration::from_secs(30), server).await;
```

Three steps: signal cancellation, give time to drain, force-stop if too slow.

## Common cancellation pitfalls

1. **Mutex guard held across `.await`.** If the task is cancelled, the guard is dropped, but if there's any logical invariant that needed to be re-established after the `.await`, it isn't.
2. **`recv().await` *outside* a `select!` cannot be cancelled by another future.** If you want a cancellable wait, put the `recv` inside a `select!` with a cancel branch.
3. **Network writes that aren't idempotent.** If you send an HTTP request and get cancelled mid-response, the server may have committed the change. Always design third-party calls to be idempotent (idempotency keys, etc.).

## Why this matters

- **Cancellation is real.** Webhooks, timeouts, user-initiated cancels — your code will be dropped mid-flight in production. Plan for it.
- **Timeouts are mandatory on outbound I/O.** No timeout = unbounded latency.
- **Cancellation safety is a design property.** Sprinkling `.await` willy-nilly into a critical section is a hidden bug; keep critical sections short and atomic.

## Green-bar checkpoint

- You can wrap a slow `reqwest::get(url)` call in a `timeout`.
- You can articulate why holding a `Mutex` across `.await` is risky.
- You can shape a service's graceful shutdown using `select!`, `CancellationToken`, and a final `timeout`.

Next: `lessons/06-shared-state.md`.
