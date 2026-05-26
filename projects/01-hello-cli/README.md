# projects/01-hello-cli

The Phase 1 capstone drill: a small `wc`-like CLI that counts lines, words, and characters in a file or stdin.

## What it teaches

- Splitting business logic (`src/lib.rs`) from CLI plumbing (`src/main.rs`) — the **first enterprise habit**.
- Using `clap` (derive API) for arg parsing.
- Returning proper exit codes from `main`.
- Unit tests for pure functions + integration tests that spawn the binary.
- Workspace-shared dependencies (`workspace = true`).
- Clippy with `pedantic` lints enabled and a forbidding `unsafe_code` lint.

## Run it

```bash
# From the curriculum repo root:
cargo run -p hello-cli -- --help
echo "hello world" | cargo run -p hello-cli --
cargo run -p hello-cli -- README.md --lines
```

## Test it

```bash
cargo test    -p hello-cli              # unit + integration
cargo clippy  -p hello-cli -- -D warnings
cargo fmt     -p hello-cli -- --check
```

Or the whole-workspace gate (what CI runs):

```bash
make verify
```

## Read it

Suggested reading order:

1. `Cargo.toml` — see how workspace inheritance works.
2. `src/lib.rs` — the pure logic, with unit tests at the bottom.
3. `src/main.rs` — the CLI wrapper, with exit codes and a custom `ReadError`.
4. `tests/cli.rs` — integration tests via `assert_cmd`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic failure (I/O, non-UTF-8 input) |
| 2 | Bad input (missing file, invalid args) |
