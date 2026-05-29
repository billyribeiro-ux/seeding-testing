# Lesson 4.9 — Build `notes-api` Step by Step

> **The capstone of Phase 4.** Walk through `projects/03-notes-api/` line by line.
> **Time:** 60 minutes.

## What we're building

An Axum service that wraps the Phase 3 `sqlx-notes` data layer with a real HTTP surface:

- `GET    /healthz`
- `GET    /v1/notes?limit=N`
- `POST   /v1/notes`
- `GET    /v1/notes/{id}`
- `PATCH  /v1/notes/{id}`
- `DELETE /v1/notes/{id}`

Every error is `application/problem+json`. Every request has a request id. Every span carries it.

## Step 0 — Browse

```bash
cd projects/03-notes-api
ls -R src tests
```

```
src/lib.rs       src/main.rs       tests/api.rs
```

Three files. The library *is* the service; `main.rs` only boots it.

## Step 1 — The manifest

```toml
[dependencies]
axum       = { workspace = true }
tower-http = { workspace = true }
serde      = { workspace = true }
serde_json = { workspace = true }
sqlx       = { workspace = true, features = [..., "sqlite", ...] }
sqlx-notes = { path = "../02c-sqlx-notes" }
# ...
```

The notable line: `sqlx-notes = { path = "../02c-sqlx-notes" }` — we depend on the Phase 3 data layer as a *workspace* crate. No code duplication. Change the schema in one place, rebuild here.

## Step 2 — The router

```rust
pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/notes",      get(list_notes).post(create_note))
        .route("/notes/{id}", get(get_note).patch(update_note).delete(delete_note));

    Router::new()
        .route("/healthz", get(health))
        .nest("/v1", v1)
        .with_state(Arc::new(state))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http().make_span_with(/* see lib.rs */))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));
}
```

Read it top-down:

1. Build `v1` as a child router. Inside `v1` we write `/notes`; from outside the world sees `/v1/notes`.
2. Mount `v1` under the root router at `/v1`.
3. Attach the shared state.
4. Layer the middleware. Reading the chain bottom-up tells you what runs *first on the way in*.

## Step 3 — A handler in full

```rust
async fn create_note(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<NoteDto>), ApiError> {
    let note = sqlx_notes::add(&s.pool, &body.body).await?;
    Ok((StatusCode::CREATED, Json(NoteDto::from(note))))
}
```

Four lines, four lessons:

- **`State<Arc<AppState>>`** — auto-injected by Axum.
- **`Json<CreateBody>`** — Axum parses the body into our struct before calling the handler.
- **`sqlx_notes::add` returns `Result<Note, NotesError>`** — `?` propagates errors via `ApiError`'s `#[from]` impl.
- **`(StatusCode::CREATED, Json(...))`** — a tuple is `IntoResponse`: first element sets the status, second is the body.

## Step 4 — Error mapping

```rust
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, kind, detail) = match &self {
            ApiError::Notes(NotesError::Empty | NotesError::TooLong(_)) => (
                StatusCode::BAD_REQUEST, "Bad Request",
                "https://memberclub.test/problems/invalid-input", self.to_string()
            ),
            ApiError::Notes(NotesError::NotFound(_)) => (
                StatusCode::NOT_FOUND, "Not Found",
                "https://memberclub.test/problems/not-found", self.to_string()
            ),
            ApiError::Notes(NotesError::Db(_)) => (
                StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error",
                "https://memberclub.test/problems/internal", "database error".into()
            ),
            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST, "Bad Request",
                "https://memberclub.test/problems/bad-request", msg.clone()
            ),
        };

        if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "request failed");
        } else {
            tracing::warn!(error = %self, status = %status, "client error");
        }

        let body = ProblemDetails { kind, title, status: status.as_u16(), detail };
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}
```

This is the heart of the boundary. Internal errors → HTTP responses with three guarantees:

1. The `type` URI is stable.
2. 5xx errors are logged with detail; the wire body says `"database error"` — no leaks.
3. The content type is `application/problem+json`.

## Step 5 — The binary

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(...).init();

    let pool = SqlitePoolOptions::new()
        .max_connections(/* 1 for :memory:, 8 otherwise */)
        .connect(&database_url).await?;
    sqlx_notes::migrate(&pool).await?;

    let app = router(AppState::new(pool));
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
```

`with_graceful_shutdown(shutdown_signal())` waits for ctrl-C *or* SIGTERM (so `docker stop` shuts us down cleanly).

## Step 6 — Integration tests

```rust
async fn app() -> axum::Router {
    let pool = SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    sqlx_notes::migrate(&pool).await.unwrap();
    router(AppState::new(pool))
}

#[tokio::test]
async fn create_then_list() {
    let app = app().await;
    let res = app.clone().oneshot(json_request("POST", "/v1/notes", &json!({"body":"hello"}))).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    // ...
}
```

`tower::ServiceExt::oneshot` calls the `Router` *directly* — no TCP listener, no port collisions. Tests run in parallel safely. We could replace SQLite with testcontainers' Postgres without changing the test shape at all.

## Step 7 — Verify

```bash
cargo test  -p notes-api               # all green
cargo clippy -p notes-api -- -D warnings
cargo fmt    -p notes-api -- --check
cargo run   -p notes-api               # listens on 127.0.0.1:3000

curl -s http://localhost:3000/healthz
# {"status":"ok","version":"0.1.0"}
curl -s -X POST http://localhost:3000/v1/notes -H 'content-type: application/json' -d '{"body":"hi"}'
# {"id":1,"body":"hi","created_at":"..."}
curl -s -i http://localhost:3000/v1/notes/999
# HTTP/1.1 404 Not Found
# content-type: application/problem+json
# {"type":"...","title":"Not Found","status":404,"detail":"note 999 not found"}
```

## Why this matters

- **A thin HTTP layer over a typed library is the cleanest enterprise pattern.** When you want to ship a CLI, a worker, or a different transport (gRPC?), you re-use the library.
- **`oneshot` integration tests are gold-standard.** Fast, deterministic, exercise the real router + middleware.
- **Problem-details at the boundary is what professional APIs do.** Stripe, GitHub, modern Microsoft — all converged on this.

## Green-bar checkpoint

- `cargo test -p notes-api` is green (the `tests/api.rs` integration suite plus the unit and snapshot tests).
- You can curl every endpoint and see the documented behaviour.
- You can explain why the library is reused as a workspace dep instead of copy-pasted.

Phase 4 is complete. Phase 5 — **Testing + Seeding** — turns this skeleton into a service we'd ship.
