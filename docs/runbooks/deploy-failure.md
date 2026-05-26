# Runbook: CD pipeline failed

## When this fires

The `cd.yml` workflow failed on `main` or a tag push. The deploy did
*not* go out (we hope — see "deploy partially completed" below).

## The five places CD fails

### 1. `build-api` / `build-web` — image build failed

Most common. Cargo error, type error, missing file.

```bash
# Read the failed job logs
gh run view --log-failed
```

Fix: open a PR with the fix; merging triggers a new CD run. The failed
deploy never happened in production; nothing to roll back.

### 2. `migrate` — DB migration failed

The image built, but the migration step failed. Three sub-cases:

- **Migration SQL has a typo.** Fix the SQL in a new PR; the next CD run
  retries.
- **Migration would lock a busy table.** Add `CONCURRENTLY` to indexes;
  use `ALTER TABLE ... SET ...` instead of full rewrites. Re-deploy.
- **Migration target DB is unreachable.** Check the DB's status; retry
  the GitHub Actions workflow.

The application *was not yet started*, so there's no inconsistency. The
old version is still serving traffic.

### 3. `deploy-api` — Fly.io deploy failed

Most common reasons:

- Image registry credentials expired (rotate `FLY_API_TOKEN`).
- Fly is having a regional outage (check https://status.flyio.net).
- The new image fails health checks (the deploy auto-rolls-back).

```bash
flyctl status --app notes-api
flyctl logs --app notes-api | rg 'health' | head -20
```

If Fly is healthy and the image passes health checks locally, the issue
is a config drift between staging and production. Diff env vars.

### 4. `deploy-web` — depends on `deploy-api`

If `deploy-api` succeeded but `deploy-web` failed, we have a partial
deploy:

- API is new.
- Web is old.
- The web app may be calling new API endpoints that exist, or expecting
  old API responses that have changed.

Two options:

- **Roll forward fast:** re-trigger `deploy-web` (often a transient).
- **Roll the API back** to match the still-old web, then redeploy
  everything in order.

If the API change was backward-compatible (it should be — see ADR 0001's
versioning discussion), the web is safe to lag for an hour while you fix
the deploy.

### 5. `smoke` — post-deploy smoke test failed

The most concerning state: the new version is *running* in production
but the smoke test failed. Either:

- The smoke test itself is flaky (curl `--retry`).
- The new version is broken in a way the build didn't catch.

```bash
# Quick check
curl -sf https://api.memberclub.test/healthz | jq
curl -sf https://api.memberclub.test/v1/notes | head -3
```

If the new version is actually broken, follow `rollback.md`.

## Re-running a failed deploy

For transient errors (network blip, image registry timeout):

```bash
gh run rerun <run-id>
```

For genuine errors (code bug, config error), fix in a new PR. Don't
just re-run; it'll fail the same way.

## When CD is gridlocked

If multiple deploys queue up while you're debugging:

```bash
# Cancel pending deploys to avoid landing them in the wrong order
gh run list --workflow=cd.yml --limit 10
gh run cancel <run-id>
```

Resolve the active issue, then push a fresh commit to re-trigger.

## "Deploy partially completed" — the unfortunate middle state

Symptoms:
- Some pods are on the new version.
- Some are on the old.
- Traffic is split.

Quick fix: complete the rollout.

```bash
flyctl restart --app notes-api  # forces remaining old pods to recycle
```

If that fails, roll back fully.

## Communication

CD failures that **do not affect prod** are silent — no comms needed.

CD failures that **do affect prod** (`smoke` failed, partial deploy)
follow the incident communication rules: `#incidents` Slack post, status
page update if customer-visible.

## Prevent recurrence

Most CD failures are preventable:

- **Pre-merge CI** catches build errors before they reach `main`.
- **Required reviewers** on `.github/workflows/cd.yml` changes — accidental
  edits to deploy logic are dangerous.
- **Canary deploys** (next-quarter goal) catch broken images before they
  hit 100% of traffic.

## Related

- `.github/workflows/cd.yml`
- `docs/runbooks/rollback.md`
- ADR 0006 (outbox makes deploys safer — async work isn't lost)
- Phase 12 lesson 5 (Incident response)
