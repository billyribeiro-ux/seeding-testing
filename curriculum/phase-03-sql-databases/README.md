# Phase 3 — SQL and Databases (Drizzle/SQLite → Postgres/sqlx)

> **Audience:** you finished Phase 2.
> **Outcome:** you can model data in SQL, run migrations, query from code, and *feel* the trade-offs between an ORM (Drizzle) and a hand-rolled SQL toolkit (sqlx).
> **Time:** 2–3 weeks.

## The mental model

> *The database is the source of truth. The application is a cache of it.*

Three corollaries that decide most database arguments:

1. **Mirror, don't synchronize.** When your app integrates with Stripe (Phase 8), your DB *mirrors* Stripe events — you never read Stripe synchronously in a user-facing path.
2. **Constraints belong in the DB.** Anything you can express as `NOT NULL`, `UNIQUE`, `CHECK`, or a foreign key is one fewer bug class you have to police in code.
3. **Schema changes are forever.** A migration that runs on production is irreversible-ish (rollbacks are expensive). Treat each one as a public API change.

## Two steps, two databases

We do this phase in two passes — deliberately. *Why?* Because the lowest-friction database you can show a non-coder is **SQLite**, and the lowest-friction ORM in 2026 is **Drizzle**. So we start there: row on screen in under an hour. Then, once persistence feels natural, we re-implement the *same* domain in the production-grade pair we'll use for the rest of the curriculum: **PostgreSQL** + **sqlx** + hand-written SQL.

| | Step A — friendly on-ramp | Step B — production stack |
|---|---|---|
| Database | SQLite (file, no server) | PostgreSQL 17 (Docker container) |
| Toolkit | Drizzle ORM (TypeScript) | sqlx (Rust, compile-time-checked SQL) |
| App | SvelteKit `projects/02b-sqlite-notes-svelte` | Rust `projects/02c-sqlx-notes` |
| What you write | `db.select().from(notes)` | `sqlx::query_as::<_, Note>("SELECT … FROM notes WHERE …")` (the runtime form, so the project builds without a live DB; the capstone uses the compile-time `query_as!` macro against Postgres) |
| What ships in MemberClub | — | **This** is the production stack |

The side-by-side teaches you to read both flavours fluently. Most senior engineers can.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-relational-fundamentals.md` | Tables, rows, columns, keys, types, NULL |
| `lessons/02-schema-design.md` | Normal forms (1NF–3NF), when to denormalize, naming conventions |
| `lessons/03-sql-by-doing.md` | SELECT / INSERT / UPDATE / DELETE / JOIN / aggregations |
| `lessons/04-indexes-and-explain.md` | B-tree, GIN, BRIN, `EXPLAIN ANALYZE` |
| `lessons/05-transactions-and-isolation.md` | ACID, isolation levels, advisory locks |
| `lessons/06-stepA-drizzle-sqlite-sveltekit.md` | Build `projects/02b-sqlite-notes-svelte` step by step |
| `lessons/07-stepB-postgres-in-docker.md` | Compose, healthchecks, psql, `\d`, EXPLAIN |
| `lessons/08-stepB-sqlx-fundamentals.md` | Pool, `query!` vs `query_as!`, offline mode, migrations |
| `lessons/09-stepB-build-sqlx-notes.md` | Build `projects/02c-sqlx-notes` step by step |
| `lessons/10-drizzle-vs-sqlx.md` | Side-by-side honest comparison |

## Capstone drills

- **`projects/02b-sqlite-notes-svelte/`** — A tiny SvelteKit app: a single page that lists, adds, and deletes notes. Drizzle schema, `drizzle-kit push`, server-only DB access via `$lib/server`.
- **`projects/02c-sqlx-notes/`** — A Rust library + integration tests. Hand-written SQL, sqlx migrations, runtime `query_as::<_, Note>` (so it builds with no live DB), typed errors. The compile-time `query_as!` macro arrives with Postgres in Phase 4.

Both projects ship clippy/lint-clean with green tests.

## Green-bar checkpoint

```bash
# Step A
cd projects/02b-sqlite-notes-svelte
pnpm install
pnpm db:push           # creates ./dev.sqlite and applies the schema
pnpm dev               # http://localhost:5173 — add and delete notes
pnpm check && pnpm build

# Step B
cd ../02c-sqlx-notes
cargo test             # in-memory SQLite via sqlx; no Docker required
```

…and `make verify` is green at the workspace root.

## What's next

Phase 4 — **Axum CRUD**. We graduate to Postgres + sqlx with a real Axum service, layered middleware, OpenAPI docs, and problem-details errors.
