# Development environment

Three on-ramps, pick what you prefer.

## Option 1 — devcontainer (zero local install)

The fastest path on any machine with VS Code or GitHub Codespaces.

```bash
gh repo clone billyribeiro-ux/seeding-testing
code seeding-testing                # VS Code prompts to reopen in container
```

The `.devcontainer/devcontainer.json` config builds an image with:
  * Rust 1.95 stable + clippy + rustfmt + rust-analyzer
  * Node 22 + pnpm 10 (via corepack)
  * Docker-in-Docker (so `make up` works inside the container)
  * GitHub CLI

`postCreateCommand` runs `.devcontainer/post-create.sh` which:
  * installs the cargo binaries the curriculum uses (`cargo-nextest`,
    `sqlx-cli`, `cargo-deny`, `cargo-audit`, `cargo-watch`)
  * wires the pre-commit hook (`git config core.hooksPath .githooks`)

When the container is built, `make verify` should pass on the first
try.

## Option 2 — local install

Follow `curriculum/phase-00-foundations/lessons/02-toolchain.md`:
Rust via `rustup`, Node via `mise` or `fnm`, Docker, GitHub CLI,
Stripe CLI. Then run `.devcontainer/post-create.sh` to wire the same
cargo binaries + pre-commit hook.

## Option 3 — Nix (TODO)

A `flake.nix` would give bit-for-bit reproducible toolchains. Not yet
shipped; tracked in the issue tracker.

# `cargo xtask` — repo-meta commands

The `xtask` workspace crate is the modern Rust idiom for "scripts that
build the build." Bash doesn't get type-checked; xtask does. Discover
the catalog with `cargo xtask --help`:

```
cargo xtask verify             # fmt + clippy -D warnings + nextest + sqlx + deny + audit
cargo xtask openapi-bless      # regen the OpenAPI snapshot after an API change
cargo xtask versions           # print every crate's version
cargo xtask bump-version 0.2.0 # bump the workspace version
cargo xtask check-migrations   # sanity-check timestamp ordering
```

`cargo xtask verify` is equivalent to `make verify` but discoverable
from the cargo subcommand catalog (Tab-complete works).

# Pre-commit hook

`.githooks/pre-commit` runs on every `git commit`. It's conservative —
it only checks files staged in this commit, so a typo in an unrelated
file doesn't block a focused commit. What it does check:

  * `rustfmt --check` on every staged `.rs` file
  * `cargo clippy --all-targets -- -D warnings` on every touched
    workspace member
  * Refuses to commit edits to existing migration files (they're
    immutable once shipped — add a new one instead)
  * Refuses to commit anything that looks like a Stripe live key,
    webhook secret, or private RSA key

Enable for a fresh clone:

```bash
git config --local core.hooksPath .githooks
```

The devcontainer does this for you.
