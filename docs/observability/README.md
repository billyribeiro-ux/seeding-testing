# notes-api observability

End-to-end recipe for getting traces *and* metrics out of `notes-api`
into a Grafana stack you can run locally on a laptop.

Two signals, two pipelines:

| Signal  | Source in notes-api                                              | Transport       | Storage         | UI       |
|---------|------------------------------------------------------------------|-----------------|-----------------|----------|
| Metrics | `metrics`/`metrics-exporter-prometheus` — scraped at `/metrics`  | Prom scrape     | Prometheus      | Grafana  |
| Traces  | `tracing` + `tracing-opentelemetry` — pushed via OTLP/gRPC       | OTLP (port 4317)| Tempo or Jaeger | Grafana  |

The traces pipeline is opt-in: if `OTEL_EXPORTER_OTLP_ENDPOINT` is not
set, the service falls back to the original `tracing-subscriber` fmt
output — no OTLP collector required, no new failure mode.

Cross-reference: the perf methodology lives in
[`docs/perf/methodology.md`](../perf/methodology.md). The dashboard in
this directory is what you stare at during the measurement window of
that protocol.

---

## TL;DR — single curl + env var

```bash
# 1. Bring the stack up (see compose snippet below).
# 2. Point notes-api at the local OTel collector and start it.
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 cargo run -p notes-api
# 3. In another shell, exercise the API to generate a span.
curl -sS http://localhost:3000/v1/notes
# 4. Open Grafana → Explore → Tempo, search for service.name=notes-api.
```

