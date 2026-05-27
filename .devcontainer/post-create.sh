#!/usr/bin/env bash
# Bootstrap a freshly-built devcontainer. Idempotent — re-running this
# is safe.

set -euo pipefail

echo "==> rustup components"
rustup component add clippy rustfmt rust-analyzer rust-src

echo "==> cargo tools"
# `--locked` so the install is reproducible across re-runs.
cargo install --locked cargo-nextest sqlx-cli@0.8.6 cargo-deny cargo-audit cargo-watch

echo "==> pnpm"
# corepack ships with Node 22; enables pnpm via the lockfile's "packageManager".
corepack enable
corepack prepare pnpm@10.0.0 --activate

echo "==> pre-commit hook"
# Wire the repo-local hook so a developer's first commit out of the
# container is already formatted and clippy-clean.
git config --local core.hooksPath .githooks

echo "==> done. Try: make verify"
