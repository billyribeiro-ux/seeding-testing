# seeding-testing — Zero to Principal Engineer L7+

A code-along curriculum that takes a learner **from never coded before** to **Principal Engineer L7+** on a modern enterprise stack:

- **Backend:** Rust 1.95 stable, Axum 0.8, Tokio 1.51 LTS, sqlx 0.8 against PostgreSQL 17, argon2 for password hashing, async-stripe for payments.
- **Frontend:** Svelte 5 / SvelteKit 2 (runes-first), with a Drizzle + SQLite warm-up before the production Postgres stack.
- **Auth:** Signed HttpOnly session cookies for the web app + short-lived JWT (access + refresh) for API clients — the dual-mode enterprise pattern.
- **Authorization:** RBAC + ABAC, audit-logged.
- **Payments:** Stripe (one-time + recurring) with `i64` cents and a $21B money ceiling. No floats anywhere near money.
- **Ops:** Docker (dev = test = CI parity), GitHub Actions, observability with `tracing` + OpenTelemetry.

This repo *is* the textbook. Read top-down, code along, run `make verify` at the end of every phase.

## Curriculum map

| # | Phase | Folder | What you'll build |
|---|---|---|---|
| 0 | Foundations | `curriculum/phase-00-foundations/` | First commit pushed, toolchain installed |
| 1 | Rust Core | `curriculum/phase-01-rust-core/` | `projects/01-hello-cli` — a tested CLI |
| 2 | Async + Tokio | `curriculum/phase-02-async-tokio/` | `projects/02-quote-generator` — concurrent HTTP |
| 3 | SQL + Databases | `curriculum/phase-03-sql-databases/` | Step A: SQLite + Drizzle in SvelteKit. Step B: Postgres + sqlx |
| 4 | Axum CRUD | `curriculum/phase-04-axum-crud/` | `projects/03-notes-api` |
| 5 | Testing + Seeding | `curriculum/phase-05-testing-seeding/` | 90%+ coverage + a seed CLI |
| 6 | Auth | `curriculum/phase-06-auth-sessions-jwt/` | `projects/04-auth-demo` — sessions + JWT |
| 7 | RBAC + ABAC | `curriculum/phase-07-rbac-abac/` | `projects/05-rbac-policy-lab` |
| 8 | Stripe + Money | `curriculum/phase-08-stripe-money/` | `projects/06-stripe-money-lab` + `07-webhook-receiver` |
| 9 | Svelte 5 + SvelteKit 2 | `curriculum/phase-09-svelte-sveltekit/` | MemberClub web app live |
| 10 | Observability | `curriculum/phase-10-observability/` | Traces, metrics, logs |
| 11 | Performance | `curriculum/phase-11-perf-caching-jobs/` | Redis, jobs, multi-tenancy |
| 12 | Principal Engineer | `curriculum/phase-12-principal-skills/` | ADRs, RFCs, postmortems |

The **capstone** lives at `apps/memberclub/` and is built progressively across phases.

## How to use this repo

1. **Read in order.** Each `phase-NN-*/README.md` opens with the mental model, then lessons. Don't skip ahead — phases compound.
2. **Code along.** Every lesson ends with a "green-bar checkpoint" you must reproduce on your machine.
3. **Verify often.** `make verify` runs the same checks CI does (fmt, clippy, tests, sqlx prepare).
4. **Commit frequently.** Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`). Push to your branch and watch `gh run watch`.
5. **When stuck:** open `TROUBLESHOOTING.md` first, then `docs/runbooks/`, then ask.

## Quick start

```bash
# 1. Get the toolchain (one-time; see Phase 0 for the full setup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
rustup component add clippy rustfmt rust-src

# 2. Verify the workspace builds
make verify

# 3. Run the Phase 1 CLI you just built
cargo run -p hello-cli -- --help
```

## Reading order

1. `PLAYBOOK.md` — mental models a Principal Engineer keeps in their head.
2. `curriculum/phase-00-foundations/README.md` — the absolute beginner on-ramp.
3. `TROUBLESHOOTING.md` — bookmark it. You will need it.
4. Phases 1 → 12 in order.

## License

Dual-licensed under MIT or Apache-2.0.
