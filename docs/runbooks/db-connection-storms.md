# Runbook: Postgres connection storms

## Detection

Alerts that fire on this:

- **`PoolSaturationHigh`** — `db_pool_in_use / db_pool_size > 0.80` for 5 min. *Warning.*
- **`PgConnectionsHigh`** — Postgres-side `pg_stat_database.numbackends > 80%` of `max_connections`. *Paging.*
- **`ErrorBudgetFastBurn`** — secondary effect; pool exhaustion → 503s.

## What "connection storm" means

Either:
- **(A) Application-side:** the pool itself is overwhelmed (held connections, too few workers,
  too low `max_connections` per pod, traffic spike).
- **(B) Server-side:** all available Postgres connections are taken across pods. New pods cannot
  connect.

## Immediate triage (60 seconds)

```sql
-- How many connections, by application_name and state?
SELECT application_name, state, COUNT(*)
FROM pg_stat_activity
WHERE pid != pg_backend_pid()
GROUP BY application_name, state
ORDER BY 3 DESC;

-- Long-running queries (likely held connections)?
SELECT pid, now() - query_start AS duration, state, left(query, 80) AS q
FROM pg_stat_activity
WHERE state != 'idle' AND now() - query_start > interval '1 second'
ORDER BY duration DESC LIMIT 20;
```

Two patterns to look for:

1. **One slow query held by many connections** → kill it; fix the index.
2. **Many short-lived connections piling up** → traffic spike or a noisy client.

## Mitigation

### A — A specific slow query is dominating

```sql
-- Terminate the worst offenders (won't crash the DB; will fail the in-flight requests).
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE now() - query_start > interval '30 seconds' AND state != 'idle';
```

Then locate the source query and missing index. See Phase 11 lesson 3.

### B — Pool too small for the load

Increase per-pod `max_connections` *if* the DB has headroom:

```bash
flyctl secrets set DATABASE_POOL_MAX=20 --app notes-api  # was 10
flyctl restart --app notes-api
```

Math: `max_per_pod × pod_count ≤ pg_max_connections − 10` (leave room for admin and other services).

### C — Server-side `max_connections` is the cap

The instinct is to bump `max_connections` on the Postgres server. **Resist.** Each connection
allocates ~10 MB on the server; doubling connections doesn't double capacity. Instead:

1. **Add a connection pooler** (PgBouncer in transaction mode). Apps connect to PgBouncer; PgBouncer
   maintains a small pool to Postgres. We get 10× the apparent connections with the same memory.
2. **Look for connection leaks.** A connection held > 30 s should be a bug ticket.
3. **Tune pool lifetimes:** `max_lifetime = 30 min` recycles connections so a leak self-heals.

### D — Application-side leak

If pool saturation persists after traffic returns to baseline:

- A handler is holding a connection across an `.await` that returned but never released.
- `tx.commit()` was forgotten on a hot path.
- A long-running query is being polled by many requests instead of cached.

Use `tokio-console` to find the stuck task. Patch with a `tokio::time::timeout` around the
acquire to surface the leaker fast.

## Investigation

After mitigation:

1. **Pull traces** from the time window. Filter for `db_pool_acquire` spans > 1 s.
2. **`EXPLAIN ANALYZE` the offender** with realistic data.
3. **Check pool config history** — was a recent deploy changing `max_connections`?
4. **Sigma / pg_stat_statements** for the top queries by total time during the window.

## Communication

- Internal `#incidents` post immediately on alert.
- If user-facing > 5 min, status-page update with "investigating connection saturation."
- No customer-facing communication needed unless the outage is > 15 min and clearly
  user-impacting.

## Recovery

After the alert clears:

- Verify `db_pool_in_use` < 50% sustained for 30 min.
- Verify no long-running queries (> 5 s).
- Verify no leaks: connection count should drop when traffic drops.

## Postmortem trigger

- > 5 min of pool exhaustion with user-visible impact → postmortem.
- < 5 min self-resolving → log in `#incident-log` channel; review monthly for trends.

## Related

- Phase 11 lesson 3 (DB perf)
- ADR 0006 (outbox — reduces concurrent writes that would otherwise compete for connections)
- `docs/runbooks/5xx-spike.md` — pool exhaustion is a common cause
