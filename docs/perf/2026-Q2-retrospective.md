# 2026 Q2 Performance Retrospective

> A worked example of the quarterly review cadence from Phase 11.8.
> Copy this template to `<year>-Q<n>-retrospective.md` at the end of
> each quarter.

## Quarter overview

- **Quarter:** Q2 2026 (April – June)
- **Authors:** @alice (lead), @bob, @carol
- **Reviewed by:** @cto on 2026-07-02

## SLO health

```
                    target    actual (90-day avg)   trend
notes-api avail.    99.9%     99.94%                ↗ stable
notes-api p99       200 ms    113 ms                ↘ improved
auth-demo avail.    99.95%    99.97%                ↗ stable
auth-demo p99       400 ms    320 ms                = unchanged
webhook recv lag    < 5 min   p99 = 12 s            ↘ massively improved
```

Net: no SLO breaches this quarter; one SLO (webhook receiver lag) is
now far ahead of target after the outbox migration.

## Wins

### W1 — List notes p99: 162 ms → 113 ms (−31%)

PR #4242 dropped the default `limit` from "no cap" to 20. Saw a 75%
throughput win for free in the bargain. Report:
`docs/perf/notes-api-2026-05-26.md`.

Action follow-ups:
- ✓ Default documented in OpenAPI and the OpenAPI snapshot test.
- ✓ Integration test asserts default of 20.

### W2 — Outbox replaced an external broker

We retired RabbitMQ as the side-effect-publishing layer in favor of
the Postgres outbox pattern (`projects/08-outbox-demo`). Net effect:

- p99 webhook receiver lag dropped from 4 minutes to 12 seconds.
- Operational complexity decreased by one moving part.
- Annual infra savings: ~$8k.

ADR `0006-outbox-over-broker.md` documents the choice.

### W3 — Cardinality audit caught a leak

A new endpoint introduced `request_id` as a Prometheus label by
accident, exploding the time-series count by 800k. Caught by
@bob during the cardinality audit (Phase 10 lesson 4). Fix shipped
in PR #4301. Adds a CI check that the workspace's
`prometheus_metrics_total_series` query stays under 10,000.

## What didn't go well

### F1 — Two unplanned rollbacks

- 2026-05-26 notes-api 5xx (see
  `docs/03-postmortems/2026-05-26-notes-api-5xx-spike-EXAMPLE.md`)
- 2026-06-11 auth-demo session-cookie-path config drift between
  staging and prod (no postmortem; root cause was a missed env-var
  rename in deploy config)

Both were sub-15-minute rollbacks via `flyctl releases rollback`.
Customer impact contained.

Root system gap (both incidents): pre-deploy load test still doesn't
auto-discover new routes from the OpenAPI diff. Carry-over to Q3.

### F2 — auth-demo argon2 hash time creeping up

login p99 was 280 ms in Q1; now 320 ms. The dominant cost is
argon2id with our chosen m_cost = 64 MB. Two paths:
- Stay the course; argon2 dominance is acceptable for this endpoint.
- Move auth to a dedicated machine with more RAM bandwidth.

Decision: stay the course. argon2 dominance is *correct* — slower
hashing is the whole point. We'll revisit if customer feedback
surfaces complaints.

## Carry-overs to Q3

- Auto-discover new routes from OpenAPI diff for the pre-deploy load
  test. **Owner:** @dave. **Target:** July 15.
- Add SLO panel for the new `usage-recorder` service (once it ships).
  **Owner:** @alice. **Target:** end of Q3.
- Investigate cross-region latency budget for the upcoming EU shard.
  **Owner:** @carol. **Target:** end of Q3.

## Q3 priorities (one per owner)

| Owner | Priority |
|---|---|
| @alice | Add SLO panel + alert for usage-recorder service |
| @bob | Implement OpenAPI-diff-driven pre-deploy load test |
| @carol | EU shard latency study |
| @dave | Migrate sessions to Redis (Phase 11.4 caching exercise) |

## Numbers we care about

Updated `docs/perf/budgets.md` *last verified* column for every row.
All routes within budget. Next re-verification scheduled for Q3 end.

## Process notes

- The retrospective took 90 minutes including writing. ROI is good.
- Future runs: add a 5-minute "anything unexpected?" round at the
  start before going into the metrics tour.

## Related

- `docs/perf/budgets.md` — current state
- `docs/perf/methodology.md` — the protocol every report follows
- Phase 11 lessons
