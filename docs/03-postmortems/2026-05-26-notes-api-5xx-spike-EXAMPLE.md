# Postmortem: notes-api 5xx spike — 2026-05-26 (Worked Example)

> **This postmortem is a worked example, not a real incident.** Phase 12
> learners read it to see what a blameless postmortem looks like in
> practice. To author your own, copy `0000-TEMPLATE.md`.

## Summary

At 14:32 UTC on 2026-05-26, `notes-api` began returning HTTP 500 on the
new `/v1/notes/search` endpoint. Mitigated by rolling back to v1.2.3 at
14:46 (14 minutes of degraded service). No data loss. Estimated
~3000 affected users (~12% of MAU during the window).

## Impact

- **Duration:** 14 minutes (14:32 – 14:46 UTC).
- **User-facing impact:** 8% peak error rate on `/v1/notes/search`,
  rendered as a generic "Something went wrong" toast in the web app.
- **Scope:** ~3000 users; users not on `/v1/notes/search` were
  unaffected.
- **Data loss:** None.
- **Revenue impact:** Negligible. No billing or payment endpoints
  affected.

## Timeline (UTC)

```
14:24  PR #2089 ("add notes search") merged.
14:28  Deploy v1.2.4 to production (auto via CD).
14:32  ErrorBudgetFastBurn alert fires; @alice paged.
14:34  @alice acknowledges; opens #incident-2026-05-26 in Slack.
14:35  Grafana shows 5xx spike + db_pool_in_use at 99%.
14:36  @alice declares herself DRI; loops in @bob (on-call backup).
14:38  Hypothesis: the new /v1/notes/search query is exhausting the pool.
14:40  @bob confirms via traces: search query taking 4–7 s under load.
14:42  Decision: rollback rather than try to ship a forward fix.
14:44  flyctl releases rollback to v1.2.3 (90 s).
14:46  Error rate normalizes; #incidents updated.
14:55  Status page updated: "investigating" → "resolved".
15:00  @alice opens follow-up issues; schedules this postmortem.
15:30  Customer support clears the backlog.
```

## Root cause

PR #2089 added `GET /v1/notes/search?q=...` with the query:

```sql
SELECT id, body, created_at FROM notes
WHERE body ILIKE '%' || $1 || '%'
ORDER BY created_at DESC LIMIT 20;
```

This query has no usable index for an `ILIKE '%...%'` predicate on a
~5M-row table. Under production load the planner chose a sequential
scan; each query took 4–7 seconds. With concurrent search traffic, the
DB connection pool (50 connections) was fully consumed in 8 seconds,
and subsequent requests for any endpoint failed at acquire time → 500
in problem-details form.

## Five whys

1. **Why did we 5xx?** The DB pool was exhausted; new requests failed
   to acquire a connection within the 5 s timeout.
2. **Why was the pool exhausted?** A single new query was holding
   connections for 4–7 s each.
3. **Why was the query so slow?** It performed a sequential scan of the
   notes table on every call.
4. **Why a sequential scan?** No index supports `ILIKE '%X%'` — leading
   wildcard prevents B-tree use.
5. **Why did this ship?** Three system gaps:
   - PR review focused on correctness, not perf characteristics.
   - The pre-deploy load test only exercises a fixed set of routes; new
     endpoints aren't auto-included.
   - The pool-saturation alert threshold was 90% — too high to give
     enough warning during a fast-burn incident.

## What worked

- **Detection.** The `ErrorBudgetFastBurn` alert fired within 4 minutes
  of customer impact.
- **Mitigation path.** `flyctl releases rollback` is rehearsed; 90
  seconds end-to-end.
- **DRI structure.** Single owner for decisions; communication clear.
- **The webhook + outbox architecture** (ADR 0006) kept all billing
  traffic flowing during the incident because the receiver was
  unaffected.

## What didn't

- **PR review missed the missing index.** Reviewer (@dave) approved
  with focus on the test coverage; no perf review.
- **Pre-deploy load test was incomplete.** The test plan was authored
  in 2026-02 and hasn't auto-updated when new routes are added.
- **Pool-saturation alert was misconfigured.** Threshold at 90%
  triggered after the pool was effectively dead.
- **Search index was never planned.** The PR shipped the endpoint
  without an associated migration.

## Follow-ups

| Action | Owner | Due | Status |
|---|---|---|---|
| Add `pg_trgm` GIN index on `notes.body` for the search query | @bob | 2026-05-30 | open |
| Add "perf review" required checklist item for any new endpoint PR | @carol | 2026-05-29 | open |
| Pre-deploy load test auto-discovers new routes from the OpenAPI diff | @dave | 2026-06-10 | open |
| Lower pool-saturation alert threshold 90% → 80% | @alice | 2026-05-27 | done |
| Add `Search responses by p99 < 200ms` to perf budget table | @alice | 2026-05-27 | done |
| Update the runbook `docs/runbooks/5xx-spike.md` to add "Recent history" entry | @alice | 2026-05-27 | done |

## Customer communication

- 14:55 Status page: "We are investigating issues with search on the
  notes app. The rest of the app is unaffected."
- 15:00 Status page: "Mitigated. We are investigating root cause."
- 17:00 Email to support customers who opened tickets: brief explanation +
  apology + assurance.
- 2026-05-29 Public engineering blog post: technical writeup (this
  postmortem, redacted of internal Slack handles).

## Owner

- **DRI (incident):** @alice
- **Postmortem author:** @alice
- **Reviewer:** @cto on 2026-05-27

## Related

- Incident channel: `#incident-2026-05-26-notes-search` (archived)
- Pull requests: #2089 (root cause), #2091–2094 (follow-ups)
- Runbook used: `docs/runbooks/5xx-spike.md`
- ADR: 0001 (Rust + Axum — unchanged), 0006 (outbox — unchanged)
- Perf budget: `docs/perf/budgets.md`
