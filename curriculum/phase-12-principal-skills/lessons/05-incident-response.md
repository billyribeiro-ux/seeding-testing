# Lesson 12.5 — Incident Response and Blameless Postmortems

> **Concept first:** incidents happen. Runbooks make them survivable.
> Postmortems make them rarer over time.
> **Time:** 20 minutes.

## The five phases of an incident

```
1. Detect       — alerts fire, customer report, internal noise
2. Triage       — DRI established, severity set, communication started
3. Mitigate     — restore service (rollback, scale, kill the bad request)
4. Investigate  — root-cause analysis after the dust settles
5. Postmortem   — write up, share, file follow-ups
```

Phase 3 happens *first*. You don't debug in production while customers
are down. Mitigate, then investigate.

## The DRI

**Directly Responsible Individual** — one person owns the incident.
Their job during the incident:

- Make the call on mitigation steps.
- Communicate with stakeholders.
- Delegate technical work.

The DRI usually *doesn't* type. They direct.

## The runbook

`docs/runbooks/<symptom>.md`. One per known failure mode:

```md
# Runbook: 5xx spike on notes-api

## Detection
Alert "ErrorBudgetFastBurn" fires when 5xx rate > 1% for 2 min.

## Immediate triage (60 seconds)
- Open Grafana dashboard: https://grafana/d/notes-api
- Check the deploy annotation list — was anything shipped in the last 30 min?
- Check `db_pool_in_use` — is the DB saturated?

## Mitigation options
1. If deploy annotation matches → flyctl releases rollback
2. If DB saturated → scale up max_connections, restart pods
3. If a single user/IP → identify in logs; add to ratelimit deny list

## Investigation
- Pull traces with status_class=5xx from the last hour
- Common root causes:
  - DB connection storm (Phase 11.3)
  - 3rd-party API outage (Stripe, mail provider)
  - Recently shipped query without an index

## Escalation
- 15 min without mitigation → page the next on-call
- 30 min without mitigation → notify @cto in #incidents

## Recent history
- 2026-04-12: caused by a missing index on `notes.user_id` (PR #1234, ADR 0019)
- 2026-03-02: caused by Stripe webhook flood (PR #1180)
```

A runbook is most useful at 2 AM when the on-call engineer is still
half-asleep. Write it for them.

## The blameless postmortem

After the incident, write up:

```md
# Postmortem: notes-api 5xx spike on 2026-05-26

## Summary
At 14:32 UTC notes-api began returning 5xx for 14 minutes. Mitigated
by rolling back to v1.2.3 at 14:46. No data was lost.

## Impact
- 14 minutes of 5xx on /v1/notes (8% error rate at peak).
- ~3000 affected users (~12% of MAU during that window).
- No data loss.

## Timeline (UTC)
- 14:24  PR #2089 merged ("add notes search").
- 14:28  Deploy v1.2.4 to production (auto via CD).
- 14:32  ErrorBudgetFastBurn alert fires.
- 14:34  On-call (@alice) acknowledged.
- 14:36  DRI established; @alice runs `flyctl logs`.
- 14:42  Root cause hypothesis: the new full-text query lacks an index.
- 14:44  Decision: rollback rather than ship a fix.
- 14:46  v1.2.3 restored; error rate falls.

## Root cause
The new `GET /v1/notes/search?q=...` endpoint added in PR #2089 issued
a `LIKE '%...%'` query on a 100k-row table without an index. Under
load the query escalated to a sequential scan exhausting the connection
pool.

## What worked
- The alert fired within 4 minutes.
- The rollback path was tested and took 90 seconds.
- The DRI structure kept communication clear.

## What didn't
- The PR review missed the missing index.
- The pre-deploy load test didn't include the new endpoint.
- The pool-saturation alert was misconfigured (threshold too high).

## Five Whys
1. Why did we 5xx? Because the DB pool was exhausted.
2. Why was the pool exhausted? A new query was taking 4s under load.
3. Why was it taking 4s? It scanned the entire `notes` table.
4. Why did it scan? It used `LIKE '%...%'` without a full-text index.
5. Why did the missing index ship? PR review didn't include perf analysis;
   the perf budget didn't cover new endpoints; the pre-deploy load test
   was incomplete.

## Follow-ups
- [ ] @bob: add a `pg_trgm` GIN index on notes.body for the search query.
- [ ] @carol: add new-endpoint review checklist item for perf budget.
- [ ] @dave: pre-deploy load test now exercises every new route in the diff.
- [ ] @alice: lower the pool-saturation alert threshold from 90% to 80%.

## Owner
@alice (DRI). Reviewed by @cto on 2026-05-27.
```

Note what's *missing*: blame. The five-whys end on systemic issues, not
"@bob wrote a bad query." Bob may have written it; the *system* let it
ship.

## The "blameless" rule

Every incident is the result of a system. People make mistakes; systems
either catch them or don't. The postmortem's job is to find *system*
fixes:

- ❌ "@bob should have caught this."
- ✓ "The pre-deploy load test didn't cover the new endpoint. Fix:
     auto-generate load tests from the OpenAPI diff."

If a person made a mistake, the system failed to catch it. Fix the
system.

## Anti-patterns

- **Multi-page timelines.** Trim to the load-bearing events.
- **No follow-ups.** A postmortem without action items is therapy.
- **Re-litigating in the postmortem.** Disagreements about what *should
  have* happened are noise. Stay on "what *did* happen" and "what
  changes."
- **Postmortems that never get written.** "Too busy with the next
  incident" — that's how the same incident recurs.

## Repeat-incident analysis

Quarterly: count incidents per root-cause category. Look for trends.

```
2026 Q1 incidents:
  Database (5)   ← prioritize: index design review, pool tuning
  Stripe (3)
  Deploy (2)
  Other (4)
```

5 DB-related incidents in a quarter is a *category* problem, not five
unrelated bugs. Fix the category.

## Why this matters

- **Customers don't remember the outage; they remember how you handled
  it.** A clear postmortem rebuilds trust.
- **Blame breaks teams.** A psychologically safe team writes truthful
  postmortems; a fearful one writes lies.
- **Trends matter more than individual incidents.** Five DB problems
  ≠ five separate bugs.

## Green-bar checkpoint

- You can name the five phases of an incident.
- You can structure a postmortem with the required sections.
- You can identify three "blame" phrases and rephrase them as system
  fixes.

Next: `lessons/06-mentoring-and-pairing.md`.
