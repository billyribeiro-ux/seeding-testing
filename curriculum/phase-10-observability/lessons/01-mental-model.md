# Lesson 10.1 — Three Signals, One Trace Id

> **Concept first:** logs, metrics, traces — each answers a different
> question. A trace id ties them together.
> **Time:** 15 minutes.

## The three signals

| | Logs | Metrics | Traces |
|---|---|---|---|
| **Answers** | "What happened in this one request?" | "How is the fleet doing right now?" | "What downstream calls happened, in what order, how long each?" |
| **Cardinality** | High (one row per event) | Low (counters and buckets) | Medium (one tree per request) |
| **Cost** | Storage-heavy | Cheap | Medium (sampled) |
| **Tools** | Loki, ELK, Datadog Logs | Prometheus, Mimir, Datadog Metrics | Tempo, Jaeger, Honeycomb, Datadog APM |

A naive engineer logs everything. A senior engineer adds metrics for what
matters, traces for what spans services, and *minimizes* logs.

## The four golden signals

For each service, the SRE bible says you need:

1. **Latency** — how long a request takes (histogram).
2. **Traffic** — requests per second (counter).
3. **Errors** — error rate (counter).
4. **Saturation** — how full the system is (CPU, memory, pool utilization).

Four metrics, four dashboards, four alerts. From those you can diagnose
~80% of incidents.

## The trace id is the glue

```
request comes in
  → hooks.server.ts generates a request_id (or reads X-Request-Id)
  → tracing::info_span!("http", req_id = %req_id)
  → log lines tagged with req_id
  → metrics labeled with… nothing (metrics are aggregate)
  → traces include req_id as a tag

later, at 2 AM:
  user reports "I got an error at 4:33pm"
  → grep logs for the time window
  → find the req_id
  → pull the trace by req_id
  → see exactly which downstream call failed
```

The request id is the *anchor*. Every log, every span, every error report
includes it. You can correlate across signals.

## High cardinality is a metric anti-pattern

A common mistake: adding `user_id` as a label on a counter.

```rust
counter!("requests_total", "user_id" => uid).increment(1);   // ☠️
```

If you have a million users, you have a million distinct time series in
Prometheus. Disk fills; queries slow to a crawl; alerts time out.

**Rule: labels must have *bounded* cardinality.** Endpoint name, HTTP
method, status class — fine. User id, request id, document id —
**never**. Those belong in logs/traces.

## What to instrument first

Walk this order:

1. **Add a request span to every handler.** One `#[instrument]` per route.
2. **Count requests by route + status class.** Four lines of code.
3. **Histogram latency per route.** Same shape.
4. **Add error logs at the boundary.** Already done in
   `IntoResponse for ApiError`.
5. **Export to OTLP / Prometheus.** One-time setup per service.
6. **Build the one dashboard** for that service.

Stop. Live with it for a week. Then add what's missing.

## Why this matters

- **You can't fix what you can't see.** Observability isn't optional past
  the first 100 users.
- **Cardinality discipline keeps cost bounded.** A naive instrumentation
  job balloons logging bills 10×.
- **The four golden signals + request id** are 80% of incident response.

## Green-bar checkpoint

- You can name the three signals and what each is best at.
- You can name the four golden signals.
- You can articulate why labeling a metric with `user_id` is dangerous.

Next: `lessons/02-tracing-in-rust.md`.
