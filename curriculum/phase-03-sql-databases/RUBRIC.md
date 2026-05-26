# Phase 3 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Relational model** | Confused by joins | Comfortable with INNER/LEFT joins, GROUP BY | Reasons in terms of relations and constraints; spots denormalization opportunities |
| **Schema design** | Lots of nullable columns, no constraints | NOT NULL, FKs, CHECK constraints by default | Designs schemas that make invalid states unrepresentable; documents `NULL`able columns |
| **Indexes** | Adds indexes by guess | Reads `EXPLAIN ANALYZE`, adds composite + partial indexes | Profiles queries before+after; understands when an index hurts (write-heavy paths) |
| **Transactions** | Forgets BEGIN/COMMIT | Wraps multi-statement writes correctly | Picks isolation levels deliberately; uses `FOR UPDATE` / advisory locks where needed |
| **Migration discipline** | Edits past migrations | Adds new migration files, never edits applied ones | Designs forward-compatible migrations; ships zero-downtime schema changes |
| **Drizzle / ORM** | Hand-writes SQL inside a builder | Uses the builder for 90% of queries, drops to raw SQL when needed | Picks ORM vs raw SQL based on long-term operational needs |
| **sqlx** | `unwrap()` on every query | Uses `query_as!`/`query_as`, `?` for error propagation | Uses transactions correctly; understands offline mode + `.sqlx/`; uses `EXPLAIN ANALYZE` |
| **Testing data code** | No tests, or mocked DB | Integration tests against in-memory SQLite | Integration tests against the real production DB engine (Postgres via testcontainers); proptest for tricky logic |
| **Operational mindset** | "Works on my machine" | Compose for dev parity; pgAdmin and `psql` are friends | Has read the Postgres docs for the features they use; reads release notes when bumping versions |

## Self-check before moving to Phase 4

- [ ] `make verify` passes locally.
- [ ] `cd projects/02b-sqlite-notes-svelte && pnpm install && pnpm test && pnpm check && pnpm build` all green.
- [ ] You completed exercises E3.1 – E3.7.
- [ ] You can sketch the `notes` schema in both Drizzle and SQL from memory.
- [ ] You can read an `EXPLAIN ANALYZE` and name a fix.
- [ ] You can articulate when to choose Drizzle vs sqlx for a given project.
- [ ] CI is green on your branch.

Phase 4 — Axum CRUD — uses Postgres + sqlx for real, behind an HTTP server.
