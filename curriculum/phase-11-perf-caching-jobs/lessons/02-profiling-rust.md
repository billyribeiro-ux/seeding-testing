# Lesson 11.2 — Profiling Rust Services

> **Concept first:** four tools cover most of Rust performance work.
> Each gives you a different view; pick the right one for the question.
> **Time:** 20 minutes.

## The four tools

| Tool | Tells you |
|---|---|
| **`cargo flamegraph`** | CPU time: which functions burn cycles |
| **`tokio-console`** | Async tasks: who's blocked, what they're waiting on |
| **`pprof-rs` + `pprof`** | Allocations, on-CPU profiling integrated into your service |
| **`tracing` flame view** | Per-request hot path via spans (free; already installed) |

A senior engineer reaches for **traces first** because they show user-visible
latency. Flamegraphs and tokio-console are for the deeper "why is this CPU
bound?" or "why is this task starving?"

## `cargo flamegraph`

```bash
cargo install flamegraph
cargo flamegraph --bin notes-api
# generates flamegraph.svg
```

Renders a stack of stacks. Wider boxes = more CPU. Look at the top of the
tallest stack — that's the function eating your CPU.

Needs a CPU profiler (perf on Linux, dtrace on macOS). Requires root or
`/proc/sys/kernel/perf_event_paranoid <= 1` on Linux.

## `tokio-console`

```toml
# Cargo.toml
tokio = { version = "1.51", features = ["tracing"] }
console-subscriber = "0.4"
```

```rust
console_subscriber::init();
```

Then in a separate terminal:

```bash
cargo install tokio-console
tokio-console http://127.0.0.1:6669
```

A TUI that lists tasks, mutexes, polls. You can see *which task is busy*,
*which is waiting on which lock*, *which one is leaking polls*.

Use it when you suspect:
- An async task is blocked on something it shouldn't be.
- The runtime is single-task-bound (one task hogging a worker thread).
- A `Mutex` is contended.

## `tracing` flame view

If you've instrumented per Phase 10, you already have spans. Tempo /
Jaeger / Honeycomb render them as flame graphs:

```
http GET /v1/notes       105ms
├── list_notes            5ms
└── sqlx::query_as       100ms ← here's your bottleneck
```

The 100ms is in the DB. Time to `EXPLAIN ANALYZE` (Lesson 11.3).

## `pprof-rs`

For an in-service profiler you can enable per-request, behind a flag:

```rust
let guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)
    .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    .build()
    .unwrap();
// ... run the workload ...
let report = guard.report().build().unwrap();
let file = std::fs::File::create("flamegraph.svg").unwrap();
report.flamegraph(file).unwrap();
```

Useful in production behind a feature flag — you can capture a 30-second
sample on a misbehaving pod.

## CPU vs IO

A flat flamegraph (no tall stacks; lots of breadth) means *you're
IO-bound*. Look at traces, not CPU.

A flamegraph with one tall narrow stack means *you're CPU-bound*. Optimize
the hot function.

A "tower of nothings" — lots of tokio internals at the top — usually means
your code isn't on the CPU at all (waiting on async IO). Flamegraph isn't
the right tool; tokio-console is.

## What "fast" means

| Operation | Realistic time |
|---|---|
| In-memory hash lookup | ~10 ns |
| Atomic increment | ~5 ns |
| Mutex lock (uncontended) | ~25 ns |
| L3 cache miss | ~100 ns |
| Branch misprediction | ~10 ns |
| Memory allocation | ~100 ns |
| `sqlx::query!` round trip (localhost) | ~500 µs |
| `sqlx::query!` round trip (cross-AZ) | ~1.5 ms |
| HTTPS request to a third party | 50–200 ms |
| Cold start of a serverless function | 100–500 ms |

Know these numbers. They are the order-of-magnitude grid you reason on.

## Why this matters

- **Profile, don't guess.** Programmers' intuitions about where the time
  goes are wrong 4 times out of 5.
- **The right tool depends on the question.** Use traces for end-to-end
  latency, flamegraph for CPU, tokio-console for async pathology.
- **Latency numbers compound.** Three 50ms calls in series = 150ms.
  In parallel = 50ms. Knowing which you're doing is half of perf.

## Green-bar checkpoint

- You can pick a tool for "this endpoint is slow but the CPU is idle."
- You can read a flamegraph and identify the hotspot.
- You can recite the latency order-of-magnitude table from memory.

Next: `lessons/03-db-perf.md`.
