# Phase 1 — Rubric

How a Principal Engineer would grade your Phase 1 work.

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Cargo fluency** | Can run `cargo run` | Comfortable with build/check/clippy/fmt/test/doc; knows when to use each | Curates `Cargo.toml`, uses workspace inheritance, profiles, feature flags |
| **Ownership** | Sprinkles `.clone()` to make errors go away | Knows when to clone, when to borrow, when to move; reads ownership errors and fixes them | Designs APIs that minimize moves; uses newtypes; can refactor `clone`-heavy code into reference-based |
| **Borrowing** | Confused by the borrow checker | Reads compiler suggestions and applies them | Internalizes "many readers OR one writer"; writes functions that take the weakest reference that does the job |
| **Types & enums** | Uses `String` and `Vec` for everything | Models domains with structs and enums; uses `Option` and `Result` correctly | Makes invalid states unrepresentable; uses newtypes for IDs and units; favours enums for state machines |
| **Traits / generics** | Reads but doesn't write them | Implements `Display`, `From`, custom traits when useful | Knows when to reach for `impl Trait` vs `dyn Trait`; uses trait bounds as documentation |
| **Error handling** | `.unwrap()` everywhere | Uses `?` and `Result`; writes typed errors with `thiserror` | Cleanly separates library-level typed errors from application-level `anyhow`; maps errors at boundaries |
| **Testing** | Writes a happy-path test | Covers edge cases, error paths; uses integration tests via `assert_cmd` | Uses property tests for pure logic; snapshot tests for output; integration tests against real I/O |
| **Module design** | Single-file crate | Splits into modules thoughtfully; uses `pub(crate)` | Designs workspace layouts; enforces architectural layering via crate boundaries |
| **CLI craftsmanship** | Prints something to the terminal | Uses `clap` derive, proper exit codes, stderr for errors | Documents exit codes, supports `--help`/`--version`, integrates with pipes (`stdin`/`stdout`) |

## Self-check before moving to Phase 2

- [ ] `make verify` passes locally and in CI.
- [ ] You completed Exercises E1.1 – E1.6 (E1.7 and E1.8 are stretch).
- [ ] You can read `projects/01-hello-cli/src/lib.rs` and `tests/cli.rs` from memory after closing the file.
- [ ] You can explain the difference between `&str`, `String`, `&String`, `Box<str>`.
- [ ] You can explain when to reach for `thiserror` vs `anyhow`.
- [ ] You committed all your exercise solutions and pushed them.
- [ ] CI is green on your branch.

Phase 2 — Async + Tokio — assumes you've internalized everything above. If any box is empty, stay another few days.
