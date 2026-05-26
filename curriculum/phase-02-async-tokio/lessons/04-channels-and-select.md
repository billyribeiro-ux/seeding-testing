# Lesson 2.4 — Channels and `select!`

> **Concept first:** channels are how tasks talk to each other without sharing memory. Tokio gives you four flavours; pick the right one and most concurrency bugs vanish.
> **Time:** 30 minutes.

## Why channels?

Two tasks need to coordinate. Option A: shared memory with a `Mutex`. Works, but lock contention and tricky ownership. Option B: pass messages through a channel. Each task owns its data; messages move ownership across.

The Go-style mantra: **"Don't communicate by sharing memory; share memory by communicating."** Rust takes it literally — channels move owned values from sender to receiver.

## The four flavours in `tokio::sync`

| Channel | When |
|---|---|
| `mpsc` (multi-producer, single-consumer) | Many producers, one consumer. The workhorse. |
| `oneshot` | One sender, one receiver, single value. Perfect for "reply with the result." |
| `broadcast` | One sender, many receivers, every receiver sees every message. Pub/sub-ish. |
| `watch` | One sender, many receivers, only the *latest* value matters. Config / status. |

### `mpsc::channel`

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<u32>(100);   // 100-message buffer

    for i in 0..3 {
        let tx = tx.clone();                        // clone the sender per producer
        tokio::spawn(async move {
            tx.send(i).await.unwrap();
        });
    }
    drop(tx);                                       // drop the original so the channel closes

    while let Some(v) = rx.recv().await {
        println!("got {v}");
    }
}
```

Notes:

- Senders are `Clone`. Receivers are not — there can be only one.
- `recv()` returns `None` when *every* sender has been dropped.
- `send()` is async; it blocks the task if the buffer is full (backpressure!).

### `oneshot::channel`

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx, rx) = oneshot::channel::<&'static str>();

    tokio::spawn(async move {
        tx.send("done").unwrap();
    });

    println!("{}", rx.await.unwrap());
}
```

Use when you spawn a task and need *one* answer back. We use this for "request a value, reply with it" patterns: handler → background worker → handler.

### `broadcast::channel`

Every receiver sees every message. Good for "notify everyone" patterns:

```rust
use tokio::sync::broadcast;
let (tx, mut rx1) = broadcast::channel::<&'static str>(8);
let mut rx2 = tx.subscribe();
tx.send("ping").unwrap();
assert_eq!(rx1.recv().await.unwrap(), "ping");
assert_eq!(rx2.recv().await.unwrap(), "ping");
```

If a slow receiver falls behind, it gets a `Lagged` error. By design — you don't want a slow consumer to keep memory growing forever.

### `watch::channel`

Each receiver only ever sees the *latest* value:

```rust
use tokio::sync::watch;
let (tx, rx) = watch::channel("starting");
tx.send("running").unwrap();
assert_eq!(*rx.borrow(), "running");
```

Use for: feature flags, config reloads, "is the server still healthy" booleans.

## `select!` — wait on multiple things

`tokio::select!` evaluates several futures in parallel and runs the branch of *whichever finishes first*. The other branches are *cancelled*.

```rust
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<&str>(8);
    let t = tokio::spawn(async move {
        sleep(Duration::from_millis(200)).await;
        tx.send("hello").await.unwrap();
    });

    tokio::select! {
        Some(msg) = rx.recv() => println!("got {msg}"),
        _ = sleep(Duration::from_millis(500))    => println!("timed out"),
        _ = tokio::signal::ctrl_c()              => println!("ctrl-C"),
    }
    let _ = t.await;
}
```

Three rules to learn:

1. **The future from the *unselected* branches is dropped (cancelled).** Make sure your code is cancellation-safe (Lesson 2.5).
2. **You can add patterns and `if` guards** just like in a normal `match`.
3. **`select!` is in a *loop* for long-running event loops.** Stream messages until you see a shutdown signal.

## A worked pattern: bounded request fan-in

A worker that processes requests from a channel and shuts down on ctrl-C:

```rust
use tokio::sync::mpsc;

async fn worker(mut rx: mpsc::Receiver<Request>) {
    loop {
        tokio::select! {
            biased;                                 // check ctrl_c first each iteration
            _ = tokio::signal::ctrl_c() => break,
            Some(req) = rx.recv() => handle(req).await,
            else => break,                          // rx closed
        }
    }
}
```

`biased;` forces `select!` to check branches in *order* instead of randomly — useful when shutdown must take priority over new work.

## Why this matters

- **Channels eliminate whole classes of bugs.** Two tasks never share a mutable reference; they exchange owned values.
- **Backpressure for free.** A bounded `mpsc` channel makes producers wait when the consumer is overwhelmed. That's exactly what you want under load.
- **`select!` is the secret to clean shutdown.** Once you internalize racing real work against a shutdown signal, services become easy to operate.

## Green-bar checkpoint

- You can sketch a producer/consumer pipeline with `mpsc`.
- You can pick the right flavour (mpsc / oneshot / broadcast / watch) for "one task waits for one answer from another."
- You can write a `select!` loop that exits on ctrl-C.

Next: `lessons/05-cancellation-and-timeouts.md`.
