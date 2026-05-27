# How `async`/`await` Actually Works (Rust + Tokio)

The curriculum uses Tokio everywhere — every HTTP handler, every database
call, every background worker is `async`. Most of the time the type system
and `#[tokio::main]` are enough that you don't have to think about what's
happening underneath. This file is the "I have to debug a hung future, or
a `future is not Send` error, or a tail-latency cliff, and treating async
as magic isn't getting me there" reference.

## The mental model in one paragraph

An `async fn` in Rust compiles to a state machine that implements the
`Future` trait. `.await` is a yield point: "poll the inner future; if it's
ready, take its value and continue; if it's not, save my state, register
a waker, and return `Poll::Pending` to whoever is polling *me*." The
executor (Tokio) is a loop that polls futures it owns, parks them when
they return `Pending`, and re-polls them when their wakers fire. There is
no implicit threading involved in any of this — async is cooperative
scheduling over the top of a small fixed thread pool.

## What `async fn` desugars to

This:

```rust
async fn fetch_user(id: u64) -> Result<User, sqlx::Error> {
    let row = sqlx::query!("SELECT * FROM users WHERE id = $1", id)
        .fetch_one(&pool)
        .await?;
    Ok(User::from(row))
}
```

Conceptually becomes something like:

```rust
fn fetch_user(id: u64) -> impl Future<Output = Result<User, sqlx::Error>> {
    enum State {
        Start { id: u64 },
        AwaitingQuery { fut: QueryFuture },
        Done,
    }

    struct FetchUserFuture { state: State, /* + captured locals */ }

    impl Future for FetchUserFuture {
        type Output = Result<User, sqlx::Error>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            loop {
                match &mut self.state {
                    State::Start { id } => {
                        let fut = sqlx::query!(...).fetch_one(&pool);
                        self.state = State::AwaitingQuery { fut };
                    }
                    State::AwaitingQuery { fut } => {
                        let row = match Pin::new(fut).poll(cx) {
                            Poll::Ready(Ok(row)) => row,
                            Poll::Ready(Err(e)) => {
                                self.state = State::Done;
                                return Poll::Ready(Err(e));
                            }
                            Poll::Pending => return Poll::Pending,
                        };
                        self.state = State::Done;
                        return Poll::Ready(Ok(User::from(row)));
                    }
                    State::Done => panic!("polled after completion"),
                }
            }
        }
    }

    FetchUserFuture { state: State::Start { id } }
}
```

The actual generated code is uglier and lives in MIR, but the shape is
exactly this. Each `.await` becomes a state with a stored sub-future; the
`poll` method is a big match on which `.await` we're suspended at.

The crucial properties that fall out of this:

- **Calling `fetch_user(42)` does nothing.** It just constructs a state
  machine in state `Start`. No query runs until something polls the
  resulting future. This is why "async functions are lazy" — they are
  pure constructors of state machines.
- **The state machine captures every local variable used across `.await`
  points.** This is where `Send` bounds bite you (see below).
- **Suspending stores the state and returns.** It doesn't park a thread.

## `.await` in plain English

`x.await` means: "poll `x`. If it's `Ready(v)`, evaluate to `v` and
continue. If it's `Pending`, save my state into the enclosing future and
propagate `Pending` upward. When something wakes me, I'll be polled
again and will re-enter `x`'s poll."

The "save my state" step is automatic — the compiler generates the state
machine such that all locals you need are in the struct.

## The executor

`#[tokio::main] async fn main()` expands roughly to:

```rust
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async { /* your main body */ });
}
```

The runtime is the executor. It owns:

- A small pool of **worker threads** (default: one per logical CPU).
- Per-worker **local queues** of ready-to-poll tasks.
- A **global injection queue** for tasks spawned from outside any worker
  (e.g. from `main` before the runtime is "in").
- An I/O driver (epoll on Linux, kqueue on macOS, IOCP on Windows)
  running on a separate thread.
