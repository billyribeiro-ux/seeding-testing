# seeding-testing — Zero to Principal Engineer L7+

A code-along curriculum that takes a learner **from never coded before** to **Principal Engineer L7+** on a modern enterprise stack:

- **Backend:** Rust 1.95 stable, Axum 0.8, Tokio 1.51 LTS, sqlx 0.8 against PostgreSQL 17, argon2 for password hashing, async-stripe for payments.
- **Frontend:** Svelte 5 / SvelteKit 2 (runes-first), with a Drizzle + SQLite warm-up before the production Postgres stack.
- **Auth:** Signed HttpOnly session cookies for the web app + short-lived JWT (access + refresh) for API clients — the dual-mode enterprise pattern.
- **Authorization:** RBAC + ABAC, audit-logged.
- **Payments:** Stripe (one-time + recurring) with `i64` cents and a $21B money ceiling. No floats anywhere near money.
- **Ops:** Docker (dev = test = CI parity), GitHub Actions (CI + CD + release), observability with `tracing` + OpenTelemetry, structured logging, Prometheus metrics.

This repo *is* the textbook. Read top-down, code along, run `make verify` at the end of every phase.

## Status: complete

- **13 phases** authored (Phase 0 + Phases 1–12).
- **110 lessons** across the curriculum.
- **16 standalone Rust projects + 2 SvelteKit warm-ups**, plus the full-stack `apps/memberclub` capstone (Rust API + SvelteKit web) — all built and tested.
- **345 Rust test functions across 17 crates + 18 vitest across 3 SvelteKit suites.** CI runs the full suite against Postgres 17 + Redis; `make verify` (fmt + clippy `-D warnings` + tests) is the gate.
- **8 ADRs**, 6 runbooks, 6 mental-model essays, 3 templates (ADR, RFC, postmortem), plus deep-dive series under `docs/` (async-internals, database-internals, distributed-systems, security, compliance, leadership, perf, observability).

## Curriculum map

| # | Phase | Folder | Project / Deliverable |
|---|---|---|---|
| 0 | Foundations | `curriculum/phase-00-foundations/` | First commit pushed, toolchain installed |
| 1 | Rust Core | `curriculum/phase-01-rust-core/` | `projects/01-hello-cli` — 16 tests |
| 2 | Async + Tokio | `curriculum/phase-02-async-tokio/` | `projects/02-quote-generator` — 9 tests |
| 3 | SQL + Databases | `curriculum/phase-03-sql-databases/` | `projects/02b-sqlite-notes-svelte` (Drizzle) + `projects/02c-sqlx-notes` (sqlx) — 19 tests |
| 4 | Axum CRUD | `curriculum/phase-04-axum-crud/` | `projects/03-notes-api` — 9 integration tests |
| 5 | Testing + Seeding | `curriculum/phase-05-testing-seeding/` | +6 seed unit tests + 2 snapshot tests + 2 property tests on hello-cli |
| 6 | Auth | `curriculum/phase-06-auth-sessions-jwt/` | `projects/04-auth-demo` — 18 tests (dual-mode pattern) |
| 7 | RBAC + ABAC | `curriculum/phase-07-rbac-abac/` | `projects/05-rbac-policy-lab` — 31 tests |
| 8 | Stripe + Money | `curriculum/phase-08-stripe-money/` | `projects/06-stripe-money-lab` (25) + `projects/07-webhook-receiver` (8) |
| 9 | Svelte 5 + SvelteKit 2 | `curriculum/phase-09-svelte-sveltekit/` | `apps/memberclub/web` — 5 vitest |
| 10 | Observability | `curriculum/phase-10-observability/` | Instrument notes-api (exercises) |
| 11 | Performance | `curriculum/phase-11-perf-caching-jobs/` | A documented perf report (exercises) |
| 12 | Principal Engineer | `curriculum/phase-12-principal-skills/` | An ADR + an RFC + a postmortem (exercises) |

The **capstone** is the `MemberClub` web app at `apps/memberclub/web/`, plus the production-shape Rust services in `projects/` (auth-demo, notes-api, webhook-receiver, money-lab, rbac-policy-lab).

## How to use this repo

1. **Read in order.** Each `phase-NN-*/README.md` opens with the mental model, then lessons. Don't skip ahead — phases compound.
2. **Code along.** Every lesson ends with a "green-bar checkpoint" you must reproduce on your machine.
3. **Verify often.** `make verify` runs the same checks CI does (fmt, clippy, tests).
4. **Commit frequently.** Use Conventional Commits. Push to your branch and watch `gh run watch`.
5. **When stuck:** open `TROUBLESHOOTING.md`, then `docs/runbooks/`, then ask.

## Quick start

