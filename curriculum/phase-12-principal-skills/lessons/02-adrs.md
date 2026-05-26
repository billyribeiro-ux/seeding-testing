# Lesson 12.2 — Architecture Decision Records

> **Concept first:** an ADR is a single immutable document capturing one
> architectural decision: *what we chose, why, what we considered, and
> what we lose.*
> **Time:** 20 minutes.

## The MADR template

```md
# ADR 0042: Use sqlx instead of an ORM

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team
- Tags: data, postgres

## Context and Problem Statement

Our data layer needs to expose typed query results with compile-time
correctness, and we expect to need Postgres-specific features (RLS,
LISTEN/NOTIFY, window functions) over the next year.

## Decision Drivers

- Compile-time correctness of queries against the live schema
- Direct access to Postgres features
- Team SQL fluency goal (every engineer writes hand-rolled SQL)
- Operational debugging (EXPLAIN ANALYZE the actual query)

## Considered Options

1. SeaORM — full ORM with active record
2. Diesel — query DSL, compile-time, mature
3. sqlx — async, compile-time-checked SQL, no DSL
4. Hand-rolled `tokio-postgres` with no library help

## Decision Outcome

Chose **sqlx**.

- Hand-written SQL satisfies the "team SQL fluency" goal directly.
- Compile-time checks via `query!`/`query_as!` macros catch schema drift.
- Async-native; integrates with Tokio without extra glue.
- Offline mode (`cargo sqlx prepare`) keeps CI fast and deterministic.

## Consequences

- Positive: queries are the literal SQL the planner sees; debugging
  with `EXPLAIN ANALYZE` is direct.
- Positive: schema changes that break queries fail at compile time.
- Negative: more boilerplate than an ORM for trivial CRUD; mitigated by
  pure-function repository helpers.
- Negative: migrations are hand-written SQL files; we accept that as a
  feature (auditable, reviewable).

## Notes

We re-evaluate every 18 months. If sqlx becomes unmaintained or
async-graphql / etc require ORM integration we'll revisit.
```

Seven sections; ~one page. Read it in 90 seconds, decide in 5 minutes.

## When to write one

Write an ADR when:

- The decision is **non-trivial** to reverse (≥ a sprint of work to
  switch).
- A **future engineer** will ask "why did we do it this way?"
- The decision **affects more than one team** or service.
- You've **considered alternatives**. ADRs that don't consider
  alternatives are wishful blog posts.

Don't write one for:

- Style preferences ("use tabs vs spaces" — that's `.editorconfig`).
- One-off bug fixes.
- Decisions you're going to change next week.

## The "Status" field

Lifecycle:

```
Proposed   → under review
Accepted   → committed to
Deprecated → superseded; the new ADR explains why
Superseded → by ADR XYZ
```

ADRs are *immutable* once accepted. To revise, write a new ADR that
supersedes the old. The audit trail is the value.

## Storage

`docs/01-architecture-decisions/0042-sqlx-not-orm.md`.

Numbered sequentially. Filename slug describes the topic. Markdown.
Reviewed via PR.

## Who reads them

- **New engineers.** Onboarding: read ADRs 0001 through latest. They'll
  ask "why?" less, ship sooner.
- **Engineers proposing a change.** "I want to switch ORMs" — start by
  re-reading 0042 to know what you're arguing against.
- **Future you.** In 18 months, when you've forgotten the reasoning, the
  ADR is your memory.

## Anti-patterns

- **ADRs as marketing.** Don't write one to *announce* a choice you've
  already made; write it during deliberation when alternatives are real.
- **ADRs without consequences.** The "Consequences" section is the
  costly-to-write, valuable-to-read part. Don't skip it.
- **ADRs without alternatives.** "We chose X" with no Y or Z considered
  is a statement, not a decision.

## What to do *right now*

The curriculum has implicit ADRs scattered through it. Promote them:

```
0001  Rust + Axum for the API
0002  ORM stance: sqlx for prod, Drizzle/SQLite for the on-ramp
0003  Money: i64 cents + $21B ceiling, never floats
0004  Dual-mode auth: cookies for web, JWT for API
0005  Explicit policies, not Casbin
0006  Outbox pattern over external broker
0007  Postgres RLS as belt-and-braces
0008  Stripe is the rail, not the source of truth
```

E12.1 has you write three of these from scratch.

## Why this matters

- **Decisions are forever.** Without ADRs you re-have the same debate
  every six months. With them, you debate it once.
- **ADRs are *cheap* documentation.** One page per decision; pays for
  itself in the first onboarding cycle.
- **They train your judgment.** Writing one forces you to articulate
  the trade-off, which is half of being a Principal.

## Green-bar checkpoint

- You can quote the seven MADR sections.
- You can name three of MemberClub's implicit ADRs.
- You can articulate when *not* to write an ADR.

Next: `lessons/03-rfcs-and-design-docs.md`.