- A timer wheel for `tokio::time::sleep` and friends.

The main loop, simplified:

```
loop {
    let task = pop_local_queue()
        .or_else(|| try_steal_from_other_worker())
        .or_else(|| pop_global_queue())
        .or_else(|| park_until_io_or_timer());

    let waker = task.make_waker();
    let mut cx = Context::from_waker(&waker);
    match Pin::new(&mut task.future).poll(&mut cx) {
        Poll::Ready(out) => task_completes(out),
        Poll::Pending => /* the future has stored the waker somewhere; do nothing */,
    }
}
```

The waker is a small handle that, when called, pushes the task back onto
some run queue. That's how a future "asks to be polled again": whatever
resource it was waiting on (a socket, a timer, a mutex) holds the waker
and calls `waker.wake()` when the resource is ready.

## What `tokio::spawn` does

```rust
let handle = tokio::spawn(async {
    do_some_work().await;
});
```

`spawn` does:

1. Wraps the future in a `Task` with metadata (id, status, output slot).
2. Pushes the task onto the current worker's local queue (or onto the
   global queue if called from outside any worker).
3. Returns a `JoinHandle` that the caller can `.await` to get the spawned
   future's result, or drop to detach.

Note that **the spawned future starts running concurrently with the
caller**, on whichever worker picks it up first (possibly the same one as
the caller, after the caller next `.await`s and yields). It is NOT a
new thread. It is a new task sharing the same handful of threads with
everything else.

`tokio::join!` does NOT spawn. It polls multiple futures on the *current*
task in lockstep:

```rust
let (a, b) = tokio::join!(fetch_a(), fetch_b());
```

This is concurrency without parallelism — both futures progress, but only
when the current task is being polled. If `fetch_a` is CPU-bound, `fetch_b`
makes no progress. `tokio::spawn` is the way to get actual parallel
progress across futures.

[`projects/02-quote-generator`](../../projects/02-quote-generator/) uses
this pattern: it fetches from multiple quote sources concurrently via
`tokio::spawn` and `JoinSet`, so that a slow source doesn't block a fast
one.

## Blocking in async code — the cardinal sin

```rust
async fn read_config() -> String {
    std::fs::read_to_string("config.toml").unwrap()  // BLOCKING
}
```

This compiles. The type is correct. It runs. And it will silently destroy
your tail latencies under load.

Here's why: the worker thread polling `read_config` calls `std::fs::read_to_string`,
which is a synchronous syscall that doesn't return until the file is read.
While that thread is blocked in the syscall, **every other task that was
in that worker's local queue is also blocked.** If you have 4 workers and
4 simultaneous file reads, your entire async runtime is stalled.

The same applies to:

