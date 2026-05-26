# Runbook: 5xx spike on notes-api

## Detection

Alerts that fire on this:

- **`ErrorBudgetFastBurn`** — 5xx rate > 1% for 2 min. *Paging.*
- **`Latency_p99_high`** — p99 > 1 s for 5 min. *Paging.*
- Customer reports in `#customer-support`.

## Immediate triage (60 seconds)

1. **Open the Grafana dashboard:** https://grafana.memberclub.test/d/notes-api
2. **Look at the deploy annotation list.** Was anything shipped in the
   last 30 min? If yes, this is likely a deploy regression — go straight
   to *Rollback*.
3. **Check `db_pool_in_use`.** If > 90%, the DB is saturated; this is a
   query problem.
4. **Check `requests_total` by `route`.** Is one route doing > 10× its
   normal volume? Could be a traffic spike or a misbehaving client.
5. **Tail the logs** for ERROR-level events:
   ```bash
   flyctl logs --app notes-api | rg '"level":"ERROR"' | head -50
   ```

## Mitigation options

### A — Rollback the deploy (preferred if a recent deploy)

```bash
flyctl releases list --app notes-api | head -5
flyctl releases rollback <previous-id> --app notes-api
# wait ~90s; the 5xx rate should drop
```

If the rollback brings error rate < 0.1%, you've mitigated. Continue
to *Investigation*.

### B — DB pool saturated

The pool is full → requests time out on acquire → 503. Two paths:

```bash
# 1. Quick: increase pod count to add pool headroom
flyctl scale count 4 --app notes-api

# 2. If a single query is eating all the connections, find it:
docker compose exec db psql -U app -d app -c \
   "SELECT pid, now()-query_start AS duration, state, query
    FROM pg_stat_activity WHERE state != 'idle' ORDER BY duration DESC LIMIT 10;"
# Identify the offender, terminate it:
docker compose exec db psql -U app -d app -c "SELECT pg_terminate_backend(<pid>);"
```

### C — Single user/IP abuse

```bash
flyctl logs --app notes-api | rg '"req_id"' | jq -r '.req_id, .ip' | sort | uniq -c | sort -rn | head -20
```

If one IP / user dominates, add to the rate-limit deny list:

```bash
# Redis
redis-cli SET ratelimit:deny:<ip> 1 EX 3600
```

### D — Third-party (Stripe / mail) outage

If error logs show `stripe::Error` or SMTP timeouts, the third party is
the cause. Switch the affected feature to *degraded mode*:

```bash
flyctl secrets set STRIPE_MODE=mirror_only --app notes-api
# (the app reads from our DB mirror instead of touching Stripe)
```

User-facing reads keep working; writes get queued in the outbox.

## Communication

Once you've started mitigation:

1. Post in `#incidents` with the **start time** and **mitigation in
   progress**.
2. If customer-impacting > 5 min, post to `status.memberclub.test`
   with a 1-sentence statement.
3. Tag DRI and on-call engineering manager.

## Escalation

- **15 min without mitigation** → page the next on-call.
- **30 min without mitigation** → notify @cto in `#incidents`.
- **60 min** → CEO notification.

## Investigation (after mitigation)

Once the error rate is back to baseline:

1. **Pull traces** for the 5xx requests from Tempo. Look at the trace
   tree — where's the time?
2. **EXPLAIN ANALYZE** any suspicious queries with realistic data.
3. **Diff the deploy** — what code changed?
4. **Check correlated metrics** — DB CPU, network bytes, queue depth.

## Postmortem

If the incident lasted > 5 minutes or affected > 100 users, a postmortem
is required. Schedule it within 48 hours. Use the template in
`docs/03-postmortems/0000-TEMPLATE.md`. Follow the blameless rules
(Lesson 12.5).

## Recent history

- 2026-04-12: caused by a missing index on `notes.user_id` (PR #1234,
  ADR 0019).
- 2026-03-02: caused by Stripe webhook flood (PR #1180).

(Update this list after every incident.)
