# Lesson 10.4 — Prometheus Metrics: The RED Method

> **Concept first:** four metrics per service cover ~80% of operational
> visibility. Don't over-instrument; do under-label.
> **Time:** 20 minutes.

## RED — Rate, Errors, Duration

Three of the four golden signals, packaged as the *RED method*:

```
http_requests_total{route,method,status_class}              counter   ← rate + errors
http_request_duration_seconds{route,method}                 histogram ← duration
```

Add `process_open_fds`, connection pool gauges, queue depth — these are
saturation signals. The four together (rate, errors, duration, saturation)
are the golden set.

## In Rust

```toml
metrics = "0.23"
metrics-exporter-prometheus = "0.16"
metrics-util = "0.18"
```

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

fn init_metrics() {
    PrometheusBuilder::new()
        .install()
        .expect("metrics exporter installed");

    metrics::describe_counter!("http_requests_total", "All HTTP requests");
    metrics::describe_histogram!("http_request_duration_seconds", "Request latency");
}
```

`PrometheusBuilder::install()` starts a `/metrics` endpoint on its own
port (default 9000). For Axum, install as a layer instead and expose the
endpoint inside the same router.

## Recording

```rust
let started = std::time::Instant::now();
// ... handle request ...
metrics::counter!("http_requests_total",
    "route" => "/v1/notes",
    "method" => "GET",
    "status_class" => "2xx",
).increment(1);
metrics::histogram!("http_request_duration_seconds",
    "route" => "/v1/notes",
    "method" => "GET",
).record(started.elapsed().as_secs_f64());
```

In production we wrap this in a middleware so handlers don't repeat the
boilerplate.

## Cardinality discipline

The two labels above (`route`, `method`, `status_class`) have:

- `route`: ~50 endpoints
- `method`: 5–8 verbs
- `status_class`: 5 buckets (1xx, 2xx, 3xx, 4xx, 5xx)

Total time series: ~2000. Manageable.

Add `user_id` and you have millions. Don't.

| Good labels (bounded) | Bad labels (unbounded) |
|---|---|
| `route` (matched-path, not full URL) | `path` (with query string) |
| `method` | `request_id` |
| `status_class` (`2xx`, not `200`) | `user_id` |
| `tenant_id` (if you have 10–100 tenants) | `error_message` |
| `feature_flag` | `timestamp` |

## PromQL queries you'll write a hundred times

```
# requests/sec last 5m
sum(rate(http_requests_total[5m]))

# error rate by route
sum by (route) (rate(http_requests_total{status_class="5xx"}[5m]))
  /
sum by (route) (rate(http_requests_total[5m]))

# p99 latency
histogram_quantile(0.99, sum by (le, route) (rate(http_request_duration_seconds_bucket[5m])))

# saturation: open DB connections vs pool size
sum(pg_pool_in_use) / sum(pg_pool_size)
```

Memorize the histogram_quantile shape. It's the difference between
"average latency looks fine" and "the p99 is melting."

## Business metrics

Beyond RED, instrument business events:

```rust
metrics::counter!("signups_total").increment(1);
metrics::counter!("subscriptions_created_total", "tier" => "pro").increment(1);
metrics::gauge!("active_subscriptions", "tier" => "pro").set(count);
metrics::counter!("refunds_total", "reason" => &reason).increment(1);
```

These give product the dashboards they actually want. Don't make them ask
the data team for a query.

## The `/metrics` endpoint

```
GET /metrics

# HELP http_requests_total All HTTP requests
# TYPE http_requests_total counter
http_requests_total{route="/v1/notes",method="GET",status_class="2xx"} 1234
...
# HELP http_request_duration_seconds Request latency
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{route="/v1/notes",method="GET",le="0.005"} 1000
http_request_duration_seconds_bucket{route="/v1/notes",method="GET",le="0.01"} 1200
...
```

Prometheus scrapes this every 15 s. The endpoint is *public to the
internal network*; never expose it on the public internet.

## Why this matters

- **RED is the universal vocabulary.** Every monitoring tool understands
  rate + errors + duration.
- **Cardinality discipline keeps the bill bounded.** A million series
  isn't a metric, it's a database.
- **Business metrics build cross-team trust.** Product, sales, support —
  they all want the same dashboards engineers do.

## Green-bar checkpoint

- You can name RED + saturation and the four metrics they correspond to.
- You can pick "good label" vs "bad label" for ten candidate labels.
- You can write the PromQL for "p99 latency by route."

Next: `lessons/05-structured-logging.md`.
