# Flamegraph Walkthrough

> A flamegraph is the single most useful chart in a perf investigation,
> and the single most commonly misread one. This page is the cheat
> sheet for reading one without confusing yourself.

This walkthrough is paired with `docs/perf/methodology.md` (the
*how* of taking a measurement) and `projects/13-load-test/`
(the harness you'll point at the service while you capture).

## Mental model first

A flamegraph is a stacked bar chart of *who called whom*, aggregated
across thousands of sampled stack traces. The profiler interrupts
the CPU at a fixed rate (typically 99 Hz or 997 Hz), records the call
stack, and the flamegraph aggregates those stacks.

The biggest misconception is that flamegraphs show time on the x-axis.
**They don't.** Two stacks that look identical get merged together
regardless of when they happened. The visualization optimizes for
"what dominates the workload" — not "what happened at 14:32:01."

## The axes

| Axis | What it means | What it does NOT mean |
|---|---|---|
| **x-axis (width)** | Sample count. Wider = more samples → more CPU time. | Time of day. Stacks are sorted alphabetically inside each row. |
| **y-axis (height)** | Stack depth. Bottom frame called the one above it. | "Importance." A deep stack isn't more or less important. |
| **colors** | Usually random or by category (kernel/user/JIT). | Hot vs cold. Color rarely carries data. |

If you remember nothing else: **width is what matters**. A wide block,
anywhere on the graph, means a lot of CPU was spent in that function
(itself or its callees).

## Hot path vs. wide path

There are two distinct shapes worth naming:

- **Hot path** — a wide column that goes from bottom to top through
  a single function, ending in a wide *leaf* (e.g. `memcpy`). The CPU
  is genuinely stuck inside one specific function. The leaf is where
  the cycles go.
- **Wide path** — a wide *base* (some entry function) that *branches*
  into many narrower leaves. The entry function is called a lot, but
  the work itself is distributed across many callees. Don't optimize
  the base; pick a leaf.

The mistake we see in code review: someone screenshots the *base* of
a wide path (always `handle_request` or similar) and proposes
optimizing it. The base isn't the problem; the leaves are.

## Self time vs. total time

When you click a frame in most flamegraph viewers (e.g. `inferno`,
`pprof`, `perf report`), you get two numbers:

- **Self time** — samples whose top-of-stack was *this* function.
  This is the time the function itself was on CPU.
- **Total time** — self time + all descendants. This is the time the
  function or anything it called was on CPU.

Rule of thumb:

- A function with high **self time** is doing work *itself*. Profile
  inside it — it's the leaf.
- A function with high **total time** but low **self time** is calling
  expensive things. Profile its callees — it's just the path.

If `handle_request` shows 80% total time and 2% self time, optimizing
`handle_request` itself does almost nothing. Find the callee that
holds the other 78%.

## Five common patterns

These are the shapes that come up over and over in Rust services.

### 1. Serde-heavy JSON

```text
handle_request ───────────────────────────────────────┐
└─ serde_json::to_writer ─────────────────────────────┤
   └─ <Vec<Note> as Serialize>::serialize ────────────┤
      └─ <Note as Serialize>::serialize ──────────────┤
         └─ serde::ser::SerializeStruct::serialize_field
```

A tall, narrow tower of `Serialize` impls dominating the leaves.
Common cure: stop returning the whole table (see
`docs/perf/notes-api-2026-05-26.md`), use `Cow<'_, str>` for fields
that are usually small, or switch to a streaming serializer.

### 2. Lock contention

```text
tokio::runtime::worker ──────────────────────────────┐
└─ futures::lock::Mutex::lock ────────────────────────┤
   └─ parking_lot::raw_mutex::lock_slow ──────────────┤
      └─ syscall(futex, WAIT) ────────────────────────┘
```

A wide block ending in `futex_wait` (Linux) or `__psynch_mutexwait`
(macOS) is contention, not work. The CPU isn't doing anything — it's
parked waiting for a lock. **Adding cores won't help.** Shrink the
critical section, switch to a sharded map, or move state into an
actor.

If you're using `tokio::sync::Mutex`, contention also shows up as
async wakeups stacked under the runtime poll loop. Look for
`Notify::notify_one` next to the mutex frames.

### 3. Async task starvation

```text
tokio::runtime::worker::run_task ────────────────────┐
└─ <Some Long Future as Future>::poll ────────────────┤
   └─ {your code} ─────────────────────────────────── ┤
      └─ blake3::hash / argon2::hash / parse_json ────┘
```

A single task holding the worker for a long time (no `.await`
inside). The graph looks fine, but throughput is bad and p99 is
spiky. The signal: a wide leaf that does *synchronous* work
(`blake3`, `argon2`, big `serde_json::from_str`) directly under the
runtime poll. Move to `spawn_blocking` or `rayon`. See
`docs/perf/budgets.md` — the `argon2 dominates` note on the auth
routes is exactly this shape, and is the *correct* answer once
moved off the runtime thread.

### 4. Allocator pressure

```text
{anything} ──────────────────────────────────────────┐
└─ alloc::alloc::alloc ───────────────────────────────┤
   └─ jemalloc / mimalloc / __rust_alloc_zeroed ──────┘
```

A surprisingly wide block in the allocator across many call sites.
This shows up as a thin "spike" under almost every leaf that touches
`Vec::push` or `String::clone`. Cures: pre-size collections,
`SmallVec` for hot paths, switch global allocator (`jemalloc`,
`mimalloc`), or — best — allocate less.

Allocator pressure is sneaky because it's *spread across the graph*,
not concentrated. If your graph is flat (see below) and you have a
lot of `clone()` in the code, suspect allocator pressure even if no
single `alloc` block is wide.

### 5. Syscall-bound (network + disk I/O)

```text
tokio::runtime::worker ──────────────────────────────┐
└─ mio::poll ─────────────────────────────────────────┤
   └─ epoll_wait ─────────────────────────────────────┘
```

A wide block ending in `epoll_wait`, `recvfrom`, or `read` means the
CPU isn't busy — it's waiting for I/O. CPU-profiler flamegraphs can
*mislead* you here: many tools count off-CPU time too, but some
don't. If your CPU graph is mostly `epoll_wait`, switch to an
off-CPU profiler (`offcputime-bpfcc`) to see what you were *actually*
waiting for. Common cure: connection pooling, batching, or moving the
slow upstream behind a cache (`projects/11-redis-cache`).

## Capturing one for the notes-api

```bash
# Build with debug symbols even in release.
RUSTFLAGS="-C force-frame-pointers=yes" \
    cargo flamegraph --bin notes-api --release -- \
        --port 3001

# In a second terminal — drive load with the harness.
cargo run -p load-test --release -- \
    --url http://127.0.0.1:3001/v1/notes \
    --concurrency 32 \
    --requests 20000
```

When the load run ends, kill the API (`Ctrl-C`). `cargo flamegraph`
writes `flamegraph.svg` in the crate root. Open it in a browser —
SVGs are interactive (click to zoom, search box top-right).

Want a longer capture? Pass `--no-default-features` to `cargo
flamegraph` for a smaller binary, then run it under `perf` directly:

```bash
sudo perf record -F 997 -g --call-graph dwarf -- ./target/release/notes-api &
# ... drive load ...
sudo perf script | inferno-collapse-perf | inferno-flamegraph > out.svg
```

### Reading the captured graph

Steps in order:

1. **Search for `handle_request`** (or your route fn). That's the
   entry. Click to zoom.
2. **Note the total-time number.** If it's < 5% of the whole graph,
   the bottleneck is elsewhere — async runtime, tokio worker
   itself, the connection acceptor. Zoom out.
3. **Walk down the widest child.** Repeat until you find a leaf
   that's either user code or a recognizable hotspot
   (serde, sqlx, hashbrown).
4. **Cross-reference** the leaf against the five patterns above.
5. **Write down the hypothesis** in your perf report before you
   change anything (see `methodology.md`).

## What to do when the flamegraph is flat

"Flat" = no single block dominates; the graph looks like a wide
shallow grass field instead of tall flames.

A flat flamegraph **is itself a signal**, not a failure. Possible
meanings, in order of how often we see them:

1. **The bottleneck is off-CPU.** The CPU isn't busy. You need an
   off-CPU profiler (`offcputime-bpfcc` on Linux, Instruments on
   macOS) to see lock waits / I/O / sleep.
2. **Allocator pressure spread thin.** Many small `alloc::alloc::alloc`
   blocks sum to a lot, but no single one stands out. Try swapping
   the global allocator and re-measuring; if p99 drops, this was it.
3. **Genuine flat workload.** Some services *are* just doing a lot
   of small, varied work (a router with 200 endpoints, each rarely
   hit). The honest answer is "we are CPU-bound at our throughput
   ceiling; throw cores at it." Use `capacity-planner` (this
   workspace) to size the cores, file the cost, move on.
