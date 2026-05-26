# Lesson 10.3 — OpenTelemetry Export

> **Concept first:** OpenTelemetry (OTel) is the vendor-neutral protocol for
> shipping signals. Once we instrument with `tracing`, we plug in an OTLP
> exporter and any backend (Tempo, Jaeger, Datadog, Honeycomb) can ingest
> them.
> **Time:** 15 minutes.

## The stack

```
Rust app
  └── tracing                       ← spans, events
  └── tracing_opentelemetry         ← bridges tracing → OTel
  └── opentelemetry_otlp            ← OTLP/gRPC exporter
       │
       ▼  push spans
  OTel Collector (or Tempo direct)
       │
       ▼
  Tempo / Jaeger / Honeycomb / ...
```

Four crates, ~50 lines of setup, one config block. Done.

## The minimum setup

```rust
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

fn init_tracing() {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()
        .expect("exporter");
    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(opentelemetry_sdk::Resource::new([
            opentelemetry::KeyValue::new("service.name", "notes-api"),
        ]))
        .build();
    let tracer = provider.tracer("notes-api");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = tracing_subscriber::fmt::layer().compact();
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();

    let subscriber = Registry::default().with(env_filter).with(fmt_layer).with(otel_layer);
    tracing::subscriber::set_global_default(subscriber).expect("set subscriber");
}
```

Five rules:

1. **`service.name` resource attribute.** Every span identifies which
   service produced it.
2. **`with_batch_exporter`** so we don't ship one span at a time. Batches
   reduce ingest cost.
3. **`Registry::default().with(...).with(...)`** composes layers. Add
   format, OTel, env filter, sampler — all separately.
4. **Don't forget the env filter.** Without it you ship every TRACE event.
5. **Shutdown gracefully.** Call `provider.shutdown()` in your shutdown
   handler so the last batch flushes.

## Backends

| Backend | Type | Notes |
|---|---|---|
| **Tempo** | Open-source, Grafana stack | Cheap; pairs with Loki + Prometheus |
| **Jaeger** | Open-source | Older, still solid |
| **Honeycomb** | SaaS | Best-in-class for high-cardinality |
| **Datadog APM** | SaaS | Bundled with logs + metrics |

For MemberClub we use the **Grafana stack** (Tempo + Prometheus + Loki +
Grafana) via Docker compose in dev. In production: managed Grafana Cloud
or self-hosted.

## Compose file (added to dev stack)

```yaml
# compose.observability.yaml
services:
  tempo:
    image: grafana/tempo:latest
    command: ["-config.file=/etc/tempo.yaml"]
    volumes: ["./infra/tempo.yaml:/etc/tempo.yaml"]
    ports: ["127.0.0.1:3200:3200", "127.0.0.1:4317:4317"]

  prometheus:
    image: prom/prometheus:latest
    volumes: ["./infra/prometheus.yaml:/etc/prometheus/prometheus.yaml"]
    ports: ["127.0.0.1:9090:9090"]

  loki:
    image: grafana/loki:latest
    ports: ["127.0.0.1:3100:3100"]

  grafana:
    image: grafana/grafana:latest
    ports: ["127.0.0.1:3001:3000"]
    environment: [ "GF_AUTH_ANONYMOUS_ENABLED=true" ]
    volumes:
      - ./infra/grafana-datasources.yaml:/etc/grafana/provisioning/datasources/datasources.yaml
      - ./infra/grafana-dashboards.yaml:/etc/grafana/provisioning/dashboards/dashboards.yaml
      - ./docs/grafana:/var/lib/grafana/dashboards
```

`make up-obs` brings it up alongside Postgres. Dashboards committed in
`docs/grafana/` are auto-loaded.

## Sampling

In production you don't ship every trace — that's expensive. Use
*head-based sampling* (decide at the entry point):

```rust
opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(0.10)   // 10% of requests
```

For errors and latency outliers, use *tail-based sampling* — sample after
the trace completes if the request was slow or errored. Requires a
collector in between.

## Why this matters

- **OTel is the wire protocol of observability.** Switching from Jaeger to
  Honeycomb is a config change, not a code change.
- **The service name + resource attributes** are how spans get attributed
  in dashboards.
- **Batching + sampling = sustainable cost.** Without them, your
  observability bill exceeds your hosting bill.

## Green-bar checkpoint

- You can sketch the four-crate stack (tracing → tracing_opentelemetry →
  otlp → backend).
- You can articulate why `service.name` matters.
- You can pick head- vs tail-based sampling for a given concern.

Next: `lessons/04-prometheus-metrics.md`.
