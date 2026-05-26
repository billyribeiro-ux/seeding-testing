# Mental Model: Async is Cooperative Multitasking

> *A `Future` is a paused recipe. `.await` is "I'm waiting; chef, take
> someone else." The runtime is the chef.*

## The kitchen metaphor

Imagine a chef in a busy kitchen:

- Many orders going at once (10 pans on the stove, 3 ovens, 2 grills).
- Each order takes seconds to minutes.
- The chef is one person; they can do exactly one thing at any moment.
- Most of the cooking is *waiting* — for water to boil, for the oven to
  preheat, for the steak to rest.

A skilled chef never stands idle. When a pot says "I need 5 more
minutes," the chef walks to another station. When the oven beeps, they
come back.

Async runtimes are that chef. *Tasks* are recipes. `.await` is the
moment the recipe says "I'm waiting on the oven."

## Three rules to internalize

### 1. `.await` is a yield point

Code between two `.await`s runs straight through — no other task can
interrupt it. Code at an `.await` may yield to the runtime.

```rust
async fn handler() {
    let a = step_1().await;        // task may yield here
    process(&a);                   // runs uninterrupted
    let b = step_2(&a).await;      // task may yield here
    save(b).await;                 // task may yield here
}
```

This means:

- Locks released between `.await`s are fine.
- Locks held across `.await`s are dangerous (Lesson 2.5).
- Long-running CPU work *between* `.await`s blocks the runtime thread.

### 2. The blocking sin

A *blocking* call holds the thread without yielding. In an async runtime
that has 4 worker threads handling 10,000 tasks, one blocking call can
freeze 2,500 of them.

Sins to never commit:

- `std::thread::sleep(...)` — use `tokio::time::sleep(...).await`.
- `std::fs::read(...)` — use `tokio::fs::read(...).await` or
  `spawn_blocking`.
- `expensive_cpu_computation()` — wrap in `spawn_blocking`.
- A 100MB JSON parse — same.

### 3. Cancellation is real

A task can be dropped mid-`.await`. Common reasons:

- `select!` won; the loser is dropped.
- A `JoinHandle::abort()` was called.
- A `tokio::time::timeout` fired.

When a task is dropped, execution *stops* at the current `.await`. No
`finally` block runs; values are `Drop`ped. Design for it:

- Don't hold a lock across an `.await` if the lock guarantees an
  invariant that has to be re-established.
- For outbound network writes that aren't idempotent, the server might
  have already received the request. Use idempotency keys.

## What "thousands of tasks" actually means

A task isn't a thread. Each task is a small heap-allocated state
machine (~few hundred bytes). The runtime polls them; only the ones
that have work to do consume CPU.

10,000 idle tasks waiting on sockets cost roughly:

- 10,000 × ~300 B of memory ≈ 3 MB.
- ~0% CPU (they're not being polled until something wakes them).

10,000 *threads* in the OS sense would cost:

- 10,000 × ~1 MB stack each ≈ 10 GB.
- Heavy context-switching overhead.

Async wins by 1000× on connection density.

## When async is not the answer

- **Single-process CPU-bound work.** A ray tracer. A matrix multiply.
  A LLM forward pass. Async adds overhead with no benefit; just use
  threads (`rayon`, `std::thread::spawn`).
- **Strict per-request work that can't be interrupted.** A real-time
  audio handler. (Though Tokio's `current_thread` runtime can serve
  even this with care.)
- **Code that never does I/O.** Pure logic libraries don't need to be
  async. Don't infect them.

## The principal-engineer takeaway

- **Every `.await` is a yield.** Reason about your code as "blocks of
  uninterrupted work separated by yields."
- **Async + I/O = scale; async + CPU = wasted complexity.**
- **Cancellation safety is a design property.** Test for it on critical
  paths.

## Related

- Phase 2 lessons (6 of them, especially 1, 2, 5)
- `projects/02-quote-generator`
- PLAYBOOK — "Async is cooperative multitasking"
