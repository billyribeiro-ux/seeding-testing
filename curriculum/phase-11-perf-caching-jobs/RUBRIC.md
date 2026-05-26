# Phase 11 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Measurement** | "I think it's faster" | Before/after numbers from `oha`/`k6` | Documented methodology + commit-ed perf report |
| **Profiling** | Doesn't profile | Uses `cargo flamegraph` or traces | Picks the right tool per question; reads tokio-console |
| **DB perf** | Adds queries without thought | Adds indexes; spots N+1 | Designs indexes from query needs; tunes pool with metrics |
| **Caching** | None or "let's cache everything" | Cache-aside with TTL | Picks TTL/delete-on-write/versioned per case; single-flight for hot keys |
| **Background jobs** | Side effects inline in handlers | Outbox table + worker | Idempotent dispatch; exponential backoff; FOR UPDATE SKIP LOCKED |
| **Multi-tenancy** | `WHERE org_id = $1` per query (forgets one) | App-layer policy checks | + Postgres RLS as belt-and-braces; audited cross-tenant endpoints |
| **Latency budgets** | None | Budget table per route | Enforced in PRs, observed in Grafana, retrospected quarterly |
| **Cost awareness** | "Just scale up" | Measures before scaling | Optimizes the hot 5%; leaves the rest alone |

## Self-check before moving to Phase 12

- [ ] You completed Exercises E11.1, E11.2, and E11.6.
- [ ] You can pick a tool (oha, k6, flamegraph, tokio-console, EXPLAIN
      ANALYZE) for a given perf question.
- [ ] You can sketch the outbox pattern.
- [ ] You can sketch a Postgres RLS policy.
- [ ] You can recite the latency order-of-magnitude table.
- [ ] You can author a perf report with the four required sections
      (methodology, command, before, after).

Phase 12 — **Principal Engineer Skills** — the non-code part of the
role: ADRs, RFCs, code reviews, incident response, mentoring.