Want a deployment-environment tag on every span (so prod and staging
don't share a Tempo namespace)?

```bash
DEPLOYMENT_ENVIRONMENT=staging \
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317 \
  ./notes-api
```

---

## Local observability stack (docker-compose snippet)

DO NOT add this to the repo's root `compose.yaml`. It's documentation
of one known-good arrangement, not a committed dev dependency. Drop it
into a scratch `observability-compose.yaml` next to the repo and run
`docker compose -f observability-compose.yaml up -d`.

```yaml
# observability-compose.yaml  (sketched; not under version control)
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:0.111.0
    command: ["--config=/etc/otel-collector-config.yaml"]
    volumes:
      - ./otel-collector-config.yaml:/etc/otel-collector-config.yaml:ro
    ports:
      - "4317:4317"   # OTLP/gRPC  ← notes-api ships here
      - "4318:4318"   # OTLP/HTTP

  tempo:
    image: grafana/tempo:2.6.1
    command: ["-config.file=/etc/tempo.yaml"]
    volumes:
      - ./tempo.yaml:/etc/tempo.yaml:ro
    ports:
      - "3200:3200"   # Tempo HTTP query
      - "4327:4317"   # OTLP/gRPC (collector → Tempo)

  # If you'd rather use Jaeger instead of Tempo, swap the tempo service
  # for the all-in-one image:
  #
  # jaeger:
  #   image: jaegertracing/all-in-one:1.62
  #   ports:
  #     - "16686:16686"  # Jaeger UI
  #     - "4327:4317"    # OTLP/gRPC (collector → Jaeger)

  prometheus:
    image: prom/prometheus:v3.0.1
    command:
      - --config.file=/etc/prometheus/prometheus.yml
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana:11.4.0
    environment:
      GF_AUTH_ANONYMOUS_ENABLED: "true"
      GF_AUTH_ANONYMOUS_ORG_ROLE: "Admin"
    volumes:
      - ./grafana-datasources.yaml:/etc/grafana/provisioning/datasources/ds.yaml:ro
    ports:
      - "3001:3000"
```

Minimal collector config (writes traces to Tempo, drops everything
else):

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317

exporters:
  otlp/tempo:
    endpoint: tempo:4317
    tls:
      insecure: true

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [otlp/tempo]
```

Minimal Prometheus scrape config:

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
scrape_configs:
  - job_name: notes-api
    static_configs:
      - targets: ["host.docker.internal:3000"]
    metrics_path: /metrics
```

(On Linux without `host.docker.internal` you'll want `network_mode:
host` on the prometheus service, or point at the host's IP directly.)

---

## Importing the dashboard

The dashboard JSON in this directory (`notes-api-dashboard.json`) is
Grafana 10/11-compatible (`schemaVersion: 39`). Two ways to bring it in:

### One-shot import

1. Grafana → Dashboards → New → Import.
2. Upload `notes-api-dashboard.json` (or paste the contents).
3. When prompted, pick the Prometheus datasource — the
   `${DS_PROMETHEUS}` variable will be replaced everywhere.
4. Save.

### Provisioned (recommended for the local stack)

Drop a provisioning file next to your compose:

```yaml
# grafana-dashboards.yaml
apiVersion: 1
providers:
  - name: "notes-api"
    folder: "notes-api"
    type: file
    options:
      path: /var/lib/grafana/dashboards
```

…and mount this directory into the Grafana container at
`/var/lib/grafana/dashboards`. Grafana will pick up the JSON on
startup, no clicks required.

---

## What the dashboard shows

Six panels, RED-method-organized:

| #  | Panel                       | Type        | Query summary                                             |
|----|-----------------------------|-------------|-----------------------------------------------------------|
| 1  | Request rate by status class| Time series | `rate(http_requests_total[...])` grouped by `status_class`|
| 2  | Latency p50 / p95 / p99     | Stat        | `histogram_quantile(...)` over the duration histogram     |
| 3  | 5xx error rate (%)          | Gauge       | 5xx rate / total rate, red >0.1%                          |
| 4  | Per-route p95 latency       | Time series | p95 grouped by `route`                                    |
| 5  | Request volume by route     | Bar chart   | rate by `route` (instant query)                           |
| 6  | Latency heatmap             | Heatmap     | bucket-rate over the duration histogram                   |

The 5xx panel's threshold (0.1%) is the value the on-call runbook ties
back to — if the gauge goes red, page.

---

## Verifying traces are flowing

The OTLP wire path can't be exercised by `cargo test` (the test would
need a live collector and would be flaky in CI). Instead, do this
manually:

```bash
# Terminal 1 — stack up.
docker compose -f observability-compose.yaml up -d

# Terminal 2 — notes-api pointing at the collector.
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
DEPLOYMENT_ENVIRONMENT=local \
cargo run -p notes-api

# Terminal 3 — make a real request that will produce a span.
curl -sS -X POST http://localhost:3000/v1/notes \
  -H 'content-type: application/json' \
  -d '{"body":"hello tempo"}'
```

Then in Grafana (`http://localhost:3001`):

1. Explore → pick the Tempo datasource.
2. Service Name = `notes-api`.
3. You should see a span named `http` (the tower-http TraceLayer span)
   with a child span from the `#[tracing::instrument]`-ed handler
   (`create_note`).
4. Resource attributes — confirm `service.name=notes-api`,
   `service.version=0.1.0`, and (because we set
   `DEPLOYMENT_ENVIRONMENT=local`) `deployment.environment=local`.

If you don't see spans:

* Confirm the collector is actually listening on `:4317` —
  `docker compose logs otel-collector` should show "Everything is
  ready. Begin running and processing data."
* Confirm notes-api is exporting — its startup log line should read
  `OTLP tracing enabled  otlp.endpoint="http://localhost:4317"`.
* Confirm Tempo received the spans — `curl
  http://localhost:3200/api/echo` (Tempo is up) and check Explorer's
  service dropdown for `notes-api`.

---

## Verifying metrics are flowing

Metrics use the *existing* `/metrics` endpoint — nothing new for this
PR. To validate the dashboard:

```bash
curl -sS http://localhost:3000/metrics | grep ^http_requests_total
curl -sS http://localhost:9090/api/v1/query?query=http_requests_total
```

Open the dashboard in Grafana, pick the Prometheus datasource for
`${DS_PROMETHEUS}`, and the panels should populate within one scrape
interval (15 s by default).

---

## Related docs

* [`docs/perf/methodology.md`](../perf/methodology.md) — the perf
  protocol this dashboard is the readout for.
* `projects/03-notes-api/src/telemetry.rs` — the OTLP wiring this
  document is the operational counterpart to.
* `projects/03-notes-api/src/lib.rs` — the metrics middleware and
  `/metrics` handler the dashboard reads from.
