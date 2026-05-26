# Phase 2 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Concurrency model** | Confuses concurrency with parallelism | Can articulate the difference; knows when async is right | Designs services where bounded concurrency, timeouts, and back-pressure are baked into the architecture |
| **`async`/`await`** | Awaits in a loop, expects parallelism | Knows futures are lazy; uses `join!`/`JoinSet`/`FuturesUnordered` correctly | Reasons about scheduling, identifies blocking calls in async code, profiles task overhead |
| **Spawning** | `tokio::spawn` everywhere | Knows when to spawn vs await; understands `JoinHandle` and `JoinError` | Designs graceful-shutdown with `CancellationToken` and `select!` |
| **Channels** | Uses `mpsc` for everything | Picks the right flavour (`mpsc`/`oneshot`/`broadcast`/`watch`) for the case | Designs backpressure and shutdown protocols around bounded channels |
| **Cancellation safety** | Unaware of it | Knows mutex-across-await is risky; uses idempotency on outbound writes | Audits every `.await` point in critical paths; can explain what happens on cancellation |
| **Timeouts** | None | Per-call `timeout` on every outbound I/O | Layered budgets: per-call + overall; documents SLA contracts |
| **Shared state** | `Arc<Mutex<T>>` everywhere | Picks atomics / RwLock / Semaphore appropriately | Minimizes locking via channels and immutable data |
| **Testing async code** | Live network or no tests | Uses wiremock; understands `flavor = "current_thread"` for mocking | Uses `tokio::time::pause()` for fast, deterministic time-dependent tests |

## Self-check before moving to Phase 3

- [ ] `make verify` passes locally and in CI for both `hello-cli` and `quote-generator`.
- [ ] You completed Exercises E2.1 – E2.6.
- [ ] You can explain why `FuturesUnordered` yields results in completion order while `for url in urls { fetch(url).await; }` doesn't.
- [ ] You can name three blocking calls and their async equivalents.
- [ ] You can sketch a graceful-shutdown loop on a whiteboard.
- [ ] CI is green.

Phase 3 — SQL and Databases (Drizzle/SQLite warm-up, then Postgres + sqlx) — assumes this is internalized.
