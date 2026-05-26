# Phase 9 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Mental model** | Confused about server vs client | Knows what runs where; uses runes appropriately | Designs new routes by *placing files*; rendering modes a deliberate choice |
| **Runes** | `$state` everywhere, including for derivable values | Uses `$state`, `$derived`, `$effect`, `$props` correctly | Reaches for `$effect` only as escape hatch; avoids cross-state syncing |
| **Routing** | Flat `routes/` | Nested layouts; dynamic params validated | Uses route groups `(group)` for shared layouts; named param validation |
| **Load functions** | Inline DB calls in components | Pure server-only `+page.server.ts` `load`s | Streaming with `Promise` returns; `depends()`/`invalidate()` for revalidation |
| **Form actions** | None — uses `fetch` from script | Named actions; `fail`/`error`/`redirect` distinction | Optimistic UI via `use:enhance`; idempotent server logic |
| **Server-only modules** | Mixed | `$lib/server/` for DB and secrets | Compiler-enforced fence; `PUBLIC_*` discipline; clean boundary tests |
| **Hooks + session auth** | Per-route cookie checks | `hooks.server.ts` populates `event.locals.user` | Multi-hook composition via `sequence`; tested revoke flow |
| **Type safety** | `any` in `App.Locals` | App-level types declared; `svelte-check` clean | Generated route types via `$types`; zero warnings in CI |
| **Testing** | Manual clicking | Vitest for logic + svelte-check for types | + Playwright e2e for golden paths; snapshot tests where helpful |
| **Progressive enhancement** | Requires JS to use the app | Default actions work without JS | `use:enhance` extends UX; accessibility a first-class concern |

## Self-check before moving to Phase 10

- [ ] `make verify` passes locally (Rust workspace).
- [ ] `cd apps/memberclub/web && pnpm test && pnpm check && pnpm build`
      all green.
- [ ] You can sign up, log in, create + delete notes, log out, *all
      with JavaScript disabled* in your browser.
- [ ] You completed Exercises E9.1 – E9.4.
- [ ] You can articulate the request lifecycle end-to-end on a whiteboard.
- [ ] You can pick between `$state`, `$derived`, and `$effect` for a given
      need without hesitation.
- [ ] CI is green on your branch.

Phase 10 — **Observability** — instruments every Rust service and the
SvelteKit web app: structured logs, OpenTelemetry traces, Prometheus
metrics, Grafana dashboards, alerting.
