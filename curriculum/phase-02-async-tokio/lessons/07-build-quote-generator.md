# Lesson 2.7 — Build `quote-generator` Step by Step

> **The capstone of Phase 2.** You'll walk through `projects/02-quote-generator/` line by line — the working code already in the repo.
> **Time:** 90 minutes.

## What we're building

A `wc`-style concurrent HTTP fetcher:

```
$ quote-generator --concurrency 4 --timeout 2s --deadline 30s \
    https://api.quotable.io/random https://api.adviceslip.com/advice
OK    200    187.4ms  https://api.adviceslip.com/advice
OK    200    241.1ms  https://api.quotable.io/random
quote-generator: completed — total=2 ok=2 fail=0
$ echo $?
0
```

It teaches three patterns we'll use forever in Axum services:

1. **Bounded concurrency with `Semaphore`.**
2. **Per-request `timeout`.**
3. **Streaming results in completion order with `FuturesUnordered`.**

## Step 0 — Browse the project

```bash
cd projects/02-quote-generator
ls -R src tests
```

You should see:

```
src/lib.rs       src/main.rs       tests/cli.rs
```

Read along — don't rebuild.

## Step 1 — The manifest

```toml
[dependencies]
clap     = { workspace = true }
tokio    = { workspace = true }
reqwest  = { workspace = true }
futures  = { workspace = true }
tracing  = { workspace = true }
humantime = { workspace = true }

[dev-dependencies]
wiremock   = { workspace = true }
```

