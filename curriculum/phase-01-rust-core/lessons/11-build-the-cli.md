# Lesson 1.11 — Build `hello-cli` Step by Step

> **The capstone of Phase 1.** You'll build `projects/01-hello-cli/` from scratch — exactly the code already in the repo, but with every step explained. By the end you'll have shipped a clippy-clean, fully-tested CLI.
> **Time:** 90 minutes.

## What we're building

A `wc`-style CLI:

```
$ echo "alpha beta gamma" | hello-cli
      1       3      17

$ hello-cli README.md --lines
     65

$ hello-cli /does/not/exist
hello-cli: /does/not/exist: no such file
$ echo $?
2
```

It teaches one *enterprise habit* in particular: **business logic in `lib.rs`, CLI plumbing in `main.rs`.**

## Step 0 — Browse the finished project

```bash
ls projects/01-hello-cli/
cat projects/01-hello-cli/Cargo.toml
```

You should see:

```
Cargo.toml      README.md       src/            tests/
```

We'll walk through each piece. Don't rebuild — just read along.

## Step 1 — The manifest

`projects/01-hello-cli/Cargo.toml`:

```toml
[package]
name        = "hello-cli"
version     = { workspace = true }
edition     = { workspace = true }
rust-version = { workspace = true }
license     = { workspace = true }
description = "Phase 1 capstone — a small word/line/char counter, fully tested."

[[bin]]
name = "hello-cli"
path = "src/main.rs"

[lib]
path = "src/lib.rs"

[dependencies]
clap     = { workspace = true }
anyhow   = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }
predicates = { workspace = true }
tempfile   = { workspace = true }
```

Five things to notice:

1. **Workspace inheritance.** `{ workspace = true }` reads version/edition/license from the workspace root.
2. **`[[bin]]`** declares the binary target (an executable).
3. **`[lib]`** declares the library target. Yes, the same crate is *both* a binary and a library. The binary uses the library.
4. **`[dependencies]` vs `[dev-dependencies]`.** Production deps vs test-only deps. `assert_cmd` only compiles when you run tests.
5. **`description`** appears on crates.io and in `cargo --help` for this binary. Always fill it in.

## Step 2 — The library (`src/lib.rs`)

The business logic. Pure, allocation-cheap, fully testable.

### 2.1 The `Counts` struct

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
}
```

Derives:
- `Debug` — for `{:?}` printing in tests and debugger output.
- `Clone, Copy` — `usize` triples are cheap; opt into copy semantics.
- `Default` — `Counts::default()` gives all-zeros.
- `PartialEq, Eq` — needed for `assert_eq!` in tests.

### 2.2 The counting function

```rust
impl Counts {
    pub fn count(input: &str) -> Self {
        Self {
            lines: input.lines().count(),
            words: input.split_whitespace().count(),
            chars: input.chars().count(),
        }
    }
}
```

Three iterator chains. Each is `O(n)`. `str::lines` doesn't count a trailing empty line — matches `wc -l` behaviour. `chars()` counts Unicode scalars, not bytes — "café" is 4 chars even though it takes 5 bytes.

### 2.3 `Display` for human output

```rust
impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:>7} {:>7} {:>7}", self.lines, self.words, self.chars)
    }
}
```

`{:>7}` is "right-align in a 7-char field" — same as `wc`'s columns.

### 2.4 `Selection`

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    pub lines: bool,
    pub words: bool,
    pub chars: bool,
}

impl Selection {
    #[must_use]
    pub fn or_default(self) -> Self {
        if self.lines || self.words || self.chars { self } else { Self { lines: true, words: true, chars: true } }
    }

    pub fn render(self, c: Counts) -> String {
        let mut out = String::new();
        if self.lines { write!(out, "{:>7}", c.lines).unwrap(); }
        if self.words {
            if !out.is_empty() { out.push(' '); }
            write!(out, "{:>7}", c.words).unwrap();
        }
        if self.chars {
            if !out.is_empty() { out.push(' '); }
            write!(out, "{:>7}", c.chars).unwrap();
        }
        out
    }
}
```

`or_default` enforces the "no flags = all flags" rule. `render` walks the columns in order. `#[must_use]` on `or_default` keeps clippy pedantic happy and signals "don't ignore this value."

