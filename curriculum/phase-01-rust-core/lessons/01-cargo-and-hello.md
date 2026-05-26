# Lesson 1.1 — Cargo and Hello, World

> **Concept first:** what Cargo is and why every Rust project uses it.
> **Then:** create a new project, build it, run it.
> **Time:** 30 minutes.

## What is Cargo?

When you write a Rust program of any real size, you need three things:

1. A **compiler** to turn your `.rs` files into a binary.
2. A **dependency manager** to pull in libraries other people wrote (so you don't reinvent JSON parsing).
3. A **build orchestrator** to run the right commands in the right order.

In some languages each of these is a separate tool. Rust ships them as one: **Cargo**. Cargo *is* the Rust project's contract with the outside world. Every Rust codebase you'll ever touch has a `Cargo.toml` at its root, and every operation — build, run, test, format, lint, publish — goes through `cargo`.

> Mental model: `cargo` is to Rust what `make` is to C — only standardized, batteries-included, and consistent across every project on Earth.

## Anatomy of a Cargo project

```
my-project/
├── Cargo.toml         ← project manifest: name, version, dependencies
├── Cargo.lock         ← exact resolved versions (commit this for binaries)
├── src/
│   └── main.rs        ← the entry point for a binary crate
└── target/            ← build artifacts (gitignored)
```

A **crate** is a single compilation unit. There are two flavours:

- **Binary crate** — has `src/main.rs`. Produces an executable.
- **Library crate** — has `src/lib.rs`. Produces a `.rlib` other crates can link against.

A crate can be both (we'll do that in Phase 1's capstone).

A **workspace** is a collection of crates that share a `Cargo.lock` and a `target/` directory. This repo is a workspace; its root `Cargo.toml` lists members under `[workspace]`.

## Hello, world — step by step

### 1. Create the project

From the curriculum repo root:

```bash
cargo new --bin hello-world --vcs none
```

What that does:
- Creates `hello-world/`.
- Creates `hello-world/Cargo.toml` with sensible defaults.
- Creates `hello-world/src/main.rs` with a starter "Hello, world!" program.
- `--vcs none` skips creating a `.git/` (we're already inside a git repo).

> We'll put this *outside* the workspace for the lesson. Add `hello-world/` to `.gitignore` if you don't want to commit it, or just delete it after.

### 2. Look at what Cargo gave you

```bash
cat hello-world/Cargo.toml
```

You'll see:

```toml
[package]
name    = "hello-world"
version = "0.1.0"
edition = "2024"

[dependencies]
```

- `name` — what the world calls this crate (must be unique on crates.io if you publish).
- `version` — semver.
- `edition` — which dialect of Rust to compile against (2015, 2018, 2021, 2024). Editions are *opt-in* — you can stay on 2021 forever; we use 2024 in this curriculum.
- `[dependencies]` — where you add libraries.

And `src/main.rs`:

```rust
fn main() {
    println!("Hello, world!");
}
```

Two pieces of vocabulary:
- `fn main()` declares a function called `main`. Every executable Rust program has one. It's where execution starts.
- `println!` is a **macro** (the `!` is the giveaway). It expands to code that prints to standard output.

### 3. Build it

```bash
cd hello-world
cargo build
```

You'll see something like:

```
   Compiling hello-world v0.1.0 (/path/to/hello-world)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
```

- "dev profile" means debug build (fast to compile, slow to run, has debug symbols).
- The binary is at `target/debug/hello-world`.

Try running it directly:

```bash
./target/debug/hello-world
```

You should see: `Hello, world!`.

### 4. Run it the Cargo way

You'll almost never invoke the binary directly during development. Instead:

```bash
cargo run
```

This:
1. Rebuilds if needed.
2. Runs the binary.

### 5. Release build

```bash
cargo build --release
./target/release/hello-world
```

- Slower to compile, much faster to run.
- Binary at `target/release/hello-world`.

### 6. Check vs build

When you only want to know *"would this compile?"* without producing a binary:

```bash
cargo check
```

It's 2–4× faster than `cargo build`. Use it constantly while editing.

## The cargo cheat-sheet

| Command | What it does | When to use |
|---|---|---|
| `cargo new --bin NAME` | Create a binary crate | New project |
| `cargo new --lib NAME` | Create a library crate | Reusable code |
| `cargo check` | Type-check without producing a binary | Fast feedback while editing |
| `cargo build` | Debug build | Developing |
| `cargo build --release` | Optimized build | Production / benchmarking |
| `cargo run` | Build + run | Daily dev |
| `cargo run -- ARGS` | Build + run with CLI args after `--` | Try a flag your program accepts |
| `cargo test` | Run all tests | Before commit |
| `cargo clippy` | Lint | Before commit |
| `cargo fmt` | Auto-format | Before commit |
| `cargo doc --open` | Build & open API docs in your browser | When reading a crate's API |
| `cargo tree` | Show the dependency graph | When something pulls in too much |
| `cargo update` | Update `Cargo.lock` to the latest versions allowed by `Cargo.toml` | Periodically |

## A tiny extension: pass an argument

Modify `src/main.rs`:

```rust
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let name = args.get(1).map(String::as_str).unwrap_or("world");
    println!("Hello, {name}!");
}
```

Then:

```bash
cargo run -- Alice
# Hello, Alice!
```

Don't worry if `Vec<String>` and `.map(String::as_str)` look mysterious — next lessons explain every piece.

## Green-bar checkpoint

You can:

- `cargo new --bin <name>` and immediately `cargo run` to see "Hello, world!"
- Explain the difference between `cargo check`, `cargo build`, `cargo run`, and `cargo build --release`.
- Explain what `Cargo.toml` and `Cargo.lock` are for.

Next: `lessons/02-values-and-types.md`.
