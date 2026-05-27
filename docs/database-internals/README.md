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

## Vocabulary cheat-sheet

These terms recur across both files. Read this once now; refer back when
the words don't click.

- **Tuple**: one physical row-version on disk. A logical row has one or
  more tuples (more, transiently, during MVCC updates).
- **xmin / xmax**: the transaction IDs that created and (if any)
  superseded a tuple. The MVCC visibility check reads them.
- **Snapshot**: the set of transaction IDs visible to a given transaction,
  taken at transaction or statement start.
- **Dead tuple**: a tuple that no still-running snapshot needs.
- **Heap**: the table's data file. "Heap fetch" = "go read the actual row."
- **Index**: a separate data structure (almost always a B-tree) that
  maps key → row-location.
- **Page**: 8KB unit of disk allocation. Postgres reads and writes
  entire pages.
- **WAL**: the Write-Ahead Log. Sequential append-only stream of every
  page-level change made to the database.
- **Checkpoint**: the periodic event that flushes dirty pages to their
  data files, freeing earlier WAL segments to be recycled or archived.
- **VACUUM**: the housekeeping process that reclaims dead-tuple space
  and updates the visibility map.
- **Visibility map**: per-table bitmap indicating which pages are
  all-visible. Powers index-only scans.
- **Freeze**: VACUUM's act of rewriting old `xmin` values to a sentinel
  to defend against transaction-ID wraparound.
- **Wraparound**: the failure mode where transaction IDs overflow the
  32-bit space and the visibility check breaks. Operationally fatal
  if you let it happen.
- **Index-only scan**: a query plan that reads only the index, no heap
  fetch. Requires the visibility map to vouch for the leaf-pointed
  heap page.
- **MVCC**: Multi-Version Concurrency Control. The umbrella term for
  "readers don't block writers and vice versa."

## When to escalate

The contents of this directory are sufficient for routine reasoning
about Postgres. Some things are not in scope and require either a
specialist or much more reading:

- Query planner internals: cost estimation, plan stability, hint-style
  workarounds. The planner is a separate, deep topic.
- Replication topology design: hot standbys, cascading replication,
  logical replication, conflict resolution.
- Sharding: when one Postgres instance is no longer enough. Citus,
  CockroachDB, Yugabyte. None of these are drop-in.
- The internals of PG extensions you depend on (`pgvector`, `postgis`,
  `pg_partman`). Each has its own quirks.

If you're in any of these areas, treat this directory as background
context and reach for the relevant primary source.

## Common questions this directory will not answer

- "Should I use Postgres or MySQL?" Use whichever your team knows.
  The differences relevant to a small/medium SaaS are smaller than the
  on-call expertise you build up running one of them.
- "Should I switch to a NewSQL distributed database?" Almost certainly
  not yet. The complexity tax is enormous. Cross that bridge when a
  single Postgres can't fit your workload, which is later than you
  think.
- "What's the right shared_buffers setting?" Start with `pg_tune` or
  cloud-provider defaults. Tune only after you have measurements
  proving the default is wrong.
- "Should I use JSONB everywhere?" No. JSONB is a tool for genuinely
  unstructured data. Schema-less storage of structured data is a
  recipe for inconsistencies you'll spend years cleaning up.

## How the files in this directory relate to each other

```
README.md (you are here)
  │
  ├── 01-mvcc-and-isolation-levels.md
  │     ├── establishes vocabulary: tuple, xmin/xmax, snapshot,
  │     │   visibility check
  │     ├── covers four isolation levels with worked subscriptions
  │     │   example
  │     └── ends with FOR UPDATE SKIP LOCKED cross-ref to outbox demo
  │
  └── 02-btree-wal-and-vacuum.md
        ├── builds on the vocabulary from 01
        ├── B-tree shape and the keyset-pagination cross-ref to
        │   projects/03-notes-api
        ├── WAL: durability, replication, PITR (cross-ref to DR runbook)
        ├── VACUUM: cleanup mechanism, autovacuum, ANALYZE
        └── XID wraparound: the operational time bomb
```

If you have an hour: read both files end-to-end.
If you have 20 minutes: read 01 through the "Postgres defaults" section
and skim the rest.
If you have 5 minutes: read this README plus the worked example in 01.

## A note on Postgres versions

The behaviors described here apply to PostgreSQL 13 through 17.
Behavior changes worth flagging:

- PG 13 added parallel VACUUM (index cleanup phase).
- PG 14 introduced more aggressive freezing (less wraparound risk
  for healthy clusters).
- PG 15 added the `MERGE` command (relevant for upsert patterns).
- PG 16 added logical replication from standbys.
- PG 17 (current LTS-ish) improved VACUUM efficiency further.

MemberClub runs PG 16 in production. The fundamentals haven't moved
in a decade; the operational quality-of-life keeps getting better.
