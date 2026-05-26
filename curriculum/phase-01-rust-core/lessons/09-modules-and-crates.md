# Lesson 1.9 — Modules and Crates

> **Concept first:** as projects grow, you want to split code across files and crates. `mod` carves a single crate into modules; the workspace ties multiple crates together.
> **Time:** 30 minutes.

## Vocabulary

- **Crate** — the unit of compilation. One `Cargo.toml`. Either a binary (`main.rs`) or a library (`lib.rs`).
- **Module** — a namespace *inside* a crate. Declared with `mod`.
- **Workspace** — a directory with a root `Cargo.toml` listing several crates that share `Cargo.lock` and `target/`.

We've already used all three:

- This repo is a **workspace**.
- `hello-cli` is a **crate** (it's *both* binary and library — `src/main.rs` *and* `src/lib.rs`).
- `tests` (the `tests/` directory) is automatically treated as an integration test target.

## Modules inside a single file

```rust
// src/lib.rs
pub fn public_thing() {}

fn private_thing() {}

mod helpers {
    pub fn hello() { println!("hello from helpers"); }
    fn secret() {}
}

pub fn use_helpers() {
    helpers::hello();
}
```

Three rules:

- `pub` makes an item visible outside its module.
- Without `pub`, items are private to the module they're in.
- Sub-modules inherit the parent's visibility hierarchy.

## Modules across files

Two layouts, both supported:

### Layout A — `mod_name.rs` next to `lib.rs`

```
src/
├── lib.rs
├── helpers.rs        ← module `helpers`
└── billing.rs        ← module `billing`
```

In `lib.rs`:

```rust
mod helpers;          // pulls in helpers.rs
mod billing;
```

### Layout B — `mod_name/mod.rs` (sub-tree)

```
src/
├── lib.rs
└── billing/
    ├── mod.rs        ← the `billing` module
    ├── money.rs      ← sub-module billing::money
    └── stripe.rs     ← sub-module billing::stripe
```

In `lib.rs`: `mod billing;`. In `billing/mod.rs`: `pub mod money; pub mod stripe;`.

Use Layout A for simple modules; Layout B when a module wants its own sub-tree.

## `use` — bringing names into scope

```rust
use std::collections::HashMap;
use std::io::{self, Read, Write};
use crate::billing::Money;            // absolute path within this crate
use super::utils::format_cents;       // parent module
use self::sub::thing;                 // explicit current module

fn main() {
    let _ = HashMap::<String, i32>::new();
}
```

- `crate::…` — absolute from the crate root.
- `super::…` — parent module.
- `self::…` — current module.

We prefer absolute paths (`crate::…`) in real code — they're stable when files move.

## Re-exporting with `pub use`

To present a clean public API without exposing internal structure:

```rust
// src/lib.rs
mod billing;                          // private module
pub use billing::Money;               // but re-export this type
```

External callers see `mycrate::Money`, not `mycrate::billing::Money`. We use this pattern heavily in the capstone API crate.

## Visibility tiers beyond `pub`

| Marker | Meaning |
|---|---|
| (none) | private to the module |
| `pub` | public to anyone |
| `pub(crate)` | public within this crate, hidden from external users |
| `pub(super)` | public to the parent module only |
| `pub(in crate::api)` | public to a named path |

`pub(crate)` is the most common after `pub`. Use it for "internal API" types.

## Workspaces

A workspace lets one repo hold many crates. The root `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = [
    "projects/01-hello-cli",
    "apps/memberclub/api",
    "apps/memberclub/db",
]

[workspace.dependencies]
clap = { version = "4.6", features = ["derive"] }
```

Each member crate inherits with `clap = { workspace = true }` in its own `Cargo.toml`. That keeps versions in lock-step.

Benefits:

- One `Cargo.lock`. Reproducible across the whole project.
- One `target/`. Shared build artifacts across crates.
- One command builds everything: `cargo build --workspace`.

We use a single workspace for the entire curriculum repo.

## When to split into a new crate

You're ready for a new crate when:

1. Its compile time would dominate the parent crate.
2. It needs different dependencies (e.g. a `cli` crate doesn't need `axum`).
3. You want to publish it separately on crates.io.
4. You want strict layering — e.g. `memberclub-domain` (pure logic) can't depend on `memberclub-axum` (web). Splitting prevents accidental cycles.

For the capstone, we'll split into roughly:

```
apps/memberclub/
├── api/          ← Axum binary
├── domain/       ← pure business logic (no I/O)
├── infra/        ← sqlx, stripe, mail adapters
└── db/           ← migrations + seed binary
```

## Why this matters

- **Modules make code readable.** A flat `src/lib.rs` with 4000 lines is unreadable. Twenty 200-line modules are.
- **Workspaces enforce architectural boundaries.** If `domain` doesn't depend on `axum`, you literally cannot accidentally couple business rules to HTTP.
- **Public APIs are a contract.** `pub use` lets you keep an internal hierarchy private while exposing a flat, stable surface.

## Green-bar checkpoint

- You can split a single-file library into `src/lib.rs` + `src/billing.rs` + `src/billing/money.rs`.
- You can explain why `pub(crate)` exists.
- You can read a workspace `Cargo.toml` and predict what `cargo build --workspace` will compile.

Next: `lessons/10-testing.md`.
