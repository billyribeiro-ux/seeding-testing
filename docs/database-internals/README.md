# Database Internals

This directory is the "I can't keep treating Postgres like a black box anymore"
deep-dive. It is intentionally Postgres-flavored — the curriculum uses Postgres
end-to-end — but every concept here has an analogue in MySQL, SQL Server, and
to a lesser extent SQLite.

## Why an application engineer needs to know any of this

Most of the time you can write Rust against `sqlx::query!` and never think about
how the bytes get to the disk. You picked the right column to index, you wrote
the right `WHERE` clause, the test passes, the production p95 looks fine, you
move on. That is the correct working mode.

The problem is that the moments where you *need* to understand the internals
are exactly the moments where pretending it's a black box will cost you a day
or a week:

- **Two transactions appear to have conflicting views of the same row.** You
  cannot debug this without a working model of MVCC and isolation levels.
- **`pg_stat_activity` is full of `idle in transaction` and the database is
  unresponsive.** You cannot fix this without understanding what a transaction
  is doing while it's "idle."
- **VACUUM took the database down at 3am.** You cannot prevent this from
  happening again without understanding tuple visibility and bloat.
- **The query planner picked a sequential scan over your "perfect" index.**
  You cannot reason about this without knowing how a B-tree is shaped and what
  the planner is actually estimating.
- **Replication lag is 4 hours and your read replicas are returning stale
  data.** You cannot triage this without understanding the WAL.
- **`txid_current()` is 200 million and climbing.** You have weeks before
  wraparound takes the database offline. This is a real failure mode that has
  killed real companies.

The threshold isn't "I will become a database expert." It is "I can read the
docs that the experts wrote and not be lost." This directory aims to get you
to that threshold and no further.

## Files in this directory

- [`01-mvcc-and-isolation-levels.md`](./01-mvcc-and-isolation-levels.md) —
  MVCC (Multi-Version Concurrency Control) is the single most important
  concept to internalize about Postgres. Every weird transaction behavior
  traces back to it. This file also covers the four SQL isolation levels,
  what each one allows vs. prevents, and walks through a concrete
  `subscriptions`-table example from the MemberClub schema under each level.

- [`02-btree-wal-and-vacuum.md`](./02-btree-wal-and-vacuum.md) —
  Three concepts that share a chapter because they are mutually entangled:
  - **B-trees** are the on-disk shape of almost every index you'll create.
  - **WAL** (Write-Ahead Log) is how Postgres guarantees durability and how
    it powers replication and point-in-time recovery.
  - **VACUUM** is the housekeeping process that exists because of MVCC and
    that prevents transaction-ID wraparound.

## Reading order

Read `01` first. The vocabulary it establishes (tuple, xmin, xmax, visibility)
is used throughout `02`. If you have only 30 minutes, read `01` to the end of
the "Postgres defaults" section.

## What this directory is NOT

- Not a SQL tutorial. Assumes you can read joins and CTEs.
- Not a query-optimizer tour. The planner is a 500-page book unto itself.
- Not advice on which RDBMS to pick. You're using Postgres for this curriculum.
- Not a guide to running Postgres in production at scale. See the runbooks
  ([`../runbooks/db-connection-storms.md`](../runbooks/db-connection-storms.md))
  for operational concerns.

## How this ties into the rest of the curriculum

| Concept | Where it shows up |
|---|---|
| `FOR UPDATE SKIP LOCKED` | [`projects/08-outbox-demo`](../../projects/08-outbox-demo/) — outbox worker claims rows |
| Keyset pagination on an indexed column | [`projects/03-notes-api`](../../projects/03-notes-api/) — `GET /notes?cursor=...` |
| Row-Level Security as MVCC-aware predicate | [`projects/12-multi-tenant-rls`](../../projects/12-multi-tenant-rls/) |
| Connection pool exhaustion (often a long transaction problem) | [`docs/runbooks/db-connection-storms.md`](../runbooks/db-connection-storms.md) |
| Audit-log append-only design | [`docs/compliance/01-gdpr.md`](../compliance/01-gdpr.md) — crypto-shredding |

## Further reading (off-repo, optional)

- *PostgreSQL 14 Internals* by Egor Rogov — the free PDF is the definitive
  modern reference; chapters 1–5 cover everything in this directory and more.
- *Designing Data-Intensive Applications* by Martin Kleppmann — chapters 7
  ("Transactions") and 9 ("Consistency and Consensus") are the cross-database
  generalizations of what's here.
- The Postgres documentation chapters on
  [Concurrency Control](https://www.postgresql.org/docs/current/mvcc.html)
  and [Routine Database Maintenance](https://www.postgresql.org/docs/current/maintenance.html)
  are surprisingly readable once the vocabulary clicks.
