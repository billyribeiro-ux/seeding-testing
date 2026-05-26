# Lesson 10.2 — `tracing` in Rust

> **Concept first:** the `tracing` crate is the standard for Rust
> observability. Spans nest; events ride spans; subscribers fan output to
> stdout / OTLP / files / etc. We've already used it in every project —
> here's the deeper view.
> **Time:** 20 minutes.

## Three concepts

| Concept | What it is |
|---|---|
| **Span** | A unit of work with a start and end (a request, a DB query, a Stripe call). |
| **Event** | A point-in-time log message that belongs to the *current* span. |
| **Subscriber** | The pipeline that receives spans + events and writes them somewhere. |

In Axum + tower-http we get a span per request (the `TraceLayer` from
`tower-http`). Inside the handler, every `tracing::info!` becomes an event
on that span. When you call a helper function, instrument it with
`#[instrument]` to get a nested span.

## Setting up tracing

```rust
use tracing_subscriber::{EnvFilter, fmt};

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,notes_api=debug,tower_http=info")))
    .with_target(false)
    .compact()
    .init();
```

Five lines. Reads `RUST_LOG=...` from env. Default to info with overrides.

For JSON output (production):

```rust
.json()
.with_target(true)
.with_current_span(true)
.with_span_list(true)
.flatten_event(true)
.init();
```

JSON for log aggregation; compact text for local dev.

## `#[instrument]` on functions

```rust
#[tracing::instrument(skip(pool), fields(user_id))]
pub async fn find_user(pool: &PgPool, user_id: i64) -> Result<User, AppError> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id)
        .fetch_one(pool).await?;
    Ok(user)
}
```

Three pieces:

- **`skip(pool)`** — don't include the pool in the span's recorded fields.
- **`fields(user_id)`** — explicitly record `user_id` (already in scope).
- **Auto-renaming.** The span's name is the function name.

Every call to `find_user` now appears as a span — start time, end time,
duration, success/failure, fields.

## Structured fields, not f-strings

Wrong:

```rust
tracing::info!("user {} did action {}", user_id, action);
```

Right:

```rust
tracing::info!(user_id, %action, "user did action");
```

The second produces a structured event:

```json
{ "user_id": 42, "action": "doc.created", "message": "user did action", "level": "INFO", ... }
```

Searchable, indexable, alertable. The first is an opaque string.

The `%` prefix calls `Display`; `?` calls `Debug`.

## Levels

```
ERROR  the system is broken; humans need to know
WARN   something unusual; investigate during business hours
INFO   business events worth recording (login, payment, signup)
DEBUG  developer-grade detail; usually off in prod
TRACE  truly granular; rarely used
```

We default to `info`. Bump to `debug` for a specific module in dev with
`RUST_LOG=notes_api=debug`.

## A worked example — the `notes-api` request flow

```
GET /v1/notes/42

http (req_id=01HXY..., method=GET, uri=/v1/notes/42)  ← TraceLayer span
└── notes_api::list_notes                              ← #[instrument]
    └── sqlx::query_as                                 ← sqlx auto-span
        └── pg::execute                                ← pgwire span (in real Postgres)

duration: 12ms (1ms application, 11ms DB)
```

In a real tracing UI (Tempo, Jaeger), this renders as a flame graph. You
can *see* the DB call dominated the request.

## When to add fields

A field per call is roughly free if it's bounded cardinality (`status`,
`route`, `kind`). Per-user fields belong in events, not span attributes —
otherwise every span has unbounded fields.

## Why this matters

- **Spans nest naturally.** A handler span contains DB query spans
  contains TCP spans. The trace is a tree.
- **Structured events are searchable.** "Find all `INFO`s where
  `user_id == 42`" is one query in any log backend.
- **Instrumentation lives at the boundary.** Handlers + DB + outbound
  HTTP get instrumented; tiny helpers don't.

## Green-bar checkpoint

- You can read `#[instrument(skip(pool), fields(user_id))]` and explain it.
- You can rewrite an `info!("user {}", id)` as structured fields.
- You can name three things to log at `INFO`, three at `WARN`, three at
  `ERROR`.

Next: `lessons/03-opentelemetry-export.md`.