Highlights:
- **`tokio` with features** `["macros", "rt-multi-thread", "time", "signal", "sync", "fs", "io-util"]` — only what we use; trims compile time.
- **`reqwest` with `rustls-tls`** (no OpenSSL dep) — easier to cross-compile.
- **`futures` for `FuturesUnordered` and `StreamExt`** (Tokio's `stream` module is feature-gated; the `futures` crate is the standard).
- **`humantime` to parse `--timeout 5s`** strings.
- **`wiremock` as a dev-dep** so integration tests never hit the public internet.

## Step 2 — The library (`src/lib.rs`)

Three pieces: a result type, a config struct, and one public function.

### 2.1 The result type

```rust
#[derive(Debug, Clone)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub body_preview: String,
    pub elapsed: Duration,
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("timeout fetching {url} after {after:?}")]
    Timeout { url: String, after: Duration },
    #[error("network error fetching {url}: {source}")]
    Network { url: String, #[source] source: reqwest::Error },
}

pub type FetchResult = Result<Fetched, FetchError>;
```

Notice: the `url` is repeated in every error variant. That's deliberate — when one of N URLs fails, the error message has to tell you *which*. We pay one `String` per error for the audit trail.

### 2.2 The config

```rust
#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub concurrency: usize,
    pub per_request_timeout: Duration,
    pub overall_deadline: Option<Duration>,
    pub preview_bytes: usize,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self { concurrency: 4, per_request_timeout: Duration::from_secs(5), overall_deadline: None, preview_bytes: 200 }
    }
}
```

A config struct, not a six-argument function. Future-proof.

### 2.3 `fetch_all` — the concurrent fan-out

```rust
pub fn fetch_all<I>(
    client: reqwest::Client, urls: I, cfg: FetchConfig,
) -> impl futures::Stream<Item = FetchResult>
where I: IntoIterator<Item = String>,
{
    let urls: Vec<String> = urls.into_iter().collect();
    let sem = Arc::new(Semaphore::new(cfg.concurrency.max(1)));
    let cfg = Arc::new(cfg);

    let futs = FuturesUnordered::new();
    for url in urls {
        let sem = sem.clone(); let cfg = cfg.clone(); let client = client.clone();
        futs.push(async move {
            let _permit = sem.acquire().await.expect("semaphore is never closed");
            fetch_one(&client, url, &cfg).await
        });
    }

    futs.boxed()
}
```

Walk through the moves:

- **`Arc::new(Semaphore::new(N))`** — a shared semaphore with `N` permits. Each task holds one while fetching.
- **`Arc::new(cfg)`** — `cfg` is read-only after this point. `Arc` avoids cloning the whole config per task.
- **`FuturesUnordered::new()`** — a collection that yields futures *as they finish*. Out-of-order, which is exactly what we want.
- **`client.clone()`** — `reqwest::Client` is internally an `Arc`. Cloning is `O(1)`.
- **`async move`** — captures `sem`, `cfg`, `client` by move into a new future. Three small clones of `Arc` per task.

The function returns `impl Stream<Item = FetchResult>` — the caller can do `while let Some(r) = stream.next().await { ... }` and print results as they arrive.

### 2.4 `fetch_one` — the inner workhorse

```rust
async fn fetch_one(client: &reqwest::Client, url: String, cfg: &FetchConfig) -> FetchResult {
    let started = Instant::now();
    let request = client.get(&url).send();

    let resp = match timeout(cfg.per_request_timeout, request).await {
        Err(_)        => return Err(FetchError::Timeout { url, after: cfg.per_request_timeout }),
        Ok(Err(e))    => return Err(FetchError::Network { url, source: e }),
        Ok(Ok(r))     => r,
    };
    // ... read body with another timeout, build Fetched ...
}
```

Two timeouts: one around `.send()` (handshake + headers) and one around `.bytes()` (body read). A single `timeout` around the whole future would be coarser; two timeouts give a tighter SLA story.

## Step 3 — The binary (`src/main.rs`)

```rust
#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt() /* … */ .init();
    let cli = Cli::parse();
    let urls = match collect_urls(&cli) { /* … */ };
    let cfg = FetchConfig { /* from cli */ };
    let client = reqwest::Client::builder().user_agent("quote-generator/…").build().unwrap();

    let mut stream = fetch_all(client, urls, cfg.clone());
    let consume = async {
        let (mut ok, mut fail) = (0usize, 0usize);
        while let Some(r) = stream.next().await {
            println!("{}", render(&r));
            match &r {
                Ok(f) if (200..300).contains(&f.status) => ok += 1,
                _ => fail += 1,
            }
        }
        (ok, fail)
    };

    let (ok, fail) = if let Some(dl) = cfg.overall_deadline {
        let Ok(counts) = tokio::time::timeout(dl, consume).await else {
            eprintln!("overall deadline {dl:?} exceeded");
            return ExitCode::from(3);
        };
        counts
    } else { consume.await };

    if fail == 0 { ExitCode::SUCCESS } else { ExitCode::from(1) }
}
```

Five things to notice:

1. **`#[tokio::main]`** — multi-thread runtime, one worker per CPU.
2. **`tracing_subscriber::fmt().init()`** — pretty logs with `RUST_LOG=info` overrideable via env.
3. **Overall deadline wraps the *consumption* of the stream** — we don't kill in-flight requests; we just stop waiting. The OS / TCP layer eventually closes their sockets.
4. **`let Ok(counts) = … else { return … };`** — Rust 1.65+ "let else" pattern. The clean way to early-return on a single arm of a Result.
5. **Exit codes are *documented*** in the file's top comment. Scripts that pipe into us depend on those.

## Step 4 — Integration tests (`tests/cli.rs`)

```rust
#[tokio::test(flavor = "current_thread")]
async fn fetches_a_single_url() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("hello"))
        .mount(&server).await;

    let url = format!("{}/q", server.uri());
    let out = tokio::task::spawn_blocking(move || {
        bin().arg(&url).assert().success().get_output().clone()
    }).await.unwrap();
    // assert on stdout
}
```

Two subtle moves:

- **`flavor = "current_thread"`** in the test attribute — wiremock works on a single-threaded runtime; we use one too so we don't get rare races.
- **`tokio::task::spawn_blocking`** — `assert_cmd::Command::assert()` *blocks the thread*. If we called it directly inside an async test on a current-thread runtime, we'd deadlock the runtime against itself. Moving the blocking call to a blocking pool decouples them.

## Step 5 — Verify, commit, push

```bash
make verify              # fmt, clippy, nextest — all green
git add -A
git commit -m "feat(phase-02): add quote-generator capstone with wiremock tests"
git push
gh run watch             # CI green
```

## Why this matters

- **Every Axum service we build later uses these same primitives.** A request handler that fans out to three downstream services uses `JoinSet`/`try_join!`. Every outbound call gets a `timeout`. Every fan-out is rate-limited by a `Semaphore`.
- **Wiremock-driven integration tests are how we test *behaviour* without the public internet.** You'll see the same pattern in Phase 8 for Stripe webhook tests.
- **Exit codes documented in the binary's doc-comment** is a senior habit. Make scripts that consume your CLI a first-class concern.

## Green-bar checkpoint

- You can write a `JoinSet`/`FuturesUnordered`-based concurrent fetcher from memory.
- You can explain *why* a per-request timeout and an overall deadline are different concerns.
- You can write a `wiremock`-based integration test for any HTTP client.

Phase 2 is complete. Phase 3 — **SQL + Databases (SQLite + Drizzle first, then Postgres + sqlx)** — starts at `curriculum/phase-03-sql-databases/`.