```bash
# 1. Get the toolchain (one-time)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
rustup component add clippy rustfmt rust-src

# 2. Verify the workspace (fmt + clippy -D warnings + the full test suite)
make verify

# 3. Try the Phase 9 web app
cd apps/memberclub/web
pnpm install
pnpm db:push
pnpm dev   # http://localhost:5173
```

## Reading order

1. `PLAYBOOK.md` — mental models a Principal Engineer keeps in their head.
2. `docs/README.md` — the docs/ structure (ADRs, RFCs, mental models, runbooks).
3. `curriculum/phase-00-foundations/README.md` — the absolute beginner on-ramp.
4. `TROUBLESHOOTING.md` — bookmark it.
5. Phases 1 → 12 in order.

## Repository layout

```
seeding-testing/
├── README.md                       (this file)
├── PLAYBOOK.md                     mental models + glossary
├── TROUBLESHOOTING.md              symptom → fix table
├── Makefile                        make verify, up, down, migrate, seed
├── compose.yaml                    Postgres + Redis + MailHog
├── rust-toolchain.toml             stable
├── Cargo.toml                      workspace root (18 members)
├── .github/workflows/
│   ├── ci.yml                      fmt, clippy, nextest, deny, audit, web matrix
│   ├── cd.yml                      GHCR build, sqlx migrate, Fly.io deploy
│   └── release.yml                 semver tag + Conventional-Commit notes
├── .claude/settings.json           rust-analyzer-mcp + rust-docs-mcp registered
├── docs/
│   ├── 00-mental-models/           money, async, db-as-source-of-truth
│   ├── 01-architecture-decisions/  ADRs 0001–0008 + template
│   ├── 02-rfcs/                    template
│   ├── 03-postmortems/             template
│   └── runbooks/                   5xx spike, Stripe webhook lag
├── curriculum/
│   ├── phase-00-foundations/
│   ├── phase-01-rust-core/
│   ├── phase-02-async-tokio/
│   ├── phase-03-sql-databases/
│   ├── phase-04-axum-crud/
│   ├── phase-05-testing-seeding/
│   ├── phase-06-auth-sessions-jwt/
│   ├── phase-07-rbac-abac/
│   ├── phase-08-stripe-money/
│   ├── phase-09-svelte-sveltekit/
│   ├── phase-10-observability/
│   ├── phase-11-perf-caching-jobs/
│   └── phase-12-principal-skills/
├── projects/                       (16 Rust projects + 2 SvelteKit warm-ups)
│   ├── 01-hello-cli/               clap CLI, 16 tests
│   ├── 02-quote-generator/         concurrent HTTP, 9 tests
│   ├── 02b-sqlite-notes-svelte/    SvelteKit + Drizzle + SQLite, 7 vitest
│   ├── 02c-sqlx-notes/             sqlx + SQLite (testcontainers), 21 tests
│   ├── 03-notes-api/               Axum CRUD + keyset + OpenAPI + seed CLI
│   ├── 04-auth-demo/               argon2 + cookies + JWT + TOTP + OAuth + magic-link
│   ├── 05-rbac-policy-lab/         pure policy library, 31 tests
│   ├── 06-stripe-money-lab/        Money primitive, 29 tests
│   ├── 07-webhook-receiver/        Stripe webhook receiver, 8 tests
│   ├── 08-outbox-demo/             transactional outbox (SQLite), 13 tests
│   ├── 09-svelte-counter/          Svelte 5 runes warm-up, 6 vitest
│   ├── 10-memberclub-cli/          API client CLI
│   ├── 11-redis-cache/             cache + single-flight + rate limit, 9 tests
│   ├── 12-multi-tenant-rls/        Postgres row-level security
│   ├── 13-load-test/               load generator + percentiles
│   ├── 14-sagas/                   saga orchestrator + compensation, 12 tests
│   ├── 15-capacity-planner/        capacity math, 11 tests
│   └── 16-event-sourcing/          event sourcing + CQRS, 16 tests
└── apps/
    └── memberclub/                 full-stack capstone
        ├── api/                    Axum + sqlx (Postgres) service
        ├── web/                    SvelteKit 2.20 + Svelte 5.55, 5 vitest
        └── infra/                  Dockerfiles + prod compose + Caddy
```

## Stack pinned to May 26, 2026

| Layer | Choice | Version |
|---|---|---|
| Language | Rust | 1.95.0 |
| HTTP | axum | 0.8.9 |
| Runtime | tokio | 1.51 LTS |
| DB driver | sqlx | 0.8.6 |
| DB | PostgreSQL | 17 |
| Password hash | argon2 | 0.5 |
| JWT | jsonwebtoken | 9 |
| Frontend | Svelte / SvelteKit | 5.55 / 2.20 |
| Node | mise / fnm | 22 |
| Tests | nextest + insta + proptest + wiremock | latest |

## License

Dual-licensed under MIT or Apache-2.0.
