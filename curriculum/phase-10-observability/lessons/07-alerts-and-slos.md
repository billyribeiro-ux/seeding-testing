# Lesson 10.7 — Alerts and SLOs

> **Concept first:** alerts must wake humans. Wake them for the right
> things. SLOs decide what "right" is.
> **Time:** 20 minutes.

## What an SLO is

A **Service Level Objective** is a numeric target: "99.9% of requests
succeed in 300 ms over a 30-day window."

Three pieces:

- **SLI** (indicator) — the metric. E.g. `success_rate = 1 - error_rate`.
- **SLO** (objective) — the threshold. E.g. SLI ≥ 99.9%.
- **SLA** (agreement) — the *contract* with customers. Usually weaker
  than the SLO so the team has slack.

The math:

```
30 days = 43_200 minutes
99.9% allowed downtime = 43.2 minutes / 30 days
```

That's the **error budget**. Spend it on deploys, experiments,
risk-taking. When the budget is empty, you stop shipping risky things.

## Designing alerts

Two rules, both important:

1. **Alert on customer impact, not internal state.** "DB latency p99 > 200ms"
   isn't an alert — it might be fine if the app caches around it.
   "User-facing success rate < 99%" is.
2. **Alert on burn rate, not absolute error count.** "5 errors in the last
   minute" is meaningless; "we'll burn our budget in < 12 hours at this
   rate" is actionable.

A modern alert in PromQL/Prometheus terms:

```yaml
- alert: ErrorBudgetFastBurn
  expr: |
    (1 - sum(rate(http_requests_total{status_class="2xx"}[1h]))
       / sum(rate(http_requests_total[1h])))
    > 0.01    # 1% error rate ⇒ at this pace, 24h burns ~30% of monthly budget
  for: 2m
  labels: { severity: page, slo: api_availability }
  annotations:
    summary: "API error rate {{ $value | humanizePercentage }} for 2m"
    runbook: "https://memberclub.test/runbooks/api-5xx"
```

`for: 2m` debounces noise; `runbook: …` is mandatory — alerts without
runbooks aren't alerts, they're confusing on-call experiences.

## The four-golden-signal alerts

Bare minimum per service:

| Alert | Threshold | Severity |
|---|---|---|
| Error rate too high | 5xx rate > 1% for 2 min | Page |
| Latency too high | p99 > 1 s for 5 min | Page |
| Saturation | DB connection pool > 80% used for 5 min | Warn |
| Traffic anomaly | rate dropped > 50% week-over-week | Warn |

For paying-customer services, page on the first two; warn on the others.
Don't page on warns — they don't need waking up.

## The two-tier burn rate

Google's SRE workbook recommends two simultaneous alerts:

- **Fast burn** — if you'd exhaust the 30-day budget in <2 hours at this
  rate, page now.
- **Slow burn** — if you'd exhaust the budget in <3 days at this rate,
  warn (ticket).

The fast catches outages; the slow catches drift you'd miss otherwise.

## Anti-patterns

- **"Disk usage > 80%."** Useless without context; depends on disk size.
  Use rate of growth + projected days-until-full.
- **"CPU > 80%."** Often fine — auto-scaling exists. Alert on the *thing
  CPU affects* (latency, errors) instead.
- **Per-host alerts.** Modern systems shed hosts; alert on the fleet.
- **Alerts without runbooks.** The on-call engineer reads the runbook
  first. If there isn't one, the alert isn't real.

## Suppression and dependencies

When the DB is down, every service degrades. You'll get 50 alerts. Group
them:

- Alert on the root cause; suppress dependents.
- Use Alertmanager's `inhibit_rules`:

```yaml
inhibit_rules:
- source_matchers: [ "alertname=DBUnreachable" ]
  target_matchers: [ "service=~'notes-api|auth-demo|webhook-receiver'" ]
  equal: [ "cluster" ]
```

One alert for the root cause; the rest go silent.

## Why this matters

- **Alerts that aren't pages are noise.** Noise trains people to ignore
  alerts.
- **SLOs are a *contract* with reality.** No SLO, no negotiable budget,
  no shipping freeze when reliability suffers.
- **Runbooks are mandatory.** "Page someone with no idea what to do" is
  a worse outage than the original.

## Green-bar checkpoint

- You can compute the error budget for a 99.9% SLO over 30 days.
- You can write a PromQL alert that fires on fast burn.
- You can articulate why "CPU > 80%" is a bad alert.

Next: `lessons/08-instrument-notes-api.md`.
