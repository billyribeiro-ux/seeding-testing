# Lesson 10.8 — Instrument `notes-api`

> **The capstone of Phase 10.** Add structured logging, OTLP traces, and
> Prometheus metrics to the Phase 4 `notes-api`.
> **Time:** 90 minutes.

## What you'll change

In `projects/03-notes-api`:

1. **Cargo.toml** — add `tracing-opentelemetry`, `opentelemetry`,
   `opentelemetry_sdk`, `opentelemetry-otlp`, `metrics`,
   `metrics-exporter-prometheus`.
2. **`src/lib.rs`** — `#[tracing::instrument]` every handler; wrap the
   router in a Prometheus middleware; expose `GET /metrics`.
3. **`src/main.rs`** — initialize OTel tracer and Prometheus exporter
   alongside the existing `tracing_subscriber`.
4. **`docs/grafana/notes-api.json`** — committed dashboard.
5. **`compose.observability.yaml`** — Tempo + Prometheus + Loki +
   Grafana.

The full walkthrough is in EXERCISES.md E10.1–E10.6. Each step has
sample code; you'll wire it together to extend Phase 4's project.

## The key bits

### Instrumenting handlers

```rust
#[tracing::instrument(skip(s), fields(limit = q.limit))]
async fn list_notes(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<NoteDto>>, ApiError> {
    /* ... existing body ... */
}
```

Every call to `list_notes` becomes a span called `list_notes` with the
`limit` field recorded.

### A Prometheus middleware

```rust
async fn record_metrics(
    matched_path: Option<MatchedPath>,
    method: Method,
    req: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let route = matched_path.map(|p| p.as_str().to_string()).unwrap_or_else(|| "unknown".into());
    let res = next.run(req).await;
    let status_class = format!("{}xx", res.status().as_u16() / 100);
    metrics::counter!("http_requests_total",
        "route" => route.clone(),
        "method" => method.to_string(),
        "status_class" => status_class,
    ).increment(1);
    metrics::histogram!("http_request_duration_seconds",
        "route" => route,
        "method" => method.to_string(),
    ).record(started.elapsed().as_secs_f64());
    res
}
```

Layer it on the router: `.layer(middleware::from_fn(record_metrics))`.

### Exposing `/metrics`

```rust
.route("/metrics", get(|| async {
    metrics_exporter_prometheus::PrometheusHandle::default().render()
}))
```

Or use the `metrics-exporter-prometheus` builder's built-in HTTP listener
on a separate port (more common in production).

### OTLP tracer init

```rust
let exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_tonic()
    .with_endpoint("http://localhost:4317")
    .build()?;
let provider = TracerProvider::builder()
    .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
    .with_resource(Resource::new([KeyValue::new("service.name", "notes-api")]))
    .build();
let tracer = provider.tracer("notes-api");
let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
```

Compose with the existing `tracing_subscriber::fmt()` via the `Registry`
pattern from Lesson 10.3.

## The dashboard

`docs/grafana/notes-api.json` (sketch — actual JSON in EXERCISES.md):

- Rate by route (top-left)
- Errors by route (top-right)
- p50/p95/p99 latency (middle)
- DB pool saturation (bottom)
- Traces panel: Tempo query showing the most recent 50 traces

## Running it

```bash
make up-obs              # compose with the observability stack
cargo run -p notes-api
seq 1 1000 | xargs -P20 -I{} curl -s http://localhost:3000/v1/notes -o /dev/null
# Then open:
# http://localhost:3001  Grafana → MemberClub → notes-api
# http://localhost:9090  Prometheus
# http://localhost:3200  Tempo
```

Within seconds, the dashboard shows:

- The request rate from your `xargs` loop.
- p50 ~1 ms, p99 ~10 ms.
- 0% error rate (until you `curl /v1/notes/99999` for a 404 — then 0.1%).
- A populated trace tree per request.

## Why this matters

- **Adding observability after the fact is painful.** Bake it in from
  day one.
- **The same patterns scale to every service** — auth-demo,
  webhook-receiver, MemberClub web. Copy the boilerplate; instrument the
  business events that differ.
- **One dashboard per service** + one overview = the operational picture.

Phase 10 is complete. Phase 11 — **Performance, Caching, Background
Jobs** — uses the observability we just installed to find and fix
bottlenecks.
