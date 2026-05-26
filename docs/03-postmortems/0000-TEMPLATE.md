# Postmortem — Template

> Copy to `YYYY-MM-DD-slug.md` and fill in. Be blameless. Fix systems,
> not people.

## Summary

One paragraph. When did it start? When did it end? What broke? Who was
affected?

## Impact

- **Duration:** X minutes (start UTC – end UTC).
- **User-facing impact:** what users saw.
- **Scope:** number of users / orgs / requests affected.
- **Data loss:** yes / no. If yes, quantify.
- **Revenue impact:** $X estimated.

## Timeline (UTC)

Use a clean tabular timeline. Only the events that *moved* the
incident — not every chat message.

```
14:24  Deploy v1.2.4 to production (PR #2089).
14:32  ErrorBudgetFastBurn alert fires.
14:34  @alice acks on call.
14:36  DRI established (@alice). Slack incident channel opened.
14:42  Hypothesis: the new search query lacks an index.
14:44  Decision: rollback to v1.2.3.
14:46  Rollback complete; error rate normalizes.
14:55  Customer-facing status updated.
15:30  Postmortem scheduled.
```

## Root cause

The *direct* technical cause, in one paragraph.

## Five whys

Walk to the *systemic* cause. Don't stop at "the engineer wrote bad
code" — keep asking until you reach a process or systems fix.

1. **Why did we 5xx?** The DB pool was exhausted.
2. **Why was the pool exhausted?** A query ran 4 s under load.
3. **Why was the query slow?** It scanned the full notes table.
4. **Why did it scan?** No index supported the `LIKE` predicate.
5. **Why did the missing index ship?** The PR review didn't include
   perf analysis; the pre-deploy load test didn't exercise the new
   endpoint.

## What worked

- The alert fired within 4 minutes of customer impact.
- The rollback path was rehearsed and took 90 seconds.
- The DRI structure kept communication clear.

## What didn't

- PR review missed the missing index.
- Pre-deploy load test didn't include the new endpoint.
- Pool-saturation alert threshold was too high.

## Follow-ups

| Action | Owner | Due | Status |
|---|---|---|---|
| Add `pg_trgm` GIN index for notes search | @bob | 2026-05-30 | open |
| Add perf-budget item to new-endpoint review checklist | @carol | 2026-05-30 | open |
| Pre-deploy load test exercises every new route in the diff | @dave | 2026-06-06 | open |
| Lower pool-saturation alert threshold 90% → 80% | @alice | 2026-05-28 | open |

## Owner and reviewers

- **DRI (incident):** @alice
- **Author (this postmortem):** @alice
- **Reviewer:** @cto on 2026-05-27

## Related

- Incident channel: `#incident-2026-05-26-notes-5xx` (archived)
- Pull requests: #2089 (root cause), #2090–93 (follow-ups)
- Runbook used: `docs/runbooks/5xx-spike.md`
