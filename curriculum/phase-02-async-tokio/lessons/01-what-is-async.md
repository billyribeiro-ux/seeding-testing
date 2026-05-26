# Lesson 2.1 — What "async" Means

> **Concept first:** async is cooperative multitasking. Read this carefully — the mental model is everything.
> **Time:** 30 minutes.

## Concurrency vs parallelism

Two words that get confused constantly:

- **Concurrency** = many things *in progress* at once, possibly on a single CPU. The OS or runtime juggles them.
- **Parallelism** = many things *executing* at once, on multiple CPUs.

Concurrency is *structure*. Parallelism is *execution*. Most servers want concurrency (handle 10,000 connections at once on 8 cores) more than they want parallelism (compute one thing 8× faster).

## The cost of one thread per request

The classic web-server design — one OS thread per connection — runs out of road at a few thousand connections. Each thread costs ~1 MB of stack RAM, dozens of µs to spawn, and context-switching between thousands of threads burns the CPU. That's why Node, Go, Python's asyncio, and Rust's Tokio all moved to "many lightweight tasks, few OS threads."

## How async actually works

Three pieces:

1. **A task is a state machine.** When you write `async fn`, the compiler rewrites your function into a struct that remembers where execution paused. Each `.await` point is a state in the machine.
2. **The runtime polls.** When a task is ready (e.g. the socket has bytes), the runtime calls `poll()` on it. The state machine advances to the next `.await`, or finishes.
3. **No magic threads.** Async tasks are cheap (tens of bytes, not megabytes). Tokio runs them on a small pool of OS threads (default: one per CPU).

The crucial property: **a task only yields control at an `.await` point.** Code between two `.awaits` runs straight through without interruption. No mid-statement context switches.

## A first taste

A toy Tokio program that prints "tick" and "tock" concurrently:

```rust
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() {
    let tick = tokio::spawn(async {
        for _ in 0..3 {
            println!("tick");
            time::sleep(Duration::from_millis(500)).await;
        }
    });
    let tock = tokio::spawn(async {
        for _ in 0..3 {
            time::sleep(Duration::from_millis(250)).await;
            println!("tock");
        }
    });
    let _ = tokio::join!(tick, tock);
}
```

Add to `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1.51", features = ["macros", "rt-multi-thread", "time"] }
```

Run it:

```bash
cargo run
```

Output (interleaved, one OS thread reused):

```
tick
tock
tock
tick
tock
tick
```

If those `sleep`s were blocking I/O instead, the same pattern would scale to thousands of network connections.

## What "blocking" means and why it's lethal

A *blocking* call is one that holds onto the thread without yielding. Examples: `std::thread::sleep`, `std::fs::read_to_string`, `std::net::TcpStream::read`. In async code, a blocking call freezes *every task scheduled on that thread*.

Three rules to internalize:

1. **In async code, use the async version of everything.** `tokio::time::sleep`, `tokio::fs`, `tokio::net::TcpStream`. They yield to the runtime instead of holding the thread.
2. **If you must call blocking code, use `tokio::task::spawn_blocking`.** It moves the blocking work to a separate pool so it doesn't strangle the runtime.
3. **CPU-bound code (parsing 100 MB of JSON, computing a hash, image resizing) is *also* blocking from the runtime's POV.** Same rule: `spawn_blocking`.

## `#[tokio::main]`

The macro that boots a runtime and runs your `async fn main()`. Defaults to the multi-threaded scheduler with one worker per CPU.

```rust
#[tokio::main]
async fn main() { /* ... */ }
```

You can configure it:

```rust
#[tokio::main(flavor = "current_thread")]    // single-thread runtime (useful for CLI tools)
async fn main() { /* ... */ }

#[tokio::main(worker_threads = 4)]           // four workers
async fn main() { /* ... */ }
```

Behind the scenes, the macro expands to roughly:

```rust
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { /* your code */ });
}
```

You'll see the manual form occasionally when you want fine control (libraries, test helpers).

## Why this matters

- **Servers in 2026 are async.** Axum, sqlx, reqwest, async-stripe — every crate in this curriculum is async.
- **Async is a *property of types*, not a separate world.** An `async fn` returns an `impl Future`. You can pass futures, store them, combine them — same as any other value.
- **One blocking call ruins everything.** This is the #1 source of latency mysteries in async servers. Train your eye to spot blocking calls early.

## Green-bar checkpoint

- You can articulate the difference between concurrency and parallelism.
- You can name three blocking calls and the async equivalents we should use instead.
- You can run the tick/tock example and explain why the output is interleaved.

Next: `lessons/02-futures-and-await.md`.
