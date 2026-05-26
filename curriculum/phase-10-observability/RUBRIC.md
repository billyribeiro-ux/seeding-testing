# Phase 10 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Tracing** | `println!` everywhere | `tracing` spans on handlers | `#[instrument]` with `skip`/`fields`; nested spans for downstream calls |
| **Logs** | Unstructured text | JSON structured; levels deliberate | PII-aware redaction; sampling; one boundary for emission |
| **OTel export** | None | OTLP to a backend | Service.name resource; batching; head sampling; graceful shutdown |
| **Metrics** | None | RED metrics per service | + saturation gauges, business metrics, careful label cardinality |
| **Dashboards** | "I built it in the UI" | Committed JSON | Provisioned via compose; deploy annotations; SLO panel |
| **Alerts** | None or too many | A few key alerts | Burn-rate alerts (fast + slow); runbooks linked; suppression rules |
| **SLOs** | "we just want it fast" | Documented SLOs per service | Error budget tracked; deploy freeze when burn is high |
| **Incident response** | Ad-hoc | Runbook per alert | Postmortems; trend lines for repeat incidents |
| **Cost discipline** | "More is better" | Aware of cardinality | Audits cardinality regularly; samples high-rate events; redacts long fields |

## Self-check before moving to Phase 11

- [ ] `cargo run -p notes-api` emits JSON logs (with `LOG_FORMAT=json`).
- [ ] `/metrics` returns Prometheus exposition format.
- [ ] Traces appear in Tempo (or your chosen backend).
- [ ] You've authored and committed at least one Grafana dashboard.
- [ ] You've authored and committed at least one alert rule.
- [ ] You can explain the four golden signals and write the PromQL for
      each.
- [ ] CI is green on your branch.

Phase 11 — **Performance, Caching, Background Jobs** — uses the
observability you just installed to find bottlenecks and fix them.
