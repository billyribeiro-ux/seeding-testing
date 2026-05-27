# Phase 2 — Exercises

Seven graded drills. Some standalone; some extending `projects/02-quote-generator`.

---

## E2.1 — Sequential vs concurrent (Easy) — shipped

`tokio::join!` drill. Both the sequential prediction (~3 s) and the concurrent rewrite (~1 s) are written out in the `<details>` answer below; the deliverable is the learner running each and timing them.

Predict the runtime, then run it and check. Each `sleep` is 1 second.

```rust
#[tokio::main]
async fn main() {
    let start = std::time::Instant::now();
    for _ in 0..3 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    println!("{:?}", start.elapsed());
}
```

Now rewrite to take ~1 second by using `tokio::join!`.

<details><summary>Answer</summary>

Original is sequential — ~3 s. Concurrent version:

```rust
let (_, _, _) = tokio::join!(
    tokio::time::sleep(std::time::Duration::from_secs(1)),
    tokio::time::sleep(std::time::Duration::from_secs(1)),
    tokio::time::sleep(std::time::Duration::from_secs(1)),
);
```

About 1 s. The three sleeps share the runtime.
</details>

---

## E2.2 — `try_join!` (Easy) — shipped

`tokio::try_join!` drill. The one-line solution that fetches both concurrently and short-circuits on either error is in the `<details>` answer below.

Given two async functions `fetch_a()` and `fetch_b()` that return `Result<String, anyhow::Error>`, write a function `both()` that fetches them concurrently and returns the pair. Short-circuit on either error.

<details><summary>Answer</summary>

```rust
async fn both() -> anyhow::Result<(String, String)> {
    Ok(tokio::try_join!(fetch_a(), fetch_b())?)
}
```
</details>

---

## E2.3 — Spawn vs await (Easy) — shipped

Spawn-vs-await drill. The diagnosis (serialized `.await`s) and both fixes (gather handles first, or `tokio::join!` them) are in the `<details>` answer below.

Why does the following take ~3 seconds instead of ~1?

```rust
let a = tokio::spawn(slow(1)).await.unwrap();
let b = tokio::spawn(slow(1)).await.unwrap();
let c = tokio::spawn(slow(1)).await.unwrap();
```

Fix it.

<details><summary>Answer</summary>

Each `.await` blocks the current task until that handle completes — so the three spawns are serialized. Fix by gathering handles first, then awaiting:

```rust
let h1 = tokio::spawn(slow(1));
let h2 = tokio::spawn(slow(1));
let h3 = tokio::spawn(slow(1));
let (a, b, c) = (h1.await?, h2.await?, h3.await?);
```

Or use `tokio::join!(h1, h2, h3)`.
</details>

---

## E2.4 — Add an overall deadline test (Medium) — shipped

Targets the shipped `projects/02-quote-generator/` binary: `src/main.rs` already returns exit code `3` and prints "overall deadline ... exceeded" when `--deadline` trips (see `cfg.overall_deadline` plumbing in `src/lib.rs`). The `<details>` answer below provides the wiremock-backed `tests/cli.rs` integration test the learner appends.

Add an integration test to `projects/02-quote-generator/tests/cli.rs` that:

1. Mocks a wiremock endpoint with `.set_delay(Duration::from_millis(800))`.
2. Runs the binary with `--deadline 200ms`.
3. Asserts the exit code is `3` and stderr contains "deadline ... exceeded".

<details><summary>Answer</summary>

```rust
#[tokio::test(flavor = "current_thread")]
async fn overall_deadline_exits_with_code_3() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(200)
            .set_delay(std::time::Duration::from_millis(800))
            .set_body_string("slow"))
        .mount(&server).await;

    let url = format!("{}/x", server.uri());
    let out = tokio::task::spawn_blocking(move || {
        bin().arg(&url).arg("--deadline").arg("200ms").assert().code(3)
            .stderr(predicate::str::contains("deadline"))
            .get_output().clone()
    }).await.unwrap();
    let _ = out;
}
```

`make verify` must still pass.
</details>

---

## E2.5 — Channels: producer/consumer (Medium) — shipped

`mpsc` drill. The complete producer/consumer program (bounded `mpsc::channel`, two spawned tasks, `tokio::join!` to wait) is in the `<details>` answer below — the deliverable is the learner pasting it into a scratch binary and running it.

Write a small program where a producer task pushes 10 integers into an `mpsc::channel`, a consumer task prints them, and `main` waits for both to finish.

<details><summary>Answer</summary>

```rust
#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<i32>(16);

    let producer = tokio::spawn(async move {
        for i in 0..10 { tx.send(i).await.unwrap(); }
    });
    let consumer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await { println!("got {v}"); }
    });
    let _ = tokio::join!(producer, consumer);
}
```
</details>

---

## E2.6 — Cancellation safety audit (Medium) — shipped

Reading exercise against the shipped `projects/02-quote-generator/src/lib.rs::fetch_one`. The two `.await` points (the `timeout(...)` around `reqwest::send`, and the `timeout(...)` around `resp.bytes()`) are already present in the code; the `<details>` answer below walks through each and links the takeaway to idempotency in Phase 8.

In `projects/02-quote-generator/src/lib.rs::fetch_one`, identify every `.await` point. For each, write one sentence explaining what happens to the in-flight work if the task is cancelled there.

<details><summary>Answer</summary>

The two `.await`s are:
1. `timeout(cfg.per_request_timeout, request).await` — if cancelled, the underlying `reqwest::send` future is dropped, closing the connection setup. Server may have received the request line but no response was consumed by us. Safe.
2. `timeout(cfg.per_request_timeout, resp.bytes()).await` — if cancelled, the body read aborts, the connection is dropped. Server-side state may have already been written; *that's why our overall design requires idempotency keys on real Stripe calls (Phase 8)*.

Lesson: cancellation-safe at the *client* level. Server-side idempotency is a separate concern.
</details>

---

## E2.7 — Bounded concurrency (Stretch) — shipped

Stretch extension of `projects/02-quote-generator/` — the base semaphore + `fetch_one` are already in `src/lib.rs`, and the `<details>` sketch below shows where to wrap the existing logic in a retry loop (holding the permit across attempts) plus how to drive the test deterministically with `wiremock`'s `up_to_n_times` and `tokio::time::pause()`. No retry code is shipped in the crate; the deliverable is the learner adding it on top.

Add a `--retries N` flag to `quote-generator`: each failed fetch is retried up to `N` times with exponential backoff (100 ms, 200 ms, 400 ms). The semaphore must still cap *total* concurrent in-flight requests (retries count).

Write a unit test that confirms a request that flakes twice then succeeds is reported as `OK`.

<details><summary>Answer (sketch)</summary>

In `fetch_one`, wrap the existing logic in a loop with `for attempt in 0..=cfg.retries { ... sleep(...).await; }`. Make sure the semaphore permit is held *for the entire retry sequence* so retries don't bypass the concurrency cap.

For the test, use `wiremock`'s `up_to_n_times(2).respond_with(503)` then `.respond_with(200)`. Stub `tokio::time::sleep` via `tokio::time::pause()` in the test to avoid real wait time.
</details>
