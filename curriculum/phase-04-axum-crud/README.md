# Phase 4 — Axum CRUD

> **Audience:** you finished Phase 3.
> **Outcome:** you can build a tested, observable, lint-clean Axum service with CRUD endpoints, validation, OpenAPI docs, problem-details errors, and middleware.
> **Time:** 2 weeks.

## The mental model

> *A web framework is just a function from `Request` to `Response`, plus a router.*

Axum makes that literal. A handler is a function. A router is a tree of routes. Middleware (called "layers" in Axum) are *functions of functions*. Everything composes.

Three habits we'll cement in this phase:

1. **Logic in a library, plumbing in a binary.** Same split as `hello-cli` and `quote-generator`. The `notes` business logic moves out of the data layer (`sqlx-notes`) into a service crate; the HTTP layer is the thinnest possible adapter.
2. **Errors map to HTTP at the boundary.** Internal code returns `Result<T, NotesError>`. An `IntoResponse` impl translates each variant into a `StatusCode` + a problem-details JSON body (RFC 7807).
3. **Every public endpoint is documented in OpenAPI** — generated from the code via `utoipa`, served at `/openapi.json` and a Swagger UI at `/docs`.

## The phase plan

| Lesson | Topic |
|---|---|
| `lessons/01-axum-mental-model.md` | Router, handlers, extractors, `IntoResponse` |
| `lessons/02-state-and-extension.md` | Sharing the pool, the config, the tracing span |
| `lessons/03-routes-and-versioning.md` | `/v1/...` prefix, nested routers, `.merge`, `.nest` |
| `lessons/04-extractors-and-validation.md` | Path/Query/Json, `validator` crate, custom extractors |
| `lessons/05-errors-as-responses.md` | Problem-details (RFC 7807), `thiserror` → `IntoResponse` |
| `lessons/06-middleware-layers.md` | tower-http (trace, cors, compression, request-id, timeout) |
| `lessons/07-openapi-with-utoipa.md` | Annotations, generated schema, Swagger UI |
| `lessons/08-pagination-and-keyset.md` | Offset is a lie; keyset paging is the answer |
| `lessons/09-build-notes-api.md` | Capstone walkthrough |

## The capstone — `projects/03-notes-api`

A real Axum service that wraps the `sqlx-notes` library with:

- `GET    /v1/notes`            — list (keyset paginated, sorted, filterable)
- `POST   /v1/notes`            — create
- `GET    /v1/notes/:id`        — single
- `PATCH  /v1/notes/:id`        — update
- `DELETE /v1/notes/:id`        — delete
- `GET    /healthz`             — health probe
- `GET    /openapi.json` + `/docs` — OpenAPI 3.1 + Swagger UI

Backed by **SQLite** through `sqlx-notes` (so the project builds without Docker), with the migration plan for Postgres documented inline. Integration tests via `axum::Router::oneshot` (no real HTTP listener; pure in-process).

## Green-bar checkpoint

```bash
cargo test  -p notes-api
cargo build -p notes-api
cargo run   -p notes-api      # listens on 127.0.0.1:3000
curl http://localhost:3000/healthz
curl http://localhost:3000/v1/notes
curl -X POST http://localhost:3000/v1/notes \
     -H 'content-type: application/json' \
     -d '{"body":"hello from curl"}'
curl http://localhost:3000/openapi.json | jq '.info'
```

## What's next

Phase 5 — **Testing + Seeding** — turns this skeleton into something we ship with confidence: 90%+ coverage, factories, seed CLI, snapshot tests, property tests.
