# Phase 4 — Exercises

Eight drills for `projects/03-notes-api`.

---

## E4.1 — Add a `GET /version` route (Easy) — shipped

Add a new route that returns `{"version": env!("CARGO_PKG_VERSION"), "git_sha": <build-time SHA>}`. Make sure it's covered by an integration test.

Reference implementation: `projects/03-notes-api/build.rs` stamps the
short `git rev-parse --short HEAD` into `GIT_SHA` (falls back to `dev`
for vendored/published builds); `projects/03-notes-api/src/lib.rs`
exposes `GET /version`; covered by
`projects/03-notes-api/tests/router_extras.rs::version_route_returns_crate_version_and_git_sha`.

<details><summary>Answer (sketch)</summary>

```rust
async fn version() -> Json<serde_json::Value> {
    let sha = option_env!("GIT_SHA").unwrap_or("dev");
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "git_sha": sha }))
}

// In router(): .route("/version", get(version))
```

Use `build.rs` to set `GIT_SHA` from `git rev-parse --short HEAD`.
</details>

---

## E4.2 — Validate `body` length at the handler level (Easy) — shipped

Add a check in `create_note` and `update_note` *before* the lib call: reject any body longer than 8 KB with `413 Payload Too Large`.

Reference implementation: `MAX_BODY_BYTES = 8 * 1024` constant +
`ApiError::PayloadTooLarge(usize)` variant + handler guards in
`projects/03-notes-api/src/lib.rs`. Two integration tests in
`projects/03-notes-api/tests/router_extras.rs` cover the oversize-rejected
case AND the boundary (exactly `MAX_BODY_BYTES` does NOT trigger 413,
landing instead in the sqlx-notes char-count validation).

<details><summary>Answer</summary>

Add a variant to `ApiError`:

```rust
#[error("payload too large: {0} bytes (max 8192)")]
PayloadTooLarge(usize),
```

In `IntoResponse`, map it to `StatusCode::PAYLOAD_TOO_LARGE`. In the handler:

```rust
if body.body.len() > 8192 { return Err(ApiError::PayloadTooLarge(body.body.len())); }
```

Add an integration test that sends a 9000-byte payload and asserts the 413.
</details>

---

## E4.3 — Add the `update` function to `sqlx-notes` (Medium) — shipped

Move the `UPDATE` SQL out of `notes-api::update_note` into `sqlx_notes::update`. Match the existing API style; add a unit test in `sqlx-notes/tests/notes.rs`.

Reference implementation: `projects/02c-sqlx-notes/src/lib.rs::update` takes
the same `(pool, id, body)` shape and enforces the same trim + non-empty +
4096-char rules as `add`. The notes-api handler collapses to a single line:
`sqlx_notes::update(&s.pool, id, &payload.body).await.map(NoteDto::from)`.
4 unit tests in `projects/02c-sqlx-notes/tests/notes.rs` cover happy path,
empty-body reject (with confirmation the original row is unchanged),
too-long reject, and unknown id.

<details><summary>Answer (sketch)</summary>

```rust
// sqlx-notes/src/lib.rs
pub async fn update(pool: &SqlitePool, id: i64, body: &str) -> NotesResult<Note> {
    let trimmed = body.trim();
    if trimmed.is_empty() { return Err(NotesError::Empty); }
    if trimmed.chars().count() > 4096 { return Err(NotesError::TooLong(trimmed.chars().count())); }
    let row: Option<Note> = sqlx::query_as::<_, Note>(
        "UPDATE notes SET body = ? WHERE id = ? RETURNING id, body, created_at",
    ).bind(trimmed).bind(id).fetch_optional(pool).await?;
    row.ok_or(NotesError::NotFound(id))
}
```

Then in `notes-api` reduce `update_note` to `sqlx_notes::update(&s.pool, id, &payload.body).await.map(NoteDto::from)`.
</details>

---

## E4.4 — Echo the request id in problem-details (Medium) — shipped

When an error response is generated, include the `x-request-id` from the request as a top-level `request_id` field in the problem-details body. Hint: implement a custom extractor for `RequestId` or extract from `req.headers()` in the layer.

Reference implementation: `inject_request_id_into_problem_details` middleware
in `projects/03-notes-api/src/lib.rs`. Runs AFTER the handler; reads
`x-request-id` from the request, then — and only if the response carries
`application/problem+json` — buffers the body, parses it as JSON, splices
`request_id` as a top-level field, and re-serializes. Successful responses
go through untouched (proven by a test). The pattern is `axum::middleware::from_fn`
so it composes with the rest of the layer stack.

<details><summary>Answer (sketch)</summary>

Add a `RequestId(String)` value to request extensions inside `SetRequestIdLayer` (already in place). Then thread it via `Extension<RequestId>` into the handler and pass to `ApiError::with_request_id(...)`.

Simpler: add a `from_fn` middleware that runs *after* the handler and, if the response is an error, mutates the JSON body to add `request_id`. The cleanest factoring depends on taste.
</details>

---

## E4.5 — Keyset pagination (Medium) — shipped

Implement keyset pagination for `GET /v1/notes`:

- New response shape: `{ "items": [...], "next": "..."|null }`.
- New query params: `?limit=N&cursor=...`.
- Cursor is base64-encoded JSON of `{ "i": <last id> }` (id-descending ordering is enough since `id` is monotonic on insert).
- Add at least two integration tests: first page + follow-up page.