- `std::fs::*` (synchronous filesystem)
- `std::sync::Mutex` held across an `.await` (not just blocking — see below)
- `std::thread::sleep`
- Any third-party crate that internally calls blocking syscalls (this is
  the worst category — it's not obvious from the function signature)
- CPU-heavy work: hashing a 10MB blob, parsing a huge JSON, image resizing
- `reqwest::blocking` (the name should tell you, but people use it anyway)
- Synchronous database drivers (`postgres` instead of `tokio-postgres`/`sqlx`)

### The fix: `tokio::task::spawn_blocking`

```rust
async fn read_config() -> String {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string("config.toml").unwrap()
    })
    .await
    .unwrap()
}
```

`spawn_blocking` runs the closure on a separate **blocking thread pool**
(default size: 512 threads). The async worker yields immediately; when the
closure finishes, its return value flows back through the returned future.
Net effect: blocking calls don't poison the async runtime.

Rules of thumb:

- Anything that takes more than ~100 microseconds of CPU should be
  `spawn_blocking`.
- Anything that does a synchronous syscall should be `spawn_blocking`,
  unless the syscall is fast and rare (reading a file once at startup
  is fine).
- The async drivers (`tokio-postgres`, `sqlx`, `reqwest`, `tokio::fs`,
  etc.) are the right answer for I/O — `spawn_blocking` is the escape
  hatch for code you can't or don't want to rewrite.

### `tokio::sync::Mutex` vs `std::sync::Mutex`

`std::sync::Mutex` is fine in async code if you only hold it briefly and
NEVER across an `.await`. If you hold a `std::sync::Mutex` and then
`.await` something, the worker thread is parked with the mutex held —
deadlock and starvation hazards multiply.

`tokio::sync::Mutex` is async: `.lock().await` returns `Pending` when
contended, releasing the worker to do other work. Use it when you need to
hold a lock across an `.await`. Otherwise prefer `std::sync::Mutex` —
it's faster on the uncontended path.

## `Pin` and `!Unpin` — just enough to debug

The full `Pin` story is a research-paper-sized topic. The application-
engineer-sized version:

- The state machine generated for `async fn` contains, among other things,
  references to its own fields. (When `.await`'d sub-future has a borrow
  of a local in the outer state, that's a self-reference.)
- If you moved that state machine in memory, the self-references would
  become dangling.
- `Pin<&mut T>` is a wrapper that promises "this value will never be
  moved." `Future::poll` takes `Pin<&mut Self>` so that the implementor
  can rely on self-references being stable.
- Most types are `Unpin` — they don't care about being moved. Async-fn
  state machines are `!Unpin` (not Unpin) by default. To poll a `!Unpin`
  future you have to pin it first — `tokio::pin!(fut)` or
  `Box::pin(fut)` are the usual incantations.

You will encounter `Pin` mostly in these forms:

- `Box::pin(fut)` — heap-allocate the future and pin it. Used everywhere
  in higher-order async code.
- `tokio::pin!(fut)` — pin on the stack. Cheaper than `Box::pin`, used
  inside `select!` and similar macros.
- Compiler error: "cannot be unpinned" — you tried to use a `!Unpin`
  future with an API expecting `Unpin`. Wrap with `Box::pin`.

You will essentially never need to write `unsafe impl !Unpin` or
implement `Future` by hand. The async machinery handles it.

## Cooperative cancellation

In Rust async, **dropping a future stops it.** The state machine is just
a struct; dropping it runs `Drop` (potentially closing sockets, releasing
locks, etc.) and that's that. There is no "kill signal," no thread to
interrupt, no special API.

This means:

```rust
tokio::select! {
    result = long_running_call() => { ... }
    _ = tokio::time::sleep(Duration::from_secs(5)) => {
        return Err("timeout");
    }
}
```

When the 5-second timer wins, the `select!` macro drops the other
future. The long-running call's state machine is destructed; its current
`.await` point loses its waker; the resource it was waiting on (e.g. a
database query) gets its `Drop` invoked (the query is canceled or the
connection returned to the pool).

The caveats:

- **Cancellation only happens at `.await` points.** A future doing pure
  CPU work cannot be canceled until it next yields. If you have a tight
  CPU loop in async code, neither cancellation nor other tasks can
  intervene.
- **Cancellation may not be clean.** Dropping mid-transaction may leave
  the transaction open until the connection is reclaimed. Dropping a
  future that was holding a file lock may leak the lock until the OS
  notices the FD is closed.
- **Cancellation can be unsafe to compose.** A function written assuming
  "if I run to completion I leave consistent state" might leave
  inconsistent state if dropped at the wrong `.await`. This is the
  "cancellation safety" issue you'll see referenced in `tokio::select!`'s
  docs. Most Tokio primitives are documented as cancellation-safe or
  not; check before using inside `select!`.

[`projects/14-sagas`](../../projects/14-sagas/) deals with this head-on:
when a saga's overall timeout fires, the in-flight step is canceled, and
the saga's compensation logic has to handle "did that step partially
complete?" The answer is "we don't know — write idempotent compensations."

## `Send` bounds across awaits

You will write code that looks fine and the compiler will tell you:

```
error: future cannot be sent between threads safely
  --> src/foo.rs:42:5
   |
42 |     tokio::spawn(async move {
   |     ^^^^^^^^^^^^ future created by async block is not `Send`
   |
   = help: within `[async block@src/foo.rs:42]`, the trait `Send` is not implemented for `Rc<Bar>`
note: future is not `Send` as this value is used across an await
```

What's happening: `tokio::spawn` requires the spawned future to be `Send`
because the multi-threaded scheduler may move the task between worker
threads. For the future to be `Send`, every value held across any `.await`
inside it must be `Send`.

The state-machine model makes this concrete: each `.await` is a struct
field. If any field is `!Send`, the whole struct is `!Send`. Common
offenders:

- `Rc<T>` (use `Arc<T>`).
- `RefCell<T>` (use `tokio::sync::Mutex` or design around it).
- `MutexGuard<'_, T>` from `std::sync::Mutex` (the guard is `!Send` on
  most platforms; either drop the guard before the `.await` or use
  `tokio::sync::Mutex`).
- Pointers into thread-local storage.

Reading the error: the compiler tells you which line *causes* the
`!Send` constraint and which line *holds* the `!Send` value across the
`.await`. Restructure to drop the offending value before the `.await`:

```rust
// BAD: rc held across await
let rc = Rc::new(data);
let value = compute_with(&rc);
do_async_thing().await;
println!("{}", rc.len());  // rc is alive across the await
```

```rust
// GOOD: rc dropped before await
let value = {
    let rc = Rc::new(data);
    compute_with(&rc)
};
do_async_thing().await;
```

Or just use `Arc` everywhere. The runtime overhead is real but tiny.

## Cross-references in this curriculum

- [`projects/02-quote-generator`](../../projects/02-quote-generator/) —
  concurrent fan-out fetches. The README walks through `JoinSet` vs
  `join!` vs `select!`.
- [`projects/08-outbox-demo`](../../projects/08-outbox-demo/) — the chaos
  test deliberately cancels worker tasks mid-loop to exercise the
  "dropped future" recovery path. Also the canonical example of
  `FOR UPDATE SKIP LOCKED` for queue-shaped workloads (see
  [`../database-internals/01-mvcc-and-isolation-levels.md`](../database-internals/01-mvcc-and-isolation-levels.md)).
- [`projects/14-sagas`](../../projects/14-sagas/) — timeouts during
  compensation. What happens when the cancellation deadline fires while
  a compensating step is mid-flight.

## Things to read once you've internalized the above

- "Asynchronous Programming in Rust" book (the official Rust async book).
- Alice Ryhl's blog posts, especially "Async: What is blocking?" and
  "Actors with Tokio."
- The `tokio::runtime` module docs — they're the reference for everything
  in this file.
- The `Pin` module docs — when you decide you really do want to know.

## A debugging checklist

When async behavior surprises you, ask:

1. **Did I forget to `.await`?** A bare `fut` without `.await` is a
   constructor, not a call. The compiler usually warns; sometimes it
   doesn't (e.g. when the future is `Result<impl Future, _>`).
2. **Is something blocking the worker thread?** Stack-trace the workers
   (`tokio-console` is the right tool); if they're in a syscall, find
   the offending blocking call.
3. **Is a task starving?** A CPU-heavy task without `.await` yields
   never. Either break it up or `spawn_blocking` it.
4. **Was the future dropped mid-flight?** Look at the cancellation path:
   `select!`, `timeout`, the parent task being dropped.
5. **`!Send` error?** Find the value held across the `.await`. Either
   move it out before the `.await` or use a `Send` equivalent.
6. **Deadlock?** Are you holding a `std::sync::Mutex` across an
   `.await`? Swap for `tokio::sync::Mutex` or restructure.
