# Lesson 2.2 — Futures and `.await`

> **Concept first:** an `async fn` returns a `Future`. A `Future` is *lazy* — nothing runs until you `.await` it. Don't fear that sentence; spend a minute internalizing it.
> **Time:** 30 minutes.

## What is a `Future`?

A `Future` is the trait every "paused recipe" implements:

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub enum Poll<T> { Ready(T), Pending }
```

The runtime calls `poll()`. If the future is ready, it returns `Poll::Ready(value)`. If it's still waiting on I/O or a timer, it returns `Poll::Pending` and registers a *waker* — a callback the runtime will fire when work is ready to resume.

You will almost never implement `Future` by hand. The `async`/`await` syntax does it for you. But knowing it's just a trait demystifies the magic.

## `async fn` returns a `Future`

```rust
async fn double(n: i32) -> i32 {
    n * 2
}
```

is syntactic sugar for:

```rust
fn double(n: i32) -> impl Future<Output = i32> {
    async { n * 2 }
}
```

Both forms produce a *future*. The body doesn't run when you call `double(5)`; it runs when you `.await` it.

## Futures are lazy

This is the part that trips up Python and JavaScript developers, where promises start running the moment they're created.

```rust
async fn say_hi() { println!("hi"); }

#[tokio::main]
async fn main() {
    let f = say_hi();       // creates the future; "hi" does NOT print
    println!("before");
    f.await;                // NOW "hi" prints
    println!("after");
}
```

Output:

```
before
hi
after
```

If you remove the `.await`, you get a compiler warning ("unused implementer of Future"); and `"hi"` never prints. **Futures must be awaited (or spawned) to make progress.**

## `.await` is "wait here and let the runtime do other work"

When the runtime polls your task and your code hits an `.await` on a not-yet-ready future, control returns to the runtime. The runtime moves on to other tasks. When the awaited future becomes ready, your task is woken and continues from that `.await`.

You write *synchronous-looking code* that the compiler rewrites into a state machine.

## Combinators

Sometimes you want to wait on multiple futures together. Tokio gives you tools:

### Sequential — just `.await` them in order

```rust
let a = fetch_thing(1).await;
let b = fetch_thing(2).await;
let c = fetch_thing(3).await;
```

`a` finishes before `b` starts. Total time = sum of all three.

### Concurrent — `join!`

```rust
use tokio::join;
let (a, b, c) = join!(fetch_thing(1), fetch_thing(2), fetch_thing(3));
```

All three start at once. Total time = max of all three. **Use `join!` when you don't need one result before starting the next.**

### Concurrent with results — `try_join!`

For futures that return `Result<T, E>`:

```rust
let (a, b, c) = tokio::try_join!(fetch(1), fetch(2), fetch(3))?;
```

Short-circuits on the *first* error. Cleaner than `join!` + manual error handling for happy-path-heavy code.

### Concurrent over a collection — `JoinSet` or `FuturesUnordered`

```rust
use tokio::task::JoinSet;

let mut set = JoinSet::new();
for url in urls {
    set.spawn(async move { reqwest::get(&url).await });
}
while let Some(res) = set.join_next().await {
    println!("{res:?}");
}
```

This is exactly what we'll use in `projects/02-quote-generator`. Spawn N tasks, process results as they finish — out-of-order, which is the whole point of concurrency.

## A subtle pitfall — futures don't run until polled

```rust
// ⚠️ Looks parallel, ISN'T
for url in urls {
    fetch(url).await;     // each .await blocks until done before starting the next
}
```

Compare:

```rust
// ✓ Actually parallel
let mut set = JoinSet::new();
for url in urls {
    set.spawn(fetch(url));
}
while let Some(_) = set.join_next().await {}
```

The first one is *sequential*, even though it's full of `.await`. The second runs concurrently because each `set.spawn` hands the future to the runtime *immediately*.

## Why this matters

- **Knowing futures are lazy lets you reason about scheduling.** "Why is this slow?" is often "because you wrote `await` in a loop instead of `JoinSet`."
- **`.await` is a yield point.** Long stretches of synchronous code between `.await`s can starve the runtime — a topic we revisit in Phase 11.
- **`async fn` is a contract.** A function marked `async` returns `impl Future<Output = …>`; callers must await it. The compiler enforces it.

## Green-bar checkpoint

- You can articulate why a `Future` doesn't do anything until `.await`ed.
- You can rewrite a sequential `for url in urls { fetch(url).await; }` into a concurrent `JoinSet`.
- You can read `tokio::join!(a, b, c)` and predict total runtime.

Next: `lessons/03-tokio-spawn-and-join.md`.
