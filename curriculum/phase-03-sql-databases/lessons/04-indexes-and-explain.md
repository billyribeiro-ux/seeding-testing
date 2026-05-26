# Lesson 3.4 — Indexes and `EXPLAIN ANALYZE`

> **Concept first:** an index is a side-cache the database consults to find rows fast. The wrong index is just expensive storage. `EXPLAIN ANALYZE` is how you tell them apart.
> **Time:** 30 minutes.

## Why indexes exist

Without an index, the DB has to do a *sequential scan* — read every row to check if it matches. That's `O(n)`. For a million-row table that's a million row-touches per query, and your `WHERE email = 'alice@b.com'` will be very slow.

With an index on `email`, the DB consults a *B-tree* (binary-tree-shaped lookup structure) and arrives at the row in `O(log n)`. For a million rows, that's ~20 hops. Microseconds.

The cost: every `INSERT`/`UPDATE`/`DELETE` to the table must also update the index. Writes get slightly slower; reads get massively faster.

## The four index types you'll meet

| Type | Use case |
|---|---|
| **B-tree** (default) | Equality (`=`) and range (`<`, `>`, `BETWEEN`) on scalar columns |
| **GIN** | Full-text search, JSONB containment, arrays |
| **GiST** | Geographic / range types |
| **BRIN** | Massive tables where values are correlated with insert order (e.g. `created_at`) |

99% of indexes are B-trees. Reach for GIN when you `WHERE json_col @> '{...}'`. BRIN occasionally for time-series tables.

## When to add an index

A practical heuristic:

> **If a column appears in a `WHERE`, `ORDER BY`, or `JOIN ON` clause of a query you run more than once a minute on a non-tiny table — index it.**

Three subtleties:

1. **Composite indexes are ordered.** An index on `(user_id, created_at DESC)` helps queries that filter by `user_id` (with or without sort), but *not* queries that filter only by `created_at`. The leading column is privileged.
2. **Partial indexes are cheaper.** `CREATE INDEX … WHERE deleted_at IS NULL` indexes only the alive rows. Smaller index, faster reads, faster writes.
3. **Foreign-key columns aren't automatically indexed in Postgres.** Always create an explicit index on every FK column you'll join on.

## `EXPLAIN ANALYZE` — your X-ray

```sql
EXPLAIN ANALYZE
SELECT * FROM users WHERE email = 'alice@b.com';
```

The output is a tree of *plan nodes*. Read it bottom-up:

```
Seq Scan on users  (cost=0.00..18324.00 rows=1 width=58)
                   (actual time=87.421..1521.382 rows=1 loops=1)
  Filter: (email = 'alice@b.com'::text)
  Rows Removed by Filter: 999999
Planning Time: 0.123 ms
Execution Time: 1521.521 ms
```

Diagnosis: a sequential scan reading 1M rows to find 1. Fix:

```sql
CREATE INDEX users_email_idx ON users (email);
EXPLAIN ANALYZE SELECT * FROM users WHERE email = 'alice@b.com';

-- Now:
Index Scan using users_email_idx on users  (cost=0.42..8.44 rows=1 width=58)
                                           (actual time=0.041..0.043 rows=1 loops=1)
  Index Cond: (email = 'alice@b.com'::text)
Planning Time: 0.067 ms
Execution Time: 0.067 ms
```

From 1.5 seconds to 67 microseconds. *That's* what indexes do.

## The four cardinal sins (and their fixes)

| Symptom in `EXPLAIN ANALYZE` | Likely cause | Fix |
|---|---|---|
| `Seq Scan` on a big table | Missing index on the filter column | Add a B-tree index |
| `Index Scan` returning thousands of rows then filtering | Index is on the wrong column; non-leading column was filtered | Reorder composite index; add a partial index |
| `Nested Loop` over millions of outer rows | Wrong join strategy | Add an index on the join column; `ANALYZE` to refresh stats |
| `Hash Aggregate` with `disk: N kB` | Aggregate spilled to disk | Increase `work_mem` for that query; reduce the row count first |

## `ANALYZE` — keeping statistics fresh

The query planner picks plans based on table statistics (row counts, value distributions). After a bulk insert or large delete, run:

```sql
ANALYZE users;
```

Postgres also does this automatically via *autovacuum*, but on bulk-loaded tables you sometimes need to nudge it.

## The N+1 query problem

A classic ORM bug: load a list of users, then for each user fire a query to load their orders. 1 + N queries. For 1,000 users, 1,001 round-trips.

Fix: one query with a join:

```sql
SELECT u.id, u.email, o.id, o.amount_cents
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.created_at > NOW() - INTERVAL '30 days';
```

Or, in sqlx specifically, use `query_as!` to fetch parent + child in one trip and group in code. We'll see this in Phase 4.

> N+1 is the most common database performance bug. Every Axum service we ship gets a CI lint that fails on suspected N+1s.

## A worked example: composite-index choice

You frequently run:

```sql
SELECT id, body
FROM posts
WHERE user_id = $1
ORDER BY created_at DESC
LIMIT 20;
```

Two index choices:

```sql
CREATE INDEX posts_user_id_idx        ON posts (user_id);
CREATE INDEX posts_user_id_created_idx ON posts (user_id, created_at DESC);
```

The composite is strictly better here: the planner walks straight to the matching `user_id` block, then reads in `created_at DESC` order — no extra sort step. For high-volume queries, this is a 10x win.

## Why this matters

- **A wrong index is worse than no index.** It costs writes and provides no read benefit. Measure with `EXPLAIN ANALYZE` before and after.
- **Indexes are the #1 lever for query performance.** Tuning queries is half "rewriting SQL" and half "did you give the planner what it needs."
- **Production indexes are added concurrently** (`CREATE INDEX CONCURRENTLY`) so they don't lock writes. Default to `CONCURRENTLY` in all production migrations.

## Green-bar checkpoint

- You can read an `EXPLAIN ANALYZE` output and identify a sequential scan vs an index scan.
- You can decide between an index on `(a)` vs `(a, b)` vs `(b, a)` given a query.
- You can articulate the N+1 problem and at least one way to fix it.

Next: `lessons/05-transactions-and-isolation.md`.
