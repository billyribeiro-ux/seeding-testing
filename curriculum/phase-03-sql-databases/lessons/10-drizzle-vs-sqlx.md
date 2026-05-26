# Lesson 3.10 — Drizzle vs sqlx: An Honest Comparison

> **The principal-engineer move:** know multiple tools. Pick the right one per situation, and articulate *why*.
> **Time:** 30 minutes.

You just shipped the *same* feature in two stacks. Now we name the trade-offs out loud.

## The headline

| Dimension | Drizzle (TypeScript) | sqlx (Rust) |
|---|---|---|
| Time to first query | ~5 minutes | ~30 minutes |
| Reading complex SQL | Through a builder; eventually leaky | Directly: it *is* SQL |
| Compile-time checks | TypeScript types from the schema | Real DB-checked queries (`.sqlx/` cache for offline) |
| Migration tooling | `drizzle-kit push` (diffs schema) | `sqlx-cli` (versioned SQL files) |
| Ecosystem | Modern TS, growing fast | Mature Rust, deeply integrated with Tokio + Axum |
| Long-term ceiling | Hits the builder's expressiveness wall | Whatever the DB supports |
| Connection / runtime cost | Node process, GC | One Rust binary, no GC |

Two perfectly valid choices. The question is what you're optimizing for.

## When to choose Drizzle

- **Frontend-led products.** SvelteKit / Next.js apps where the same TypeScript team owns the DB.
- **Admin dashboards, internal tools, MVPs.** Frictionless, low-stakes-but-typed.
- **Small surface area.** Up to a dozen tables, the builder API stays comprehensible.

## When to choose sqlx (or another raw-SQL toolkit)

- **Performance-sensitive APIs.** Stripe webhook receivers, anything sub-millisecond.
- **Complex SQL** — window functions, CTEs, advisory locks, FOR UPDATE SKIP LOCKED, `LISTEN`/`NOTIFY`, native Postgres types (`tsvector`, `range`, `inet`, `cidr`).
- **Operational maturity.** When `EXPLAIN ANALYZE` is part of your weekly habit, you want to write the SQL the planner sees.
- **You already speak SQL.** Half your team's brain RAM goes free if there's no ORM in between.

## The principal-engineer split

Real companies often **use both, on the same product**:

- **Drizzle for the SvelteKit admin app** that internal staff use. It owns its own tiny database.
- **sqlx for the public API** that customer traffic hits. Different stack, different concerns.

The two databases coordinate via events on a shared message bus, not by sharing tables. That's a deliberate architectural boundary.

## Specific things sqlx does that Drizzle doesn't (cleanly)

- **`SELECT ... FOR UPDATE SKIP LOCKED`** — the cleanest queue-worker pattern in Postgres.
- **Window functions** — `ROW_NUMBER() OVER (PARTITION BY ...)`, lead/lag, cumulative sums.
- **`LISTEN` / `NOTIFY`** — Postgres pub/sub from inside a transaction.
- **Native types** — `inet`, `cidr`, `tsvector`, `range`, `geometry` (with PostGIS).
- **Custom functions and triggers** — when business logic *has* to live next to the data.
- **Partitioned tables** — the standard for time-series data at any scale.

Drizzle can talk to all of these via `sql\`...\`` template tags, but at that point you're writing raw SQL anyway and bypassing the ORM you're paying for.

## Specific things Drizzle does that sqlx doesn't (cleanly)

- **Schema-as-code** — your TypeScript schema *is* the source of truth; `drizzle-kit` diffs it against the DB to generate migrations.
- **Type inference end-to-end** — `db.select().from(...).where(...).leftJoin(...)` types itself with zero annotation.
- **Editor go-to-definition across schema and queries.**
- **Lower onboarding cost** for full-stack TypeScript engineers.

## What the curriculum chose, and why

The capstone (`apps/memberclub/`) uses **sqlx + Postgres**. Reasons:

1. We want **every learner** to be fluent in SQL, not in one ORM's API. SQL is the durable skill.
2. The capstone exercises features (RLS, `FOR UPDATE`, `LISTEN/NOTIFY`, `ON CONFLICT`) that need raw SQL to teach cleanly.
3. The Stripe + money work demands `BIGINT` + `CHECK` constraints + idempotency tables. We control all of those at the schema level.
4. Rust + sqlx is the *production* stack we're teaching for, and we want learners to see it end-to-end.

We use Drizzle once — in Phase 3 Step A — as a *deliberate first taste*. It's a real tool worth knowing; it's also the lowest-friction way to see a row show up on screen.

## A Principal-Engineer-grade move

Once you internalize this lesson, you'll catch yourself doing something senior in code reviews:

> When you see *any* tool, ask: "What does this make easy? What does this make hard? What's the failure mode when scale or complexity outgrows it?"

That question — applied to ORMs, to runtimes, to languages, to architectures — is half of being a Principal Engineer.

## Green-bar checkpoint

- You can articulate two situations where Drizzle is the right call, and two where sqlx is.
- You can name three Postgres features that are awkward in any ORM but trivial in sqlx.
- You can describe a real-world setup that uses *both* in one product.

Phase 3 is complete. Phase 4 — **Axum CRUD** — is where the Rust + sqlx stack gets its first real HTTP service.
