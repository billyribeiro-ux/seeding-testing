# Lesson 1.10 — Testing

> **Concept first:** tests are how yesterday's work stays alive. Rust ships test infrastructure with the language — no extra setup. Two flavours: unit tests live with the code, integration tests live alongside it.
> **Time:** 45 minutes.

## Why test?

Three reasons, in order of importance:

1. **Refactoring safety.** With tests, you can change code with confidence; without them, every refactor is gambling.
2. **Documentation.** A good test explains *how* something is supposed to be used and *what* it returns.
3. **Bug prevention.** Catching a bug in CI is 100× cheaper than catching it in production.

Coverage isn't a number to game. It's a leading indicator of *did you think about what could go wrong.*

## Unit tests live with the code

Inside any `.rs` file, you can add a test module:

```rust
// src/lib.rs
pub fn double(n: i32) -> i32 { n * 2 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_zero() {
        assert_eq!(double(0), 0);
    }

    #[test]
    fn doubles_positive() {
        assert_eq!(double(5), 10);
    }
}
```

- `#[cfg(test)]` means "only compile this when running tests."
- `#[test]` marks a function as a test.
- Tests live in the same crate, so they can call private functions.

Run them:

```bash
cargo test                   # all crates in the workspace
cargo test -p hello-cli      # just this crate
cargo test doubles_zero      # by name (substring match)
cargo test -- --nocapture    # don't swallow println!s (helpful when debugging)
```

## Integration tests live in `tests/`

Inside any crate, a `tests/` directory holds *integration tests* — files compiled as separate binaries that only see the crate's *public* API:

```
hello-cli/
├── src/
│   ├── lib.rs
│   └── main.rs
└── tests/
    └── cli.rs                ← integration tests
```

`tests/cli.rs` can only call `hello_cli::Counts` (public items). It cannot reach into private internals — which is exactly what you want: integration tests should treat the crate as a black box.

You've already seen one: `projects/01-hello-cli/tests/cli.rs` spawns the actual binary via `assert_cmd` and asserts on its stdout/stderr/exit code.

## Assertions

The three macros you'll use 99% of the time:

```rust
assert!(condition);                        // condition must be true
assert_eq!(a, b);                          // a == b (prints both on failure)
assert_ne!(a, b);                          // a != b
```

With custom messages:

```rust
assert!(user.is_active(), "user {:?} should be active", user);
```

Custom-formatted output on failure is the difference between a useful test report and a confused engineer at 2 AM.

## Testing failures

```rust
#[test]
#[should_panic(expected = "divide by zero")]
fn panics_on_zero() {
    divide(1, 0);
}
```

`#[should_panic]` asserts the test panics; `expected = "…"` asserts the panic message contains a substring. Use sparingly — usually `Result`-returning APIs are cleaner.

For `Result`:

```rust
#[test]
fn parses_valid_email() -> Result<(), ParseError> {
    let e = parse_email("a@b.com")?;
    assert_eq!(e.domain(), "b.com");
    Ok(())
}
```

A `Result`-returning test fails on `Err`. Cleaner than `.unwrap()`.

## `cargo nextest` — the faster, prettier runner

`cargo test` is fine, but it has limitations: serial reporting, slow startup, less-pretty output. `cargo nextest` is the modern alternative.

Install once:

```bash
cargo install --locked cargo-nextest
```

Then:

```bash
cargo nextest run --workspace
```

You get parallel execution, per-test isolation, and a clean summary. Our `Makefile` and CI prefer `nextest` when available.

## Property tests with `proptest`

When testing pure math (money calculations, parsers, encoders), unit tests cover the cases *you thought of*. Property tests generate *thousands* of random inputs and check that an invariant holds:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn doubling_is_associative(a in 0i64..1_000, b in 0i64..1_000) {
        let lhs = (a + b) * 2;
        let rhs = a * 2 + b * 2;
        prop_assert_eq!(lhs, rhs);
    }
}
```

We use proptest extensively in Phase 8 (Stripe + Money) to prove that `split_proportional` always sums to the original cents.

## Snapshot tests with `insta`

For testing rendered output (JSON responses, error messages, HTML), `insta` records a "golden" snapshot, diffs against it on subsequent runs, and lets you accept changes interactively:

```rust
#[test]
fn renders_user_to_json() {
    let u = User { id: 1, email: "a@b.com".into() };
    insta::assert_json_snapshot!(u);
}
```

The first run creates `snapshots/<test>.snap`. Later runs compare. Run `cargo insta accept` to bless legitimate changes.

## Testing with a real database

For sqlx code, we run integration tests against a real Postgres via `testcontainers-rs` — it spins up a fresh, ephemeral Postgres container per test run. No mocks. Phase 5 covers the setup in detail.

```rust
use testcontainers_modules::postgres::Postgres;
use testcontainers::runners::AsyncRunner;

#[tokio::test]
async fn creates_a_user() {
    let pg = Postgres::default().start().await.unwrap();
    let url = format!("postgres://postgres@127.0.0.1:{}/postgres", pg.get_host_port_ipv4(5432).await.unwrap());
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    // ... migrations + assertions ...
}
```

The lesson: **don't mock the database**. Tests should exercise the real SQL — that's where most bugs hide.

## What to test

A practical checklist for every public function:

- **Happy path.** Does it return what we expect for valid input?
- **Edge cases.** Empty inputs, max values, Unicode, whitespace-only?
- **Error paths.** What happens with bad input, network failures, malformed JSON?
- **Idempotency** (state-changing functions). Calling it twice is the same as calling it once?
- **Boundary conditions.** Off-by-one, zero, negative, max?

For pure functions, target ≥90% line coverage. For I/O-heavy code, target *every interesting behaviour* — a single integration test can cover an entire happy path.

## A worked example: `hello-cli`'s test suite

```rust
// src/lib.rs — unit tests for pure logic
#[test]
fn empty_string_has_zero_of_everything() { ... }
#[test]
fn whitespace_only_has_zero_words() { ... }
#[test]
fn unicode_is_counted_per_scalar_not_byte() { ... }
```

```rust
// tests/cli.rs — integration tests for the binary
#[test]
fn missing_file_exits_with_code_2() { ... }
#[test]
fn reads_stdin_when_no_file() { ... }
```

Unit tests cover the logic; integration tests cover the CLI surface. Both are necessary.

## Why this matters

- **Tests are the only thing that lets you refactor.** A senior engineer who can't refactor confidently is just a junior engineer with seniority.
- **Tests are documentation that doesn't lie.** Comments rot; tests fail loudly.
- **CI without tests is theatre.** Green CI doesn't mean "code works" — it means "the things you bothered to check are still working."

## Green-bar checkpoint

- You can run `cargo test`, `cargo test -p <crate>`, and `cargo nextest run`.
- You can write a unit test that calls a private function.
- You can write an integration test that exercises only the public API.
- You can articulate when to use proptest vs unit tests.

Next: `lessons/11-build-the-cli.md` — the Phase 1 capstone walk-through.
