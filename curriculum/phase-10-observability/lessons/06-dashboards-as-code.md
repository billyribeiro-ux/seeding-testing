# Lesson 10.6 — Dashboards as Code

> **Concept first:** the dashboard JSON is committed to the repo. Like any
> code, it's reviewed, versioned, and recoverable.
> **Time:** 15 minutes.

## Why dashboards belong in git

The alternative — clicking around Grafana — works until:

- Someone deletes a panel by accident.
- A junior changes a query and "fixes the metric that was wrong" (which
  was actually right).
- The team has 100 dashboards and none of them are searchable.
- You want the same dashboard in staging and prod.

Commit the JSON, review changes in PRs, generate dashboards from a
deployment pipeline.

## The shape

```
docs/grafana/
├── notes-api.json
├── auth-demo.json
├── memberclub-web.json
├── stripe-receipts.json
└── overview.json
```

One JSON per service + one overview. Each panel:

- Has a clear title.
- Has the query (PromQL or LogQL) inline.
- Documents its threshold (red zone for high-error-rate, etc.).

## Provisioning

Grafana reads `provisioning/dashboards.yaml` at startup and loads dashboards
from a directory. In compose:

```yaml
grafana:
  image: grafana/grafana:latest
  volumes:
    - ./infra/grafana-dashboards.yaml:/etc/grafana/provisioning/dashboards/dashboards.yaml
    - ./docs/grafana:/var/lib/grafana/dashboards
```

```yaml
# infra/grafana-dashboards.yaml
apiVersion: 1
providers:
  - name: 'memberclub'
    folder: 'MemberClub'
    type: file
    options:
      path: /var/lib/grafana/dashboards
```

Now any JSON in `docs/grafana/` shows up automatically.

## Panels every service should have

The minimal dashboard:

1. **Rate** — `sum(rate(http_requests_total[5m]))` by route.
2. **Errors** — error % over 5 min, with red threshold > 1%.
3. **p50/p95/p99 latency** — histogram_quantile across routes.
4. **Saturation** — pool in-use vs size, queue depth.
5. **Top 10 slowest routes** — by p99 latency.

Five panels, one screen. Pin it as the team's home dashboard.

## Annotation: deploys

When CD ships a new version, write an *annotation* to Grafana:

```bash
curl -X POST http://grafana/api/annotations \
    -H 'content-type: application/json' \
    -d '{"text": "deploy notes-api v1.2.3", "tags": ["deploy", "notes-api"]}'
```

The annotation shows as a vertical line on every dashboard. When p99 jumps
right after a deploy, you can *see* the cause.

We add this to our `cd.yml` workflow's post-deploy step.

## SLO panels

For a 99.9% availability SLO:

```
# Error budget remaining (last 30d):
1 - (sum(rate(http_requests_total{status_class="5xx"}[30d])) /
     sum(rate(http_requests_total[30d])))
```

If that drops below 99.9%, you've burned the budget — *don't ship risky
changes* until it recovers.

The four-golden-signals dashboard + the SLO panel = 95% of operational
visibility.

## Tools

- **Grafana** for visualization.
- **Jsonnet / Grafonnet** for generating dashboards from templates if
  you have many services. Not worth the complexity at < 5 services.

## Why this matters

- **Dashboards as code** survive deletions and PR review.
- **Annotations make causality visible.** Deploys jump out.
- **SLO panels turn reliability into a number.** Negotiable, trackable,
  not a vibe.

## Green-bar checkpoint

- You can name the five minimal panels.
- You can write the PromQL for error rate by route.
- You can articulate why a deploy annotation matters.

Next: `lessons/07-alerts-and-slos.md`.
