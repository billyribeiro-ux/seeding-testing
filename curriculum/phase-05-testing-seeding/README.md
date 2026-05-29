# Phase 5 — Testing and Seeding

> **Audience:** you finished Phase 4. You have a working Axum service.
> **Outcome:** you write tests that catch regressions, fixtures that mirror production data, and a seed CLI that's safe to run anywhere.
> **Time:** 1–2 weeks.

## The mental model

> *Tests are how yesterday's work stays alive. Seeds are how your dev environment stays indistinguishable from prod.*

Two ideas that compound:

1. **A failing test before you fix a bug is the bug's permanent vaccine.** If you don't write the test first, the bug *will* come back.
2. **A seed script is a runbook in code.** Anyone — new hire, on-call engineer, auditor — should be able to clone the repo, run `make seed`, and have a system that behaves like production.

## The five test flavors

| Flavor | What it proves | Tooling |
|---|---|---|
| **Unit** | A pure function does what it claims | `#[test]` + `cargo nextest` |
| **Integration** | A boundary (HTTP, DB) behaves correctly end-to-end | `tower::ServiceExt::oneshot`, testcontainers |
| **Snapshot** | Output (JSON, SQL plan, HTML) hasn't drifted | `insta` |
| **Property** | An invariant holds for *all* inputs in a class | `proptest` |
| **Smoke** | The production-like deploy isn't on fire | curl + Playwright + post-deploy hooks |

Each one catches a different bug class. You'll see all five before this phase ends.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-test-pyramid.md` | The pyramid, the trophy, what to put where |
| `lessons/02-nextest.md` | Faster runs, retries, partitioning, JUnit XML for CI |
| `lessons/03-integration-with-testcontainers.md` | Real Postgres per test run, no Docker on the host? no problem |
| `lessons/04-snapshot-and-property.md` | `insta` for golden files, `proptest` for invariants |
| `lessons/05-factories-and-fixtures.md` | `fake` crate, factory functions, builders for tests |
| `lessons/06-seeding-strategies.md` | seed_dev / seed_demo / seed_load, idempotency, profiles |
| `lessons/07-coverage-as-leading-indicator.md` | `cargo llvm-cov`, threshold gates, what coverage *doesn't* tell you |
| `lessons/08-build-seed-cli.md` | Capstone: a `notes-seed` binary you can run safely anywhere |

## The capstone

We extend `projects/03-notes-api` with:

1. **Property tests** for any pure logic (validation rules).
2. **Snapshot tests** for the problem-details response shapes (so any wire format change is intentional).
3. **A factory module** in `sqlx-notes/src/factory.rs` that builds plausible `Note` records.
4. **A `notes-seed` binary** in `notes-api` that takes `--profile dev|demo|load` and idempotently seeds the database.
5. **A coverage gate** in CI: `cargo llvm-cov` reports per-crate coverage and fails CI if any project drops below 80%.

## Green-bar checkpoint

```bash
# All quality gates
make verify

# Coverage
cargo install --locked cargo-llvm-cov
cargo llvm-cov --workspace --lcov --output-path lcov.info
cargo llvm-cov report                    # human-readable
cargo llvm-cov --fail-under-lines 80     # CI gate

# Seed CLI (point DATABASE_URL at a file so the rows persist — the
# binary defaults to an in-memory DB that vanishes on exit)
DATABASE_URL=sqlite://./dev.sqlite cargo run -p notes-api --bin notes-seed -- --profile dev
sqlite3 ./dev.sqlite "SELECT COUNT(*) FROM notes;"     # ~20
```

…and CI is green on your branch.

## What's next

Phase 6 — **Auth**. We add password hashing (`argon2`), signed session cookies, JWT access + refresh tokens, 2FA (TOTP), and the dual-mode pattern (cookies for SvelteKit web, JWT for API clients).
