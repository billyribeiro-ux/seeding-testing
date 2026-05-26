# Lesson 4.7 — OpenAPI with `utoipa`

> **Concept first:** the OpenAPI spec *is* your public API documentation. Generate it from code so it can't drift.
> **Time:** 20 minutes.

## Why we generate, not write

A hand-written OpenAPI spec rots inside two weeks. Generated specs:

- Update on every commit.
- Cover every route by definition (you can't forget to document one).
- Power client-SDK generators (TypeScript via `openapi-typescript`, Go via `oapi-codegen`, etc.).
- Are the source of truth for `/docs` UIs.

The two Rust libraries in 2026:

- **`utoipa`** — annotation-driven, generates a static `openapi.json` from `#[utoipa::path]` and `#[derive(ToSchema)]` macros. Mature, integrates with Axum cleanly.
- **`apistos`** — newer, builder-driven, requires no macros. Less idiomatic but supports OpenAPI 3.1 more cleanly.

We use **utoipa**.

## The shape

```rust
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

#[derive(ToSchema, Serialize)]
struct NoteDto { id: i64, body: String, created_at: String }

#[utoipa::path(
    get, path = "/v1/notes",
    responses(
        (status = 200, description = "All notes, newest first", body = Vec<NoteDto>),
        (status = 500, description = "Server error", body = ProblemDetails),
    ),
)]
async fn list_notes(...) -> ... { ... }

#[derive(OpenApi)]
#[openapi(
    paths(list_notes, create_note, get_note, update_note, delete_note),
    components(schemas(NoteDto, CreateBody, UpdateBody, ProblemDetails)),
    info(title = "notes-api", version = env!("CARGO_PKG_VERSION")),
)]
struct ApiDoc;

let app = Router::new()
    .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
    .route("/v1/notes", get(list_notes))
    // ...
```

What you get:

- `GET /openapi.json` returns the spec.
- `GET /docs` serves Swagger UI rendered against the spec.

## Why this isn't in `notes-api` yet

The Phase 4 capstone keeps utoipa as a *stretch* — see `EXERCISES.md` E4.7. The patterns above are exactly what you'll add.

The reason: utoipa annotations are noisy in a tutorial. We learn Axum's routing and error model first; *then* we layer documentation on top in an exercise.

## Workflow once it's added

```bash
# Generate the spec at build time (commit it for diff visibility)
cargo run -p notes-api --bin export-openapi -- > openapi.json
# Generate a TS client
pnpm dlx openapi-typescript openapi.json --output apps/web/src/lib/api/schema.d.ts
```

A CI check (`scripts/check-openapi.sh`) compares generated spec to committed spec; drift fails the build. That's how you keep client SDKs in sync.

## Documentation hygiene

- **Every endpoint has at least one `(status = 4xx, body = ProblemDetails)` response.** Clients can deserialize errors uniformly.
- **Every public DTO has `description = "..."` on each field.** It shows up in the UI tooltip.
- **Use `nullable = true`** for fields that can be `null`. Without it, generated clients won't accept nulls.

## Why this matters

- **Generated specs eliminate the #1 source of API friction** — out-of-date docs.
- **Client SDKs become a function of the spec.** No more "the iOS team is on the wrong version of `User`."
- **The `/docs` page is your *living* changelog for partners.** They reload it whenever they're confused.

## Green-bar checkpoint

- You can annotate a handler with `#[utoipa::path]` and a DTO with `#[derive(ToSchema)]`.
- You can mount Swagger UI at `/docs`.
- You can explain why a CI check for spec drift is worth the 30 lines of shell.

Next: `lessons/08-pagination-and-keyset.md`.
