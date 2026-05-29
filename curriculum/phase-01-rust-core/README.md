# Phase 1 — Rust Core

> **Audience:** you just finished Phase 0.
> **Outcome:** you understand the language well enough to read most Rust code, and you've shipped a working CLI tool with tests.
> **Time:** 3–4 weeks at 10–15 hrs/week.

## The mental model

Rust's reputation is built on three big ideas. We'll build each one slowly, concept first, then code:

1. **The compiler is your pair programmer.** Most languages run your code, then crash if it's wrong. Rust refuses to *compile* code that's wrong — and it explains why in plain English.
2. **Ownership.** Every value has exactly one owner. When the owner goes out of scope, the value's memory is reclaimed automatically. No garbage collector, no `free()`, no use-after-free bugs.
3. **Fearless mutation.** You can have *many readers* OR *one writer*, never both at once. This single rule eliminates data races at compile time.

If those sentences feel abstract, that's fine — by the end of this phase they'll feel obvious.

## The phase plan — concepts and code, step by step

| Lesson | Concept | What you'll type |
|---|---|---|
| `lessons/01-cargo-and-hello.md` | What Cargo is and why it exists | `cargo new`, `cargo run`, `cargo build` |
| `lessons/02-values-and-types.md` | Numbers, strings, booleans, tuples, arrays | Bind variables, do arithmetic |
| `lessons/03-control-flow.md` | `if`, `match`, `for`, `while`, `loop` | Branch and iterate |
| `lessons/04-ownership.md` | The three ownership rules, with pictures | Move, copy, scope |
| `lessons/05-borrowing-and-lifetimes.md` | `&T`, `&mut T`, lifetimes for beginners | Pass references without losing ownership |
| `lessons/06-structs-and-enums.md` | Custom types | Build a `Point`, a `Shape`, a `Result`-like enum |
| `lessons/07-traits-and-generics.md` | Behavior shared across types | Implement `Display`, write a generic `largest` function |
| `lessons/08-error-handling.md` | `Result<T, E>`, the `?` operator, `thiserror`, `anyhow` | Propagate errors cleanly |
| `lessons/09-modules-and-crates.md` | `mod`, `pub`, `use`, workspaces | Organize a small project |
| `lessons/10-testing.md` | Unit tests, integration tests, `cargo test`, `cargo nextest` | Write tests that catch regressions |
| `lessons/11-build-the-cli.md` | Build `projects/01-hello-cli` step by step | The capstone of this phase |

Each lesson is 20–60 minutes. Do them in order. Don't skip the "Why" sections — they're the difference between writing Rust and *understanding* Rust.

## The capstone drill — `projects/01-hello-cli`

A real, small command-line tool that:

- Reads a file (or stdin) and counts **lines**, **words**, and **characters**.
- Accepts flags via `clap`: `--lines`, `--words`, `--chars` (passing none prints all three).
- Returns proper exit codes (`0` success, `2` for missing file).
- Has **unit tests** for every public function in `src/lib.rs`.
- Has **integration tests** in `tests/` that run the actual binary via `assert_cmd`.
- Builds clean with `cargo clippy -- -D warnings`.

By the end of this phase, you'll have written, tested, committed, and pushed this CLI. CI will be green.

## Green-bar checkpoint

You're done with Phase 1 when:

```bash
cd projects/01-hello-cli
cargo build --release           # OK
cargo test                      # all tests pass
cargo clippy -- -D warnings     # no warnings
./target/release/hello-cli --help                  # prints usage
echo "one two three" | ./target/release/hello-cli  # prints counts
```

…and your push has CI green via `gh run watch`.

## What's next

Phase 2 — **Async + Tokio**. We turn that synchronous CLI into a concurrent HTTP client. You'll meet `Future`, `.await`, and `tokio::spawn`.
