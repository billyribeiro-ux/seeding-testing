# Phase 4 — Rubric

| Dimension | Beginner (1) | Competent (3) | Senior (5) |
|---|---|---|---|
| **Routing** | Single flat router | Nested versioned router with `/v1` prefix | Designs API surface up front; documents deprecation policy |
| **Extractors** | Reads `req` and parses by hand | Uses built-in `Path`/`Query`/`Json` cleanly | Writes custom extractors for cross-cutting concerns (auth, tenancy) |
| **State sharing** | Globals or copies | `State<Arc<AppState>>`, internally `Clone`-cheap | Designs `AppState` so handlers depend on the minimum subset they need |
| **Error mapping** | Returns plain text or stack traces | Implements `IntoResponse` for a typed error | Maps to RFC 7807 problem-details; logs server errors verbosely, sanitizes wire body |
| **Middleware** | None | Stacks tracing, request-id, cors, compression | Designs layer order; understands tower-http timeouts, rate limits, retries |
| **Validation** | None or panic on bad input | Inline checks + DB CHECK constraints | Uses `validator` (or equivalent), returns field-level errors in body |
| **OpenAPI / docs** | No docs | utoipa annotations on every endpoint | CI gate for spec drift; generated TS client; Swagger UI live |
| **Pagination** | Offset everywhere | `?limit=N` and keyset where it matters | Opaque cursor encoding, indexed sort key, hard ceiling on `limit` |
| **Tests** | curl-by-hand | `oneshot` integration tests against the Router | testcontainers Postgres + snapshot tests + spec-drift check |

## Self-check before moving to Phase 5

- [ ] `make verify` passes locally.
- [ ] `cargo run -p notes-api` boots; curl walks every endpoint successfully.
- [ ] You completed Exercises E4.1 – E4.6 (E4.7 and E4.8 are stretch).
- [ ] You can read the whole `src/lib.rs` of `notes-api` and explain every line.
- [ ] You can articulate the difference between `State<T>` and `Extension<T>`.
- [ ] You can pick a status code from the 5-most-common list for any given failure.
- [ ] CI is green on your branch.

Phase 5 — Testing + Seeding — turns this skeleton into something we ship: high coverage, factories, seed CLI, snapshots, properties.
