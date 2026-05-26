# Lesson 5.7 — Coverage is a Leading Indicator, Not a Goal

> **Concept first:** coverage tells you what code is *exercised by tests*. It doesn't tell you whether the test asserts the right thing. Use it to find blind spots, not as a finish line.
> **Time:** 15 minutes.

## What coverage measures

| Metric | What it counts |
|---|---|
| **Line** | Lines of source code that were executed during the test run |
| **Branch** | Each `if`/`else`/`match` arm taken at least once |
| **Region** | Sub-expression-level granularity (LLVM-instrument-based) |
| **Function** | Each function called at least once |

Higher is generally better. But high coverage doesn't equal *good* tests.

## The cheap-coverage trap

```rust
#[test]
fn test_create() {
    let result = create_user("alice@b.com");
    let _ = result; // never asserted
}
```

100% coverage. Zero confidence. The test runs the code but doesn't check the outcome.

This is why we say *leading indicator* — a low coverage number tells you something is definitely untested; a high number only tells you the code ran.

## Install and run

```bash
cargo install --locked cargo-llvm-cov
cargo llvm-cov --workspace                          # human-readable report
cargo llvm-cov --workspace --lcov --output-path lcov.info     # for Codecov / SonarQube / GitHub
cargo llvm-cov report --summary-only                # one line per crate
cargo llvm-cov --fail-under-lines 80                # CI gate
```

Add a `cov` target to the Makefile:

```makefile
cov: ## Generate coverage report
	cargo llvm-cov --workspace --lcov --output-path lcov.info
	cargo llvm-cov report --summary-only
```

## What to gate on

| Gate | Rationale |
|---|---|
| **Whole-workspace ≥ 80% lines** | Reasonable floor; not punishing |
| **Per-crate gate for pure logic crates ≥ 95%** | Money math, validators — there's no reason for any line to be untested |
| **No drop > 5% PR-over-PR** | Catches "I added 200 lines without tests" |
| **`unsafe` blocks excluded** | If you write any, they have *separate* scrutiny |

Don't gate on 100%. That forces silly tests just to cover trivial getters and pushes engineers toward gaming the metric.

## What to *not* gate on

- **Test code coverage** — pointless.
- **Generated code** (sqlx `query!` macros, OpenAPI clients) — exclude from coverage.
- **`main.rs`** — too thin to be meaningful.

## Reading a coverage report

```
Filename            Regions    Missed Regions    Cover    Functions    Missed
src/lib.rs               45                 2    95.6%           12         0
src/main.rs              18                18     0.0%            4         4
```

`main.rs` at 0% is normal — it's only run via the binary, not by tests. Add `[lib]` and put the testable logic there; `main` calls `lib::run()`.

## Coverage diff in PRs

Tools like Codecov post a comment on each PR with the line-level diff:

```
+ src/billing/refund.rs            +120 lines, 87% covered (was 92%)
```

A tiny dip is normal. A 10% dip on a PR full of new code without tests is a conversation.

## What coverage *can't* tell you

- **Is the test asserting the right thing?** Read the test, not the number.
- **Are the integration boundaries tested?** Use the trophy/pyramid.
- **Are there edge cases the test missed?** Property tests answer that, not coverage.

## Why this matters

- **A low number is signal you can act on.** "The auth module is 30% covered" — go look.
- **A high number is permission to refactor.** "The whole crate is 95% covered" — change implementation freely.
- **Gating in CI prevents *gradual* erosion.** A team that's never measured coverage drifts down over time.

## Green-bar checkpoint

- You can run `cargo llvm-cov` and read the per-file table.
- You can articulate why 100% coverage isn't the goal.
- You can pick which directory or file should be excluded and explain why.

Next: `lessons/08-build-seed-cli.md`.
