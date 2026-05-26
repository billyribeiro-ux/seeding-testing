# Troubleshooting Guide

> When something breaks, look here first. Entries are added as we hit (and fix) real failures during authoring. Format: **Symptom → Likely Cause → First-line Fix**.

If your symptom is not here:

1. Read the error *out loud*. Often the compiler / runtime already told you the answer.
2. `cargo clean && cargo build` (or `pnpm install` / `docker compose down -v && docker compose up -d`).
3. Search the closed issues of the failing crate on GitHub (`gh issue list --repo tokio-rs/axum --state closed --search "your error"`).
4. Open a discussion in the curriculum repo — and once you have an answer, **add it here**. The guide grows with the curriculum.

---

## Table of Contents

- [Toolchain & install](#toolchain--install)
- [Rust / cargo](#rust--cargo)
- [Docker & Compose](#docker--compose)
- [PostgreSQL & sqlx](#postgresql--sqlx)
- [Auth (argon2, JWT, sessions)](#auth-argon2-jwt-sessions)
- [Stripe](#stripe)
- [SvelteKit](#sveltekit)
- [MCP servers](#mcp-servers)
- [Git & GitHub](#git--github)
- [CI](#ci)

---

## Toolchain & install

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `error: linker cc not found` | Missing build essentials | Debian/Ubuntu: `sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev`. macOS: `xcode-select --install`. |
| `rustup` ships `1.94.x` even though we want `1.95` | Stable hasn't been pulled yet on this machine | `rustup update stable` then `rustc --version`. |
| `command not found: cargo` after install | `~/.cargo/bin` not on PATH | Add `export PATH="$HOME/.cargo/bin:$PATH"` to `~/.bashrc` (or `~/.zshrc`) and `source` it. |
| `command not found: pnpm` | Corepack not enabled | `corepack enable && corepack prepare pnpm@latest --activate`. |
| `command not found: gh` | GitHub CLI not installed | Linux: `sudo apt install gh` (or follow https://cli.github.com). macOS: `brew install gh`. |

## Rust / cargo

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `cannot find macro query! in this scope` | `sqlx` `macros` feature missing | Add `features = ["macros", ...]` in `Cargo.toml`. |
| Duplicate-version dependency warnings from `cargo deny` | Two crates pull different majors | `cargo tree -d` to find the offender; pin a single version. |
| `cargo nextest` finds 0 tests | Test crate not listed in workspace `members` | Add it to root `Cargo.toml` `[workspace] members`. |
| `cargo expand` says "not installed" | Subcommand binary missing | `cargo install --locked cargo-expand`. |
| `the trait bound ... is not satisfied` after upgrade | Feature flag changed between versions | `cargo update -p <crate>` then check the changelog for breaking flag renames. |

## Docker & Compose

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `permission denied while trying to connect to the Docker daemon` | User not in `docker` group | `sudo usermod -aG docker $USER && newgrp docker`. |
| `port is already allocated` on `docker compose up` | Local Postgres / Redis already running on the same port | `sudo lsof -i :5432` to find it; either stop the host service or change the published port in `compose.yaml`. |
| Volume holds stale data | Named volume not pruned | `docker compose down -v` (destroys volumes; data loss intentional in dev). |
| Container exits immediately | Healthcheck failing or env missing | `docker compose logs <service> --tail=100`. |

## PostgreSQL & sqlx

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `sqlx::query!` complains "set DATABASE_URL or run `cargo sqlx prepare`" | Offline cache stale or env var missing | `export DATABASE_URL=postgres://app:app@localhost:5432/app && cargo sqlx prepare --workspace`. Commit the resulting `.sqlx/` directory. |
| `connection refused` from app to Postgres | App is using `localhost` but compose service is `db` (or vice versa) | Inside compose: use service name `db`. From host: use `localhost` + the published port. |
| `relation "users" does not exist` | Migrations not run | `cargo sqlx migrate run` (or `make migrate`). |
| `password authentication failed for user` | Env mismatch between compose and `.env` | Diff `.env` and `compose.yaml`; canonicalize via `.env.example`. |
| Slow query in prod | Missing index | `EXPLAIN ANALYZE <query>`; add index in a migration. |

## Auth (argon2, JWT, sessions)

| Symptom | Likely cause | First-line fix |
|---|---|---|
| Argon2 verify panics or rejects valid passwords | Mixed `argon2` crate versions in the dep graph | `cargo tree -d` to find dupes; pin one version in workspace deps. |
| JWT verify fails locally but works in tests | Clock skew on host | `sudo timedatectl set-ntp true`. Add leeway: `validation.leeway = 60`. |
| Session cookie not sent by browser | `Secure` set but on `http://` localhost | In dev, gate `secure(true)` behind `cfg(not(debug_assertions))` or an env var. |
| Refresh token works after we said it shouldn't | Refresh rotation not implemented or stale token reused | Verify the *previous refresh JTI* is added to the revocation list at issue time. |

## Stripe

| Symptom | Likely cause | First-line fix |
|---|---|---|
| Stripe webhook signature invalid | Body parsed *before* signature check | Use Axum's raw `Bytes` body, verify signature, *then* `serde_json::from_slice`. |
| Duplicate side effects from a single Stripe event | Event handler not idempotent | Persist `stripe_event_id` in a UNIQUE column inside the same txn as the side effect; ignore on conflict. |
| Subscription state lagging | `customer.subscription.updated` arrived out of order | Use the event's `created` timestamp; only apply if newer than stored. |
| Money totals off by 1 cent | Float used somewhere | `rg -n '\\bf(32\|64)\\b' apps/`; convert any money paths to `i64` cents + `rust_decimal` for proration only. |
| Test mode webhooks not arriving | `stripe listen` not running | `stripe listen --forward-to localhost:3000/webhooks/stripe`. |

## SvelteKit

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `500: not implemented` on form action | Wrong adapter for the deploy target | Use `@sveltejs/adapter-node` for our Docker deploy. |
| `$env/dynamic/private` import works on server but errors at build | Imported from a client component | Move it to `+page.server.ts` / `+layout.server.ts` or a `src/lib/server/` file. |
| Hydration mismatch warnings | SSR rendered different content than client | Check for `Date.now()` / `Math.random()` / locale-dependent code without a stable seed. |
| `vite-plugin-svelte` "unknown rune" | Older `svelte` version | `pnpm up svelte@latest @sveltejs/kit@latest`. |

## MCP servers

| Symptom | Likely cause | First-line fix |
|---|---|---|
| MCP server "command not found" | `~/.cargo/bin` not on PATH for the Claude process | Use the absolute path in `.claude/settings.json`, e.g. `/home/you/.cargo/bin/rust-analyzer-mcp`. |
| `rust-analyzer-mcp` reports "no project" | Run from outside the workspace root | The MCP must launch with cwd at the workspace root. |
| `rust-docs-mcp` returns nothing | Crate name typo | Use the exact crate name from `Cargo.toml`, not its module path. |

## Git & GitHub

| Symptom | Likely cause | First-line fix |
|---|---|---|
| `gh: command not found` | GitHub CLI not installed (see Toolchain row above) | Install + `gh auth login`. |
| `gh pr create` 403 | Branch protection requires a green CI check | Push, wait for CI, then re-run. |
| `git push` rejected as non-fast-forward | Branch diverged | `git fetch origin && git rebase origin/<branch>` (or `git pull --rebase`). |
| Commit blocked by pre-commit hook | Format / lint failed | Read the hook output, fix, `git add`, recommit. **Never `--no-verify`.** |

## CI

| Symptom | Likely cause | First-line fix |
|---|---|---|
| CI passes locally, fails in GHA | Different toolchain version | Pin `rust-toolchain.toml` and `actions/setup-node`. |
| `make verify` flaky on testcontainers | Docker socket perms in CI runner | Use `services:` keyword for Postgres/Redis instead of testcontainers in CI; reserve testcontainers for local. |
| Tests timeout in CI | Default 60s too short | `cargo nextest run --test-threads <n> --slow-timeout 90`. |
