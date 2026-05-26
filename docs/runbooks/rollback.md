# Runbook: Rolling back a bad deploy

## When to roll back

Roll back when *all four* are true:

1. A user-visible regression is happening (5xx, latency, broken UX).
2. The regression started after a recent deploy.
3. The previous release was known good.
4. A forward fix would take > 10 min, or you don't know the root cause yet.

If forward fix is < 10 min and you understand the cause → roll forward.
If you're unsure → roll back. *Always.* The DB and feature flags can
help you re-ship safely later.

## The rollback procedure (Fly.io)

```bash
# 1. List recent releases
flyctl releases list --app notes-api | head -10

# 2. Identify the last known-good release ID (typically the one before
#    the current). Example: v45 deployed at 14:24 = bad; v44 at 13:01 = good.

# 3. Roll back
flyctl releases rollback v44 --app notes-api

# 4. Watch
flyctl status --app notes-api
flyctl logs --app notes-api
```

Expected: 60–120 seconds. Pods recycle one at a time (rolling restart by default).

## After rollback

1. **Verify the regression is gone.** Check Grafana dashboard; error
   rate should return to baseline.
2. **Communicate.** Slack `#incidents` channel: "rolled back to v44;
   error rate normalized."
3. **Pin the bad release.** Mark PR #N as "do not redeploy until fixed."
4. **Schedule the postmortem** within 48 hours (template in
   `docs/03-postmortems/`).

## Database migrations: the hard case

If the bad deploy included a DB migration, rollback is no longer just
"redeploy the binary." Three scenarios:

### A — Migration was additive (new column, new table, new index)

Safe to leave applied. Roll back the binary; the new column/table/index
is unused. Drop it later when convenient.

### B — Migration was destructive (`DROP COLUMN`, `DROP TABLE`)

You probably can't roll back without restoring from backup. Two
mitigations:

- **Don't ship destructive migrations.** Always do `ALTER TABLE ... DROP COLUMN` in a *follow-up* PR
  after the binary stops referencing the column for at least one release.
- **If you must:** restore the latest backup that predates the migration to a fresh DB,
  swap connection strings, and accept the data lost between backup and now (with a separate plan
  to recover it).

### C — Migration was a data fill (`UPDATE ... SET`)

The values are now in the DB; the binary rollback can't unset them.
Treat this as an additive migration — the data is fine; the old binary
won't notice or care.

## Forward-fix instead

A forward fix is appropriate when:

- The bug is in a single function with a one-line fix.
- A flag exists to disable the new feature.
- The on-call engineer is confident in the fix path.

The skill is recognizing when *not* to forward-fix. Most outage
escalations involve "we kept trying to forward-fix instead of rolling
back." When in doubt → roll back.

## Feature flags as the third option

If the regression is contained to one feature behind a flag:

```bash
flyctl secrets set FEATURE_NEW_SEARCH=false --app notes-api
flyctl restart --app notes-api
```

This is the cleanest "stop the bleeding" option. Use feature flags
on all new features by default.

## Communication template

```
TITLE: notes-api regression mitigated by rollback

WHEN: 14:32–14:46 UTC, 2026-05-26
WHAT: GET /v1/notes/search returning 500 due to missing index on a new query
WHO: ~3000 users seeing 8% error rate
ACTION: rolled back to v44 at 14:46 UTC; error rate normalized
NEXT: postmortem on Friday; forward fix pinned to PR #2091
```

Post this in `#incidents` and on the status page (paraphrased).

## Practice this

The team should rehearse rollback once per quarter. A 30-minute
exercise:

1. Pick a staging environment.
2. Deploy a deliberately bad version.
3. Run the rollback.
4. Time how long the on-call engineer needed.

Goal: under 5 minutes from "I should roll back" to "rollback complete."

## Related

- Phase 12 lesson 5 (Incident response + postmortems)
- `docs/runbooks/5xx-spike.md`
- `.github/workflows/cd.yml` (which the rollback button operates against)
