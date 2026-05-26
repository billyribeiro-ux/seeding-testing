# Lesson 5.2 — `cargo nextest`

> **Concept first:** `cargo test` works. `cargo nextest` is just *better* at it — parallel by default, isolated, prettier, with retries, partitioning, JUnit XML, and structured output.
> **Time:** 15 minutes.

## Install

```bash
cargo install --locked cargo-nextest
```

The repo's `Makefile` already prefers `nextest` when available.

## The basic loop

```bash
cargo nextest run                       # run everything
cargo nextest run -p notes-api          # one crate
cargo nextest run -E 'test(/create.*/)' # filter expression
cargo nextest run --no-fail-fast        # don't stop on first failure
```

## Process isolation

Every test runs in *its own process*. Three consequences:

1. **Static state in one test cannot leak into another.** A global `OnceCell` initialized in test A is a fresh one in test B.
2. **Crashes/segfaults are isolated.** A panic in one test doesn't take down the rest.
3. **Parallelism is automatic.** nextest defaults to one process per CPU core.

`cargo test` runs all tests in a single process by default. nextest is strictly safer.

## Retries for flaky externals

If you have a small number of inherently flaky tests (e.g. exercising a real Stripe sandbox), allow retries:

```toml
# .config/nextest.toml
[profile.default]
retries = 2

[[profile.default.overrides]]
filter = 'test(/stripe::/)'
retries = 5
```

The default profile retries up to 2; tests matching `stripe::` get up to 5.

A retry should never *hide* a bug. Tag the test, retry to absorb sandbox noise, but if a non-flaky test is suddenly retrying, that's an alarm.

## Partitioning for CI

For very large workspaces you can split the test set into N shards and run them on N runners:

```bash
cargo nextest run --partition count:1/4    # shard 1 of 4
cargo nextest run --partition count:2/4
```

We don't need this yet. We will when the test count crosses ~5 minutes per run.

## JUnit XML for CI

```toml
[profile.ci]
junit = { path = "target/nextest/ci/junit.xml" }
```

Then in GHA:

```yaml
- run: cargo nextest run --profile ci
- uses: dorny/test-reporter@v1
  with:
    name: cargo nextest
    path: target/nextest/ci/junit.xml
    reporter: java-junit
```

The PR page now shows the test report inline. Failing tests get a clickable link.

## Status and slow-test warnings

```toml
[profile.default]
slow-timeout = { period = "30s", terminate-after = 2 }
```

Tests slower than 30s get a yellow warning the first time, are terminated on the third. Catches accidentally-hung tests.

## What nextest *doesn't* do

- Doctests. Run them with `cargo test --doc` (or skip them — most teams skip).
- Benchmarks. Use `cargo bench` or `divan`.

## Why this matters

- **Process isolation eliminates a whole class of flakiness** — accidental state leaks.
- **Retries should be tagged, not global.** Knowing which tests *expected* to retry is signal.
- **JUnit XML is the lingua franca of CI test reporters.** Free with one config flag.

## Green-bar checkpoint

- You can `cargo nextest run` and see green output.
- You can write a `.config/nextest.toml` with one retry rule and one slow-timeout rule.
- You can articulate the difference between flaky-and-retry-it vs flaky-and-fix-it.

Next: `lessons/03-integration-with-testcontainers.md`.
