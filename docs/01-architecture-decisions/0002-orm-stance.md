# ADR 0002 — ORM stance: sqlx for production, Drizzle + SQLite for the on-ramp

- Status: Accepted
- Date: 2026-05-26
- Deciders: api-team, education-team
- Tags: data, postgres, education

## Context and Problem Statement

Two audiences. *Capstone services* (notes-api, auth-demo, MemberClub)
will be operated at scale; they need the lowest impedance between code
and Postgres. *Curriculum learners* need to see persistence work within
the first hour without installing Postgres.

## Decision Drivers

- Compile-time correctness of queries against the live schema.
- Direct access to Postgres-specific features (RLS, LISTEN/NOTIFY, JSONB,
  partial indexes, `FOR UPDATE SKIP LOCKED`).
- Team SQL fluency goal — every engineer reads and writes raw SQL.
- Friction-free first experience for new learners.

## Considered Options

1. **One stack everywhere (sqlx + Postgres).** Consistent; high onboarding cost.
2. **One stack everywhere (an ORM like SeaORM).** Easy on-ramp; hits walls
   at scale.
3. **Two stacks: Drizzle/SQLite for learning, sqlx/Postgres for prod.**
   Two things to learn; both useful in real careers.

## Decision Outcome

Chose **option 3**.

- Phase 3 *deliberately* presents both, side by side. A learner reads
  the *same* notes domain twice — once in Drizzle, once in sqlx — and
  forms their own judgment.
- Production stack is sqlx + Postgres. The capstone services and
  MemberClub API are sqlx.
- Phase 3 Step A uses Drizzle + better-sqlite3 inside SvelteKit. Tiny,
  no Docker, no server.

## Consequences

- **Positive:** learners ship a row-on-screen within the first hour;
  production maintains direct access to Postgres features.
- **Negative:** the curriculum carries two persistence stacks; engineers
  joining MemberClub from the Drizzle on-ramp must do the Phase 3 Step
  B graduate.
- **Mitigations:** Lesson 3.10 is explicitly a side-by-side comparison;
  ADR readers know to expect both.

## Notes

If the production team ever needs an ORM (e.g. for an admin-only side
service), reach for SeaORM first. Do not introduce a third option.
