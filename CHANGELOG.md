# Changelog

All notable changes to the curriculum and its projects.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

Items being worked on but not yet shipped:

- (none currently)

## [0.1.0] — 2026-05-26

The curriculum-complete release. Thirteen phases, ten projects, 148 tests green.

### Added

#### Curriculum

- **Phase 0 — Foundations.** Mental model + toolchain install + git/gh,
  for the absolute beginner.
- **Phase 1 — Rust Core** (11 lessons). Cargo through traits, generics,
  errors, modules, testing. Capstone: `projects/01-hello-cli`.
- **Phase 2 — Async + Tokio** (7 lessons). Futures, `.await`, `tokio::spawn`,
  channels, cancellation, shared state. Capstone:
  `projects/02-quote-generator`.
- **Phase 3 — SQL + Databases** (10 lessons). Two-step: Drizzle/SQLite
  on-ramp via SvelteKit (`projects/02b-sqlite-notes-svelte`), then
  sqlx/SQLite production graduate (`projects/02c-sqlx-notes`).
- **Phase 4 — Axum CRUD** (9 lessons). Mental model through OpenAPI and
  keyset pagination. Capstone: `projects/03-notes-api` with
  RFC 7807 problem-details errors.
- **Phase 5 — Testing + Seeding** (8 lessons). Pyramid, nextest,
  testcontainers, snapshot + property tests, factories, seed CLI,
  coverage as a leading indicator. Adds: `notes-seed` binary + 8 tests.
- **Phase 6 — Auth** (9 lessons). argon2id, signed HttpOnly session
  cookies, RS256 JWT, 2FA TOTP, rate-limiting, the dual-mode
  `AuthenticatedUser` extractor. Capstone: `projects/04-auth-demo`
  (18 tests).
- **Phase 7 — RBAC + ABAC** (9 lessons). Pure-function policies, the
  `require!` macro, ownership / tenancy / time-bounded patterns,
  admin bootstrap, append-only audit log. Capstone:
  `projects/05-rbac-policy-lab` (31 tests).
- **Phase 8 — Stripe + Money** (12 lessons). The Money primitive
  (`i64` cents, $21B ceiling), largest-remainder split, Stripe data
  model, Checkout, subscriptions, webhook reliability, dunning, tax,
  refunds, reconciliation. Two capstones:
  `projects/06-stripe-money-lab` (25 tests) +
  `projects/07-webhook-receiver` (8 tests).
- **Phase 9 — Svelte 5 + SvelteKit 2** (8 lessons). Runes, routing +
  layouts, form actions, server-only modules, hooks + session auth.
  Capstone: `apps/memberclub/web` (5 vitest tests).
- **Phase 10 — Observability** (8 lessons). Three signals, `tracing`,
  OpenTelemetry, Prometheus metrics, structured logging, dashboards
  as code, alerts and SLOs.
- **Phase 11 — Performance, Caching, Background Jobs** (8 lessons).
  Profiling, DB perf, Redis caching, the outbox pattern, multi-tenancy
  + RLS, load testing, perf budgets.
- **Phase 12 — Principal Engineer Skills** (8 lessons). Mental model
  for the role, ADRs, RFCs, code review at L7, incident response,
  mentoring, system design, capstone deliverables.

#### docs/

- **8 ADRs** capturing the curriculum's architectural decisions:
  0001 Rust+Axum, 0002 sqlx-vs-Drizzle stance, 0003 Money i64 cents,
  0004 dual-mode auth, 0005 explicit policies, 0006 outbox over broker,
  0007 Postgres RLS, 0008 Stripe is the rail.
- **5 mental-model essays**: money, async, db-as-source-of-truth, auth,
  rbac-vs-abac.
- **5 runbooks**: 5xx spike, Stripe webhook lag, db connection storms,
  rollback, deploy failure.
- **3 templates**: ADR, RFC, postmortem.
- **2 worked examples**: an RFC for usage-based billing, a postmortem
  for a simulated notes-api 5xx incident.
- **Performance docs**: budgets table, methodology recipe, sample
  before/after report.

#### Repository infrastructure

- **CI/CD**: `ci.yml` (fmt + clippy + nextest with Postgres/Redis
  services + cargo-deny + cargo-audit + matrix SvelteKit job),
  `cd.yml` (GHCR multi-arch build + sqlx migrate + Fly.io deploy +
  post-deploy smoke), `release.yml` (semver tag + Conventional-Commit
  notes).
- **Makefile** with `verify`, `up/down/logs/migrate/seed`, `web-*`,
  `demo`, `bootstrap`, `ci` targets.
- **`compose.yaml`** with Postgres 17 + Redis 7.4 + MailHog.
- **`.claude/settings.json`** registering `rust-analyzer-mcp` and
  `rust-docs-mcp`, plus an allowlist for the curriculum's expected
  commands.
- **`scripts/bootstrap.sh`** — idempotent one-shot toolchain install.
- **`scripts/demo.sh`** — end-to-end smoke of every Rust service;
  verified working.
- **`TROUBLESHOOTING.md`** — 90+ symptom→fix entries across
  toolchain, Rust/cargo, Docker, Postgres/sqlx, auth, Stripe,
  SvelteKit, MCPs, git/GitHub, CI, and 18 patterns specifically
  captured while authoring the curriculum.
- **`CONTRIBUTING.md`** — how to land a PR; ADR/RFC discipline;
  lesson + exercise + project templates.

### Stack pinned (May 26, 2026)

- Rust 1.95.0 / edition 2024 / resolver 3
- Axum 0.8.9 + tower 0.5 + tower-http 0.6
- Tokio 1.51 LTS
- sqlx 0.8.6 (Postgres + SQLite features)
- argon2 0.5
- jsonwebtoken 9
- async-stripe 0.41 (with an upgrade-lab to 1.0-rc documented)
- Svelte 5.55 / SvelteKit 2.20 / adapter-node 5 / vite 6 / vitest 3
- Drizzle ORM 0.36 + better-sqlite3 11
- PostgreSQL 17 + Redis 7.4 + MailHog
- Node 22 LTS + pnpm 10
- cargo-nextest, insta, proptest, fake, rand, wiremock, testcontainers (mentioned)

### Tests

- 136 Rust tests across 8 crates, all via `cargo nextest`.
- 12 vitest tests across the two SvelteKit projects.
- `make verify` aggregates fmt + clippy + nextest.
- `make demo` end-to-end-runs every Rust service and asserts the
  expected HTTP responses.
