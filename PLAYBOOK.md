# Playbook — Mental Models a Principal Engineer Carries

> "A senior engineer writes code that works. A staff engineer writes code that works *and* explains why. A principal engineer changes the system so the wrong code can't be written." — internal motto for this repo.

This Playbook is the *condensed* reference. The expanded essays live in `docs/00-mental-models/`. The decisions that shaped this repo live as Architecture Decision Records (ADRs) in `docs/01-architecture-decisions/`. The on-call recipes live in `docs/runbooks/`.

---

## 1. The Compiler is Your Pair Programmer

Rust's borrow checker isn't a tax — it's a colleague who refuses to merge unsafe code. Treat compile errors as feedback, not obstacles. The cost of fighting the compiler now is far less than the cost of a memory bug at 3 AM.

## 2. The Database is the Source of Truth

Your application is a cache of the database. If the cache and the database disagree, the database wins. Two corollaries:

- **Mirror, don't synchronize.** When we integrate with Stripe, our DB *mirrors* Stripe events, but we never read Stripe in a user-facing request path.
- **Constraints belong in the DB.** Anything you can express as `NOT NULL`, `UNIQUE`, `CHECK`, or a foreign key is one more bug class you've made impossible.

## 3. Async is Cooperative Multitasking

A `Future` is a paused recipe. `.await` is "I'm waiting on something — chef, take someone else." Two rules to internalize:

- **Don't `.await` while holding a lock.** Either drop the lock or use `tokio::sync` (async-aware) locks.
- **Cancellation is real.** Every `.await` is a potential cancellation point. Code that holds a half-applied write across an `.await` is a bug waiting to happen.

## 4. Money is Not a Float

Ever. The curriculum codifies this in a single `Money(i64 cents, Currency)` newtype. The constant `MONEY_CEILING_CENTS = 2_100_000_000_00` ($21B) bounds every monetary value. See `docs/00-mental-models/money.md` and ADR `0003-money-i64-cents.md`.

## 5. Authentication ≠ Authorization

- **AuthN** answers *who are you?* — sessions (cookies) for browsers, JWT (bearer) for API clients.
- **AuthZ** answers *can you do this?* — RBAC for roles, ABAC for context-sensitive permissions.

Conflating them is the most common security bug we see. See `docs/00-mental-models/auth.md` and `rbac-vs-abac.md`.

## 6. Idempotency Everywhere

Networks fail. Retries happen. Every state-changing endpoint must accept an idempotency key. Every Stripe webhook must be replay-safe. The default is *at-least-once delivery* — design for it.

## 7. Tests are How Yesterday's Work Stays Alive

- **Unit tests** for pure logic.
- **Integration tests** against a real Postgres (testcontainers), never a mock.
- **Snapshot tests** for response payloads.
- **Property tests** for math (money!).
- **E2E tests** for golden paths.

Coverage isn't a number to game — it's a leading indicator of *did you think about what could go wrong*.

## 8. Observability is Non-Negotiable

If you can't see it, you can't operate it. Every request gets a trace. Every error gets structured context. Every business metric (signups, MRR, failed payments) gets a counter. Dashboards are committed to the repo.

## 9. The Runbook is Half the System

The other half is the code. When an incident happens at 2 AM, the half-asleep on-call engineer's only friend is `docs/runbooks/<symptom>.md`. Write the runbook *before* you ship the feature.

## 10. Code is the Smallest Part of Being Principal

The rest is judgment, communication, and force-multiplication:

- **ADRs** capture *why*, so the next engineer doesn't relitigate decisions.
- **RFCs** debate *what* before *how*.
- **Postmortems** are blameless and structural.
- **Mentoring** scales you beyond what your hands can type.

---

## ADR Index

ADRs live at `docs/01-architecture-decisions/`. They use the MADR template (Markdown ADR). The numbered, immutable record of every irreversible architectural choice.

| # | Title | Status |
|---|---|---|
| 0001 | Rust + Axum for the API | (planned) |
| 0002 | ORM stance: sqlx for prod, Drizzle/SQLite for the on-ramp | (planned) |
| 0003 | Money: i64 cents + $21B ceiling, never floats | (planned) |
| 0004 | Dual-mode auth: cookies for web, JWT for API | (planned) |
| 0005 | Explicit policy functions, not Casbin | (planned) |

## Runbook Index

| Incident | Runbook |
|---|---|
| Postgres connection storms | `docs/runbooks/postgres-connection-storms.md` (planned) |
| Stripe webhook lag | `docs/runbooks/stripe-webhook-lag.md` (planned) |
| 5xx spike | `docs/runbooks/5xx-spike.md` (planned) |

## Glossary

| Term | Meaning |
|---|---|
| **ADR** | Architecture Decision Record — a short doc capturing a single irreversible decision and why. |
| **ABAC** | Attribute-Based Access Control — decisions based on (subject, action, resource, context) attributes. |
| **DRI** | Directly Responsible Individual — the single person accountable for an incident or decision. |
| **MRR / ARR** | Monthly / Annual Recurring Revenue. |
| **N+1** | A query bug where you fetch 1 parent then N children, one round-trip each. Fatal at scale. |
| **PII** | Personally Identifiable Information — handle with care; log carefully; encrypt at rest. |
| **RBAC** | Role-Based Access Control — decisions based on what role a user holds. |
| **RFC** | Request for Comments — a design doc circulated before a non-trivial change. |
| **SLO** | Service Level Objective — the internal target for reliability (e.g. 99.9% uptime / month). |
