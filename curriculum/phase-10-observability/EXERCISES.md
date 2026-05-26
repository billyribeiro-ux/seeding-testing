# Phase 10 — Exercises

Six drills that extend `projects/03-notes-api` with full observability.
Each builds on the previous.

---

## E10.1 — JSON logging in prod (Easy)

Modify `src/main.rs` so that when `LOG_FORMAT=json` is set, the subscriber
emits JSON instead of compact text. Add a CI smoke test that asserts
running with `LOG_FORMAT=json` produces parseable JSON on stdout.

---

## E10.2 — `#[instrument]` every handler (Easy)

Add `#[tracing::instrument(skip(s))]` to `list_notes`, `create_note`,
`get_note`, `update_note`, `delete_note`. Verify by running with
`RUST_LOG=info,notes_api=debug` that each request emits a span with the
handler's name.

---

## E10.3 — Prometheus metrics middleware (Medium)

Implement the `record_metrics` middleware from Lesson 10.8 and expose
`/metrics`. Add an integration test that:

1. Sends 3 GET requests.
2. Calls `/metrics`.
3. Asserts `http_requests_total{route="/v1/notes",method="GET",status_class="2xx"} 3`.

---

## E10.4 — OTLP tracing (Medium)

Add the OTLP tracer init from Lesson 10.3. Set `OTEL_EXPORTER_OTLP_ENDPOINT`
in env. Add a feature flag `tracing-otlp` so the dependency is opt-in (so
local dev without Tempo doesn't fail).

---

## E10.5 — Grafana dashboard JSON (Medium)

Author `docs/grafana/notes-api.json` with the five panels from Lesson 10.6.
Use Grafana's "Export JSON" after building it interactively if that's
easier than authoring from scratch. Commit it. Verify it auto-loads in
`docker compose -f compose.yaml -f compose.observability.yaml up`.

---

## E10.6 — One alert rule (Stretch)

Add `infra/alert-rules.yaml` containing the `ErrorBudgetFastBurn` alert
from Lesson 10.7. Wire Alertmanager (or a stub `webhook` receiver) to
forward alerts. Trigger a synthetic high-error-rate run with
`curl http://localhost:3000/forced-500` (add this endpoint behind
`#[cfg(debug_assertions)]`) and verify the alert fires.
