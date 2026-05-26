# Lesson 11.3 — Database Performance

> **Concept first:** most "the API is slow" stories trace back to the DB.
> `EXPLAIN ANALYZE` is your X-ray. Indexes, N+1, and pool tuning fix 90%
> of cases.
> **Time:** 25 minutes.

## `EXPLAIN ANALYZE` recap

We met this in Phase 3.4. The shortest mantra:

```sql
EXPLAIN (ANALYZE, BUFFERS) SELECT ...;
```

Read bottom-up. Look for:
- **`Seq Scan` on a big table** with a `Filter` removing many rows →
  missing index.
- **`Index Scan`** with `Rows Removed by Index Recheck` → wrong index
  shape; consider partial / composite.
- **`Nested Loop`** over millions on the outer side → use a hash join.
- **`Sort`** on a huge intermediate → add a covering index.

## The most common bug: N+1

```rust
// Wrong
let posts = sqlx::query_as!(Post, "SELECT * FROM posts WHERE author_id = $1", uid).fetch_all(&pool).await?;
for p in &posts {
    let author = sqlx::query_as!(Author, "SELECT * FROM users WHERE id = $1", p.author_id).fetch_one(&pool).await?;
    // ...
}
// N+1 queries. For 1000 posts, 1001 round trips.
```

The fix is a single query with a join:

```sql
SELECT p.*, u.email AS author_email, u.created_at AS author_joined_at
FROM posts p
JOIN users u ON u.id = p.author_id
WHERE p.author_id = $1;
```

Or, if you must keep separate types, batch-load the related rows:

```rust
let posts = sqlx::query_as!(Post, "SELECT * FROM posts WHERE author_id = $1", uid).fetch_all(&pool).await?;
let author_ids: Vec<i64> = posts.iter().map(|p| p.author_id).collect();
let authors = sqlx::query_as!(Author, "SELECT * FROM users WHERE id = ANY($1)", &author_ids).fetch_all(&pool).await?;
let by_id: HashMap<i64, &Author> = authors.iter().map(|a| (a.id, a)).collect();
// ...zip posts with by_id...
```

Two queries, regardless of `N`. Use `=ANY($1)` with arrays for the lookup.

## Index design — the 80% rule

Three rules cover most needs:

1. **Index every foreign key column you join on.** Postgres doesn't do
   this automatically; do it in the migration.
2. **For a multi-condition `WHERE` + `ORDER BY`**, build a composite
   `(filter_col, sort_col DESC)`. The order matters.
3. **For "alive rows only" queries**, build a *partial* index:
   `CREATE INDEX ... WHERE deleted_at IS NULL`. Smaller index, faster
   reads, faster writes.

## Pool tuning

```rust
PgPoolOptions::new()
    .max_connections(20)              // typical app pool size
    .min_connections(2)
    .acquire_timeout(Duration::from_secs(5))
    .idle_timeout(Duration::from_secs(10 * 60))
    .max_lifetime(Duration::from_secs(30 * 60))
    .connect(&url).await?
```

Three rules:

- **`max_connections` × pods ≤ Postgres's `max_connections`** (default
  100). Otherwise the *N*+1th pod can't connect.
- **`acquire_timeout`** so requests fail fast instead of hanging. 1–5s
  is normal.
- **`max_lifetime`** so long-lived idle connections get recycled —
  prevents stale TLS sessions, helps PG load balancers rebalance.

Add a `db_pool_in_use` gauge metric (from Phase 10.4) and an alert at 80%.

## Statement timeouts

```sql
ALTER ROLE app SET statement_timeout = '5s';
```

Any query that exceeds 5 seconds dies on the DB side. Frees a connection,
fails fast.

For an even more aggressive guard, set per-query timeouts in sqlx:

```rust
sqlx::query!("SELECT pg_sleep($1::text)::interval", interval)
    .execute(&pool).await
```

(sqlx doesn't have native per-query timeouts; wrap in `tokio::time::timeout`.)

## Vacuum and bloat

Long-running DBs accumulate bloat — dead tuples that haven't been
reclaimed. The Postgres `autovacuum` handles most of this, but you should
know:

- **`VACUUM (VERBOSE, ANALYZE) my_table;`** — manual cleanup.
- **`SELECT * FROM pg_stat_user_tables WHERE n_dead_tup > 10000;`** — find
  bloated tables.
- **`REINDEX CONCURRENTLY my_index;`** — rebuild an index without taking
  a write lock.

Run a quarterly check. Set up the
`pg_stat_user_tables` query as a Grafana panel and alert on extreme bloat.

## Prepared statements + sqlx

sqlx's `query!` macro auto-prepares. Repeated execution of the same query
benefits from PG's prepared-statement cache. You don't have to think about
it — but know that *changing the query string each call* defeats the
cache.

## Read replicas

For read-heavy workloads, route SELECTs to a read replica. Two patterns:

- **Pool per kind** — `PgPool` for primary, `PgPool` for replica.
  Handlers pick based on the request type.
- **Smart router** — middleware that picks based on path or HTTP method.

Don't reach for this until your primary's CPU is consistently > 60%. Until
then, it's just complexity.

## Why this matters

- **Most application slowness lives in the DB.** Indexes + N+1 fixes
  are the highest-leverage optimizations in any backend.
- **Pool tuning is a real engineering decision.** Too low = thundering
  herd on acquire; too high = exhausting PG. Measure both.
- **Statement timeouts + acquire timeouts** turn slow into failure fast.
  Customers prefer 500s to 30-second hangs.

## Green-bar checkpoint

- You can rewrite an N+1 loop as a single query (or two queries with
  `= ANY(...)`).
- You can pick a composite index given a `WHERE` + `ORDER BY`.
- You can configure `PgPoolOptions` with sensible numbers.

Next: `lessons/04-caching-with-redis.md`.
