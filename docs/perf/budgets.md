# Performance Budgets

Per-route latency targets. Reviewed quarterly; enforced in PRs;
observed in Grafana.

A green row means "within budget at last measurement." Red means the
team has a release-blocking ticket open.

## API routes

| Service | Route | p99 budget | Last p99 | Last verified | Owner | Status |
|---|---|---|---|---|---|---|
| notes-api | `GET /v1/notes` | 200 ms | 145 ms | 2026-05-26 | api-team | ✓ |
| notes-api | `GET /v1/notes/{id}` | 150 ms | 51 ms | 2026-05-26 | api-team | ✓ |
| notes-api | `POST /v1/notes` | 300 ms | 87 ms | 2026-05-26 | api-team | ✓ |
| notes-api | `PATCH /v1/notes/{id}` | 250 ms | 76 ms | 2026-05-26 | api-team | ✓ |
| notes-api | `DELETE /v1/notes/{id}` | 200 ms | 45 ms | 2026-05-26 | api-team | ✓ |
| notes-api | `GET /v1/notes/search` | 200 ms | 4200 ms | 2026-05-26 | api-team | ✗ rollback in progress, see postmortem |
| notes-api | `GET /healthz` | 10 ms | 2 ms | 2026-05-26 | api-team | ✓ |
| auth-demo | `POST /auth/register` | 500 ms | 320 ms | 2026-05-26 | auth-team | ✓ argon2 dominates |
| auth-demo | `POST /auth/login` | 400 ms | 312 ms | 2026-05-26 | auth-team | ✓ argon2 dominates |
| auth-demo | `POST /auth/logout` | 100 ms | 18 ms | 2026-05-26 | auth-team | ✓ |
| auth-demo | `POST /auth/refresh` | 100 ms | 9 ms | 2026-05-26 | auth-team | ✓ |
| auth-demo | `GET /me` | 100 ms | 42 ms | 2026-05-26 | auth-team | ✓ |
| webhook-receiver | `POST /webhooks/stripe` | 100 ms | 18 ms | 2026-05-26 | billing | ✓ |

## Web routes (SvelteKit)

| Route | TTFB budget | Last TTFB | Last verified | Owner | Status |
|---|---|---|---|---|---|
| `/` | 200 ms | 110 ms | 2026-05-26 | web-team | ✓ |
| `/login` | 200 ms | 95 ms | 2026-05-26 | web-team | ✓ |
| `/notes` | 300 ms | 180 ms | 2026-05-26 | web-team | ✓ |

## Cross-system flows

| Flow | Total budget | Owner | Status |
|---|---|---|---|
| Signup → first dashboard render | 2.5 s | full-stack | ✓ |
| Upgrade click → Stripe Checkout loaded | 2.0 s | billing | ✓ |
| Login (web) → /notes loaded | 1.5 s | full-stack | ✓ |

## How to use this table

1. **For a new endpoint PR.** Add a row with a proposed budget,
   justify it in the PR description, and run a load test (`oha` for
   a single route; `k6` for a flow). Attach the report.
2. **When you see a row turn red.** Open a ticket within 24 hours. The
   team prioritizes it the next business day.
3. **Quarterly review.** Re-run the load tests; bump *last verified*;
   document any trends in `docs/perf/<service>-Q<n>.md`.
4. **When deprecating an endpoint.** Remove the row in the same PR.

## Related

- Phase 11 lessons (`curriculum/phase-11-perf-caching-jobs/`)
- ADR 0008 — Stripe is the rail (sets the constraint on Stripe
  Checkout's portion of the upgrade flow)
- `docs/runbooks/5xx-spike.md` — what to do when a route blows budget
  in production