### 2.5 Tests at the bottom

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_zero_of_everything() { ... }
    #[test]
    fn single_line_no_trailing_newline()    { ... }
    #[test]
    fn multiple_lines_with_trailing_newline() { ... }
    #[test]
    fn whitespace_only_has_zero_words()     { ... }
    #[test]
    fn unicode_is_counted_per_scalar_not_byte() { ... }
    #[test]
    fn display_pads_each_field_to_seven_columns() { ... }
    #[test]
    fn selection_or_default_turns_on_everything_when_empty() { ... }
    #[test]
    fn selection_render_respects_chosen_columns() { ... }
}
```

Notice: every public method has at least one test. The test names *are* the spec.

## Step 3 — The binary (`src/main.rs`)

CLI plumbing. Argument parsing, I/O, exit codes.

### 3.1 The `Cli` struct

```rust
#[derive(Debug, Parser)]
#[command(name = "hello-cli", version, about)]
struct Cli {
    file: Option<PathBuf>,
    #[arg(short = 'l', long)] lines: bool,
    #[arg(short = 'w', long)] words: bool,
    #[arg(short = 'c', long)] chars: bool,
}
```

`clap` (derive API) turns this into a fully-fledged parser. `--help`, `--version`, short/long flags, error messages — all generated. The doc comment above `Cli` (in the real file) becomes the help text.

### 3.2 The `main` function

```rust
fn main() -> ExitCode {
    let cli = Cli::parse();

    let input = match read_input(cli.file.as_deref()) {
        Ok(s) => s,
        Err(ReadError::NotFound(p)) => {
            eprintln!("hello-cli: {}: no such file", p.display());
            return ExitCode::from(2);
        }
        Err(ReadError::Io(e)) => {
            eprintln!("hello-cli: I/O error: {e}");
            return ExitCode::from(1);
        }
        Err(ReadError::NotUtf8) => {
            eprintln!("hello-cli: input is not valid UTF-8");
            return ExitCode::from(1);
        }
    };

    let counts = Counts::count(&input);
    let sel = Selection { lines: cli.lines, words: cli.words, chars: cli.chars }.or_default();
    println!("{}", sel.render(counts));
    ExitCode::SUCCESS
}
```

Three things to notice:

1. **`fn main() -> ExitCode`.** Returning an `ExitCode` (or `Result<(), E>`) is the modern way to signal failure. `ExitCode::from(2)` = exit code 2.
2. **Errors are mapped to user-facing messages and codes** *at the boundary*. The library doesn't know about exit codes; `main` does.
3. **`eprintln!` to stderr, `println!` to stdout.** Tools that pipe to other tools should put data on stdout and noise on stderr. Always.

### 3.3 `read_input`

```rust
fn read_input(path: Option<&Path>) -> Result<String, ReadError> {
    let Some(p) = path else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).map_err(ReadError::Io)?;
        return Ok(buf);
    };
    match fs::read(p) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ReadError::NotFound(p.to_path_buf())),
        Err(e) => Err(ReadError::Io(e)),
    }
}
```

Notice the `let Some(p) = path else { ... return ... }` — that early-return pattern from Lesson 1.3. It keeps the "happy" path flat.

## Step 4 — Integration tests (`tests/cli.rs`)

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command { Command::cargo_bin("hello-cli").expect("binary built") }

#[test]
fn prints_help() {
    bin().arg("--help").assert().success()
        .stdout(predicate::str::contains("Count lines, words, and characters"));
}

#[test]
fn missing_file_exits_with_code_2() {
    bin().arg("/path/that/does/not/exist").assert().code(2)
        .stderr(predicate::str::contains("no such file"));
}
```

`assert_cmd::Command::cargo_bin("hello-cli")` builds & locates the binary. Then we assert on stdout, stderr, and exit code.

## Step 5 — Verify, commit, push

```bash
cd /home/user/seeding-testing
make verify              # fmt + clippy + tests
```

You should see `verify: OK`. Then:

```bash
git add projects/01-hello-cli
git commit -m "feat(phase-01): add hello-cli capstone with tests"
git push
gh run watch
```

Watch CI tick green. Take a screenshot. You shipped your first real Rust project.

## Why this matters

- **Splitting lib + main is a habit.** Every real service in the capstone follows this pattern. Business logic in a library; thin CLI/HTTP entry point.
- **Exit codes are an API.** Scripts piping into your CLI will check `$?`. Document and test the codes you return.
- **Integration tests prove the contract.** They use the same surface a user does. Refactor freely; if these still pass, you didn't break anyone.

## Green-bar checkpoint

You can do every line of this lesson without looking. You can explain *why* `Selection::or_default()` returns `Self` instead of mutating. You can replicate this pattern in a brand-new project tomorrow.

Phase 1 is complete. Phase 2 — Async + Tokio — starts at `curriculum/phase-02-async-tokio/`.
