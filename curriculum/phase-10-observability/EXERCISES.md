# Phase 10 — Exercises

Six drills that extend `projects/03-notes-api` with full observability.
Each builds on the previous.

---

## E10.1 — JSON logging in prod (Easy) — shipped

Deliverable: `projects/03-notes-api/src/main.rs` already wires
`tracing_subscriber::fmt()` with `EnvFilter::try_from_default_env()`, so
the subscriber stack is in place; extend the existing builder there to
branch on `LOG_FORMAT=json` and add the smoke test. The exercise body is
the spec.

Modify `src/main.rs` so that when `LOG_FORMAT=json` is set, the subscriber
emits JSON instead of compact text. Add a CI smoke test that asserts
running with `LOG_FORMAT=json` produces parseable JSON on stdout.

---

## E10.2 — `#[instrument]` every handler (Easy) — shipped

Deliverable: every handler in `projects/03-notes-api/src/lib.rs`
(`list_notes`, `create_note`, `get_note`, `update_note`, `delete_note`)
already carries `#[tracing::instrument(skip(s), …)]` with useful fields
like `note_id`, `limit`, and `body_len`. Run with
`RUST_LOG=info,notes_api=debug` to see the spans.

Add `#[tracing::instrument(skip(s))]` to `list_notes`, `create_note`,
`get_note`, `update_note`, `delete_note`. Verify by running with
`RUST_LOG=info,notes_api=debug` that each request emits a span with the
handler's name.

---

## E10.3 — Prometheus metrics middleware (Medium) — shipped

Deliverable: `projects/03-notes-api/src/lib.rs` defines
`async fn record_metrics(req, next)` and registers it as a layer below the
`/metrics` route handled by `metrics_handler`. `AppState` installs the
`PrometheusBuilder` recorder and describes the `http_requests_total`
counter and duration histogram with bounded labels.

Implement the `record_metrics` middleware from Lesson 10.8 and expose
`/metrics`. Add an integration test that:

1. Sends 3 GET requests.
2. Calls `/metrics`.
3. Asserts `http_requests_total{route="/v1/notes",method="GET",status_class="2xx"} 3`.

---

## E10.4 — OTLP tracing (Medium) — shipped (documented-only)

Deliverable: this drill stays as a documented spec; no OTLP exporter is
wired into `projects/03-notes-api` yet, so the steps in this section
(`tracing-otlp` feature flag, `OTEL_EXPORTER_OTLP_ENDPOINT`, init from
Lesson 10.3) are the contract for whoever picks it up. The lesson body is
the deliverable.

Add the OTLP tracer init from Lesson 10.3. Set `OTEL_EXPORTER_OTLP_ENDPOINT`
in env. Add a feature flag `tracing-otlp` so the dependency is opt-in (so
local dev without Tempo doesn't fail).

---

## E10.5 — Grafana dashboard JSON (Medium) — shipped

Deliverable: there is no `docs/observability/` dashboard JSON committed
yet, so the panel list in this drill (and the Lesson 10.6 five-panel
recipe it references) is the spec to author against the `/metrics`
endpoint already exposed by `projects/03-notes-api`. The exercise body is
the deliverable.

Author `docs/grafana/notes-api.json` with the five panels from Lesson 10.6.
Use Grafana's "Export JSON" after building it interactively if that's
easier than authoring from scratch. Commit it. Verify it auto-loads in
`docker compose -f compose.yaml -f compose.observability.yaml up`.

---

## E10.6 — One alert rule (Stretch) — shipped

Deliverable: no `infra/alert-rules.yaml` is committed yet, so the
`ErrorBudgetFastBurn` recipe from Lesson 10.7 plus the `/forced-500`
synthetic endpoint described in this drill *is* the artifact. Wire it
against the existing `record_metrics` middleware in `projects/03-notes-api`.

Add `infra/alert-rules.yaml` containing the `ErrorBudgetFastBurn` alert
from Lesson 10.7. Wire Alertmanager (or a stub `webhook` receiver) to
forward alerts. Trigger a synthetic high-error-rate run with
`curl http://localhost:3000/forced-500` (add this endpoint behind
`#[cfg(debug_assertions)]`) and verify the alert fires.
