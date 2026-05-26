# Lesson 11.8 — Performance Budgets and Deploy Gates

> **Concept first:** a budget is a number engineers agree on *before*
> writing the code. Reviews enforce it. Regressions block the deploy.
> **Time:** 15 minutes.

## What a budget looks like

```
Route                  p99 budget   Owner       Last verified
/v1/notes              200ms        api-team    2026-05-26
/v1/notes/{id}         150ms        api-team    2026-05-26
POST /v1/notes         300ms        api-team    2026-05-26
GET /me                100ms        auth-team   2026-05-26
POST /auth/login       400ms        auth-team   2026-05-26
POST /webhooks/stripe  100ms        billing     2026-05-26
```

Five columns: route, budget, owner, last-verified date. Commit to
`docs/perf/budgets.md`. Treat it as a *living contract*.

## The two questions before adding to it

1. **Why this number?** Document the reasoning. "p99 of 200ms because
   our customer-facing dashboard polls every 5 seconds; a 200ms tail is
   invisible to humans."
2. **What blows the budget?** "A 10× growth in note count without an
   added index would push us over. The index `notes_user_id_created_at_idx`
   keeps it bounded."

Without those answers, the budget is arbitrary.

## Three gates

1. **PR gate.** Big refactors should run the load tests and attach the
   before/after report to the PR. Reviewers see the numbers.
2. **Pre-deploy gate.** Optional canary: deploy to staging, run the
   load suite, check no route breaches its budget. If breach, block the
   deploy.
3. **Production observation.** Grafana alert at "p99 > budget for 5
   minutes." Page on it (Phase 10.7).

Three gates of increasing cost. Even one is better than none.

## When to violate the budget on purpose

It's allowed:

- **A new feature** that has no budget yet. Add a temporary entry; set
  a budget after one week of production data.
- **A new tier of service.** Free users get more lenient budgets than
  Enterprise.
- **Known external dependency.** "Stripe Checkout p99 is whatever
  Stripe is." Document and move on.

What's *not* allowed: ignoring a breach silently.

## The "find the slowest route" query

For a Grafana alert:

```promql
topk(5,
  histogram_quantile(0.99, sum by (le, route) (rate(http_request_duration_seconds_bucket[5m])))
)
```

Top 5 worst p99s. Pin this to the team's dashboard.

For a periodic email to the team:

```sql
-- if you store metrics in the DB
SELECT route, percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms) AS p99
FROM http_metrics
WHERE bucket >= NOW() - INTERVAL '7 days'
GROUP BY route
ORDER BY p99 DESC
LIMIT 10;
```

## Performance retrospectives

Quarterly:

1. Review the budget table.
2. Update *last-verified* by re-running the load tests.
3. Note any route that has trended toward its budget (warning signal).
4. Identify one route to optimize for the *next* quarter.

These take 90 minutes per service and pay for themselves.

## What seniors hold their teams to

- **Every PR with a "perf" tag includes before/after numbers.**
- **No "I think it'll be faster."** Prove it.
- **No "let's optimize later."** If you don't optimize during the
  implementation, you'll burn 5× the time understanding the code later.

## Why this matters

- **Performance work without a target is just busywork.** The budget is
  the target.
- **Gates make regressions costly to ship.** Without them, slow drift is
  inevitable.
- **The retrospective makes the budget honest.** Without it, "last
  verified 2026-01" becomes the norm.

## Green-bar checkpoint

- You can author a perf budget row from scratch with all five columns
  filled.
- You can sketch a Grafana alert that fires when a route exceeds its
  budget.
- You can name three places performance work belongs (PR review,
  pre-deploy, prod observation).

Phase 11 is complete. Phase 12 — **Principal Engineer Skills** — the
non-code part of the role.
