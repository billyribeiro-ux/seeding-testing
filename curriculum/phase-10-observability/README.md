# Phase 10 — Observability

> **Audience:** you finished Phase 9.
> **Outcome:** every service is instrumented. Traces show *what happened*; metrics show *how often and how slow*; logs answer *why*. Dashboards and alerts make incidents discoverable.
> **Time:** 1–2 weeks.

## The mental model

> **If you can't see it, you can't operate it.**

Three signals, each with a distinct role:

| Signal | What it answers | Cost |
|---|---|---|
| **Logs** | "What did this single request do?" — narrative, high cardinality | Storage |
| **Metrics** | "How many of these are happening, how fast?" — counters, histograms | Low |
| **Traces** | "What downstream calls happened during this request?" — flame-graph view | Medium |

A senior engineer reaches for *traces first*, because traces carry the
request id that ties logs and metrics together.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-mental-model.md` | Three signals; cardinality; the trace-first habit |
| `lessons/02-tracing-in-rust.md` | `tracing` crate, spans, `#[instrument]`, structured fields |
| `lessons/03-opentelemetry-export.md` | OTLP exporter, Tempo/Jaeger backends |
| `lessons/04-prometheus-metrics.md` | Counters, gauges, histograms, RED method (Rate/Errors/Duration) |
| `lessons/05-structured-logging.md` | JSON logs, levels, sampling, redaction |
| `lessons/06-dashboards-as-code.md` | Grafana JSON committed to the repo; one panel per SLO |
| `lessons/07-alerts-and-slos.md` | Alertmanager, the four-golden-signals, error-budget burn |
| `lessons/08-instrument-notes-api.md` | Capstone: instrument notes-api and ship a Grafana dashboard |

## The capstone

We extend `projects/03-notes-api` with:

- `#[tracing::instrument]` on every handler.
- A `/metrics` Prometheus endpoint via `metrics-exporter-prometheus`.
- An OTLP exporter that sends traces to Tempo (via compose).
- A committed `docs/grafana/notes-api.json` dashboard.
- One alert rule (`5xx rate > 1%`) committed as YAML.

The full implementation lives in EXERCISES.md (E10.1–E10.6) — each lesson
ends with a snippet you can drop in.

## Green-bar checkpoint

```bash
# bring up the grafana stack
docker compose -f compose.yaml -f compose.observability.yaml up -d
cargo run -p notes-api
# generate traffic
seq 1 100 | xargs -P10 -I{} curl -s http://localhost:3000/v1/notes -o /dev/null

# Visit:
#   - http://localhost:3001  (Grafana)
#   - http://localhost:9090  (Prometheus)
#   - http://localhost:3200  (Tempo)
```

…and the committed dashboard renders correctly.

## What's next

Phase 11 — **Performance, Caching, Background Jobs**. Now that we can
*see* the system, we optimize it.
