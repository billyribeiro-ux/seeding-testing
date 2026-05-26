# Phase 2 — Async and Tokio

> **Audience:** you finished Phase 1.
> **Outcome:** you understand `async`/`await`, can spawn concurrent tasks with Tokio, and have shipped a concurrent HTTP client with tests.
> **Time:** 1–2 weeks.

## The mental model

A web server doesn't sit there waiting for each user one at a time. When user A is downloading a 5 MB image, the server keeps serving users B, C, D — and only goes back to A when more bytes are ready. That's **async**: cooperative multitasking on a single thread (or a small pool), with the runtime juggling thousands of "paused recipes" so the CPU is never idle.

Two anchor metaphors we'll lean on the whole phase:

1. **A `Future` is a paused recipe.** A value that *will eventually* produce a result. Until you `.await` it, nothing happens.
2. **The runtime is a chef rotating between paused recipes.** When recipe A says "I'm waiting on the oven," the chef walks over to recipe B for a while. When the oven beeps, A resumes.

The chef in our kitchen is **Tokio** — the dominant async runtime for Rust, the one Axum, sqlx, reqwest, and async-stripe all build on.

## The phase plan

| Lesson | Concept | What you'll type |
|---|---|---|
| `lessons/01-what-is-async.md` | Concurrency vs parallelism. Why async exists. | Tokio "hello, world" |
| `lessons/02-futures-and-await.md` | The `Future` trait, `.await`, when nothing runs | Manually poll a future |
| `lessons/03-tokio-spawn-and-join.md` | `tokio::spawn`, `JoinHandle`, joining many tasks | Spawn 10 tasks, collect results |
| `lessons/04-channels-and-select.md` | `mpsc`, `oneshot`, `broadcast`, `select!` | Build a producer/consumer pipeline |
| `lessons/05-cancellation-and-timeouts.md` | Cancellation safety, `tokio::time::timeout`, graceful shutdown | Race a slow task against a deadline |
| `lessons/06-shared-state.md` | `Arc<T>`, `tokio::sync::Mutex/RwLock/Semaphore` | Share a counter across tasks safely |
| `lessons/07-build-quote-generator.md` | Capstone walk-through | Build `projects/02-quote-generator` |

## The capstone — `projects/02-quote-generator`

A real, small concurrent HTTP client that:

- Accepts a list of URLs on the command line *or* from a file.
- Fetches them **concurrently** with `reqwest`, with a per-request timeout and an overall deadline.
- Prints each result as soon as it arrives (out-of-order is fine; that's the point).
- Returns proper exit codes: `0` if every URL succeeded, `1` if any failed, `2` for bad input.
- Has unit tests for the pure logic and **integration tests against `wiremock`** (a real local HTTP server) so we don't depend on the public internet.

By the end of this phase, you'll have shipped a clippy-pedantic-clean, fully-tested concurrent CLI that demonstrates every async building block we'll use in Axum.

## Green-bar checkpoint

```bash
cd projects/02-quote-generator
cargo test                                  # unit + integration green
cargo clippy -- -D warnings                 # clean
cargo build --release
./target/release/quote-generator \
    --concurrency 4 --timeout 2s --deadline 10s \
    https://api.quotable.io/random
```

…and `make verify` passes at the workspace root.

## What's next

Phase 3 — **SQL + Databases**. We meet Drizzle/SQLite as the friendly on-ramp, then graduate to Postgres + sqlx for the production stack.