A reference implementation lives in `projects/02c-sqlx-notes/src/lib.rs`
(`list_keyset`) and the `list_notes` handler in
`projects/03-notes-api/src/lib.rs`. Eight tests cover the contract:
three unit tests in `projects/02c-sqlx-notes/tests/notes.rs` (no cursor,
mid-walk cursor, end-of-list cursor) plus five integration tests in
`projects/03-notes-api/tests/keyset_pagination.rs` (full multi-page walk
in id-descending order, "no row dropped or duplicated" invariant over
17 rows × `limit=5`, malformed-cursor → 400 problem-details with the
specific `/problems/bad-cursor` type, and `limit` clamping at both
ends `[1, 100]`).

<details><summary>Answer (sketch)</summary>

```rust
#[derive(Serialize, Deserialize)]
struct Cursor { i: i64 }

fn encode(c: &Cursor) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(c).unwrap())
}
fn decode(s: &str) -> Result<Cursor, ApiError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)
        .map_err(|_| ApiError::BadRequest("bad cursor".into()))?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::BadRequest("bad cursor".into()))
}

async fn list_notes(State(s): ..., Query(q): Query<ListQuery>) -> Result<Json<Page>, ApiError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100) as i64;
    let after = q.cursor.as_deref().map(decode).transpose()?;
    let sql = match after {
        Some(c) => format!("SELECT id, body, created_at FROM notes WHERE id < {} ORDER BY id DESC LIMIT ?", c.i),
        None    => "SELECT id, body, created_at FROM notes ORDER BY id DESC LIMIT ?".into(),
    };
    let items: Vec<Note> = sqlx::query_as(&sql).bind(limit + 1).fetch_all(&s.pool).await?;
    let (page, next) = if items.len() as i64 > limit {
        let last = &items[limit as usize - 1];
        (items.into_iter().take(limit as usize).collect::<Vec<_>>(), Some(encode(&Cursor { i: last.id })))
    } else { (items, None) };
    Ok(Json(Page { items: page.into_iter().map(NoteDto::from).collect(), next }))
}
```
</details>

---

## E4.6 — Per-request timeout (Medium) — shipped

Add a 5-second `TimeoutLayer` to the router. Add an integration test that simulates a slow handler (`tokio::time::sleep(6s)`) and verifies the client receives a `503 Service Unavailable` (or `408`, depending on tower-http version).

Reference implementation: `tower_http::timeout::TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(5))`
wired into `router()` in `projects/03-notes-api/src/lib.rs` (placed
below the metrics middleware so a timed-out request still records a
counter — observability for slow paths). Test in
`projects/03-notes-api/tests/router_extras.rs::timeout_layer_returns_408_when_handler_runs_past_budget`
uses a 100 ms budget so CI doesn't wait the full 5 s.

<details><summary>Answer</summary>

```rust
.layer(tower_http::timeout::TimeoutLayer::new(Duration::from_secs(5)))
```

Test:

```rust
async fn slow() { tokio::time::sleep(Duration::from_secs(6)).await; }
let app = Router::new().route("/slow", get(slow)).layer(TimeoutLayer::new(Duration::from_secs(1)));
let res = app.oneshot(Request::get("/slow").body(Body::empty()).unwrap()).await.unwrap();
assert!(res.status().is_server_error() || res.status() == StatusCode::REQUEST_TIMEOUT);
```
</details>

---

## E4.7 — Add OpenAPI via utoipa (Stretch)

Add `utoipa` annotations to every handler in `notes-api`. Serve the spec at `/openapi.json` and Swagger UI at `/docs`. Add a snapshot test that asserts `openapi.json` matches a committed file (so spec drift fails CI).

<details><summary>Answer (sketch)</summary>

See Lesson 4.7 for the full pattern. Add deps:

```toml
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-axum = "0.2"
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

Annotate each handler with `#[utoipa::path(...)]`. Build `ApiDoc` and mount the SwaggerUi router.

For the snapshot test, use `insta::assert_json_snapshot!` against `ApiDoc::openapi()`.
</details>

---

## E4.8 — Replace `Json` rejection with problem-details (Stretch) — shipped

When a client sends malformed JSON, Axum returns plain text. Override it: when `Json<T>` extraction fails, return a `400` problem-details with a useful message. Hint: write a custom `Json<T>` extractor that wraps the built-in and remaps rejections.

Reference implementation: `JsonBody<T>` extractor in
`projects/03-notes-api/src/lib.rs` — wraps `axum::Json::<T>::from_request`
and matches each `JsonRejection` variant (JsonDataError, JsonSyntaxError,
MissingJsonContentType, BytesRejection, …) into `ApiError::BadJson(msg)`.
Routes swap `Json<T>` for `JsonBody<T>` and inherit the consistent
problem-details shape. Two new tests in `tests/router_extras.rs` cover
malformed JSON syntax → 400 + /problems/bad-json AND missing content-type
header → same 400 + /problems/bad-json (previously a 415 plain-text).

<details><summary>Answer (sketch)</summary>

```rust
pub struct AppJson<T>(pub T);
impl<S, T> FromRequest<S> for AppJson<T>
where S: Send + Sync, T: DeserializeOwned,
{
    type Rejection = ApiError;
    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, _state).await {
            Ok(axum::Json(v)) => Ok(Self(v)),
            Err(rej) => Err(ApiError::BadRequest(rej.body_text())),
        }
    }
}
```

Then replace every handler signature's `Json<T>` with `AppJson<T>`.
</details>
