# Phase 4 — Exercises

Eight drills for `projects/03-notes-api`.

---

## E4.1 — Add a `GET /version` route (Easy)

Add a new route that returns `{"version": env!("CARGO_PKG_VERSION"), "git_sha": <build-time SHA>}`. Make sure it's covered by an integration test.

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

## E4.2 — Validate `body` length at the handler level (Easy)

Add a check in `create_note` and `update_note` *before* the lib call: reject any body longer than 8 KB with `413 Payload Too Large`.

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

## E4.3 — Add the `update` function to `sqlx-notes` (Medium)

Move the `UPDATE` SQL out of `notes-api::update_note` into `sqlx_notes::update`. Match the existing API style; add a unit test in `sqlx-notes/tests/notes.rs`.

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

## E4.4 — Echo the request id in problem-details (Medium)

When an error response is generated, include the `x-request-id` from the request as a top-level `request_id` field in the problem-details body. Hint: implement a custom extractor for `RequestId` or extract from `req.headers()` in the layer.

<details><summary>Answer (sketch)</summary>

Add a `RequestId(String)` value to request extensions inside `SetRequestIdLayer` (already in place). Then thread it via `Extension<RequestId>` into the handler and pass to `ApiError::with_request_id(...)`.

Simpler: add a `from_fn` middleware that runs *after* the handler and, if the response is an error, mutates the JSON body to add `request_id`. The cleanest factoring depends on taste.
</details>

---

## E4.5 — Keyset pagination (Medium)

Implement keyset pagination for `GET /v1/notes`:

- New response shape: `{ "items": [...], "next": "..."|null }`.
- New query params: `?limit=N&cursor=...`.
- Cursor is base64-encoded JSON of `{ "i": <last id> }` (id-descending ordering is enough since `id` is monotonic on insert).
- Add at least two integration tests: first page + follow-up page.

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

## E4.6 — Per-request timeout (Medium)

Add a 5-second `TimeoutLayer` to the router. Add an integration test that simulates a slow handler (`tokio::time::sleep(6s)`) and verifies the client receives a `503 Service Unavailable` (or `408`, depending on tower-http version).

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

## E4.8 — Replace `Json` rejection with problem-details (Stretch)

When a client sends malformed JSON, Axum returns plain text. Override it: when `Json<T>` extraction fails, return a `400` problem-details with a useful message. Hint: write a custom `Json<T>` extractor that wraps the built-in and remaps rejections.

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