4. **Sampling rate too low.** If you only have ~200 samples total,
   nothing will stand out. Re-capture with `-F 997` and a longer
   window.
5. **Symbols are missing.** Lots of `0x7f8...` raw addresses or
   `[unknown]` frames? You stripped symbols. Rebuild with
   `[profile.release] strip = "none"` and `force-frame-pointers=yes`.

A flat graph after ruling out (4) and (5) means the easy wins are
gone. That's a valid result. The team should pivot to:

- Profile a *specific slow request* in isolation
  (e.g. `tokio-console` for async hangs).
- Look at the database side (`EXPLAIN ANALYZE`, slow-query log).
- Re-examine algorithmic complexity rather than constant factors.

## Tools

| Tool | When to use |
|---|---|
| `cargo flamegraph` | Default. One command, SVG out. |
| `perf` + `inferno` | Long captures, more control, Linux only. |
| `samply` | Cross-platform, browser viewer. Nice if `perf` is awkward. |
| `tokio-console` | Async hangs, **not** CPU hot paths. |
| `offcputime-bpfcc` | When CPU graph is flat — shows where you're blocked. |
| `heaptrack` | Allocation pressure (memory + count, not CPU). |

## Related

- `docs/perf/methodology.md` — load-test protocol.
- `docs/perf/budgets.md` — per-route p99 budgets the flamegraph helps
  you stay under.
- `docs/perf/notes-api-2026-05-26.md` — worked example where the
  fix was found by reading the flamegraph (serde dominating).
- `projects/13-load-test/` — the load harness referenced above.
- `projects/15-capacity-planner/` — once you've optimized the request
  path, size the fleet.
- Phase 11 curriculum lessons 6–8.
