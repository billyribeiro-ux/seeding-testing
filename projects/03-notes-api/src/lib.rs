//! notes-api — an Axum service wrapping the `sqlx-notes` data layer.
//!
//! Architecture:
//!   `axum::Router` → handlers (this crate) → `sqlx-notes` (data layer) → `SQLite`
//!
//! The HTTP layer is deliberately thin: parse, call the lib, map errors to
//! problem-details. Business logic lives in `sqlx-notes`.
//!
//! ## Observability
//!
//! Each handler is `#[tracing::instrument]`-ed; spans carry the request id.
//! Every request increments `http_requests_total{route,method,status_class}`
//! and records into `http_request_duration_seconds_bucket{route,method}`.
//! Metrics are exposed at `GET /metrics` in the Prometheus exposition format.

pub mod seed;
pub mod telemetry;

use std::sync::Arc;
use std::time::Instant;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, MatchedPath, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::{Span, info_span};
use utoipa::OpenApi;

use sqlx_notes::{Note, NotesError};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub metrics: Arc<PrometheusHandle>,
}

/// Process-global Prometheus render handle.
///
/// The `metrics` crate has exactly one process-wide recorder. The first
/// `AppState::new` installs it and keeps the handle that renders it; every
/// later `AppState` must reuse *that same* handle. If each `AppState` built
/// its own handle, only the first would point at the recorder that the
/// `record_metrics` middleware actually writes into — so a second `AppState`
/// (e.g. a later test in the same `cargo test` process) would render an empty
/// registry. Caching the handle in a `OnceLock` makes `/metrics` deterministic
/// under both `cargo test` (one process, many tests) and `cargo nextest`
/// (process per test).
static METRICS_HANDLE: std::sync::OnceLock<PrometheusHandle> = std::sync::OnceLock::new();

impl AppState {
    /// Create a fresh `AppState`, reusing the process-global Prometheus
    /// recorder (installed exactly once — safe to call repeatedly in tests).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        let handle = METRICS_HANDLE.get_or_init(|| {
            match PrometheusBuilder::new().install_recorder() {
                Ok(handle) => handle,
                // Another component already owns the global recorder; fall
                // back to a local one so `render()` still works. (No metrics
                // flow into it, but this path only happens if something other
                // than `AppState` installed a recorder first.)
                Err(_) => PrometheusBuilder::new().build_recorder().handle(),
            }
        });
        let metrics = Arc::new(handle.clone());

        metrics::describe_counter!(
            "http_requests_total",
            "All HTTP requests, by route + method + status class"
        );
        metrics::describe_histogram!(
            "http_request_duration_seconds",
            "Request latency, by route + method"
        );

        Self { pool, metrics }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the complete `Router` for the service, including v1 routes,
/// health probe, `/metrics`, and middleware layers.
pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/notes", get(list_notes).post(create_note))
        .route(
            "/notes/{id}",
            get(get_note).patch(update_note).delete(delete_note),
        );

    let shared = Arc::new(state);

    Router::new()
        .route("/healthz", get(health))
        .route("/version", get(version))
        .route("/metrics", get(metrics_handler))
        .route("/openapi.json", get(openapi_json))
        .nest("/v1", v1)
        .with_state(shared)
        .layer(middleware::from_fn(inject_request_id_into_problem_details))
        .layer(middleware::from_fn(record_metrics))
        // Phase 4 — E4.6: kill any handler that doesn't respond within
        // 5 s. The tower-http layer returns 408 Request Timeout. We
        // place it BELOW the metrics middleware so the timed-out call
        // still records into our counter.
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(5),
        ))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                let req_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("-");
                info_span!("http", method=%req.method(), uri=%req.uri(), req_id = %req_id)
            }),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

// ---------------------------------------------------------------------------
// Metrics middleware + endpoint
// ---------------------------------------------------------------------------

async fn metrics_handler(State(s): State<Arc<AppState>>) -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        s.metrics.render(),
    )
}

/// Record `http_requests_total` and `http_request_duration_seconds` for every
/// non-`/metrics` request. The label set is intentionally bounded:
/// Phase 4 — E4.4. After a handler runs, if the response is an RFC 7807
/// problem-details body, splice the request's `x-request-id` into the
/// JSON as a top-level field. The whole point: when a customer reports
/// "I got a 500 around 10:42", they can paste back the `request_id`
/// from the problem-details body and we can pivot straight to the
/// trace.
///
/// We only touch responses where the upstream `SetRequestIdLayer` has
/// already provided a request id (always, in practice) AND the
/// content-type begins with `application/problem+json`. Everything
/// else is forwarded byte-for-byte.
async fn inject_request_id_into_problem_details(req: Request, next: Next) -> Response {
    let req_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let response = next.run(req).await;

    let Some(req_id) = req_id else {
        return response;
    };
    let is_problem = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/problem+json"));
    if !is_problem {
        return response;
    }

    let (parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, 64 * 1024).await else {
        // Body too large to buffer or stream error — forward an empty
        // body rather than panic. In practice problem-details bodies
        // are <1 KiB; this branch is a safety hatch.
        return Response::from_parts(parts, axum::body::Body::empty());
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        // The body claimed application/problem+json but isn't valid
        // JSON — pass it through unchanged.
        return Response::from_parts(parts, axum::body::Body::from(bytes));
    };
    json["request_id"] = serde_json::Value::String(req_id);
    let new_bytes = serde_json::to_vec(&json).unwrap_or_default();
    Response::from_parts(parts, axum::body::Body::from(new_bytes))
}

/// `route` is the *matched-path template* (`/v1/notes/{id}`, not the
/// concrete URI); `method` is the HTTP verb; `status_class` is one of
/// `1xx`/`2xx`/`3xx`/`4xx`/`5xx`. Three labels with small cardinality —
/// safe for Prometheus.
async fn record_metrics(req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_string(), |p| p.as_str().to_string());

    // Skip /metrics itself to avoid self-amplification in the histogram.
    if route == "/metrics" {
        return next.run(req).await;
    }

    let method = req.method().clone();
    let started = Instant::now();
    let res = next.run(req).await;
    let elapsed = started.elapsed().as_secs_f64();
    let status_class = status_class_of(res.status());

    metrics::counter!(
        "http_requests_total",
        "route" => route.clone(),
        "method" => method.to_string(),
        "status_class" => status_class.to_string(),
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "route" => route,
        "method" => method.to_string(),
    )
    .record(elapsed);

    res
}

fn status_class_of(status: StatusCode) -> &'static str {
    match status.as_u16() / 100 {
        1 => "1xx",
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /version` — Phase 4 E4.1. Reports the crate version and the
/// short git SHA that the binary was built from. Stays useful in
/// incident timelines ("we deployed `<sha>` at 14:02; the 5xx spike
/// started at 14:06") long after the rest of the running fleet has
/// rolled forward.
#[derive(Serialize)]
struct Version {
    version: &'static str,
    git_sha: &'static str,
}

async fn version() -> Json<Version> {
    Json(Version {
        version: env!("CARGO_PKG_VERSION"),
        git_sha: env!("GIT_SHA"),
    })
}

/// Phase 4 — E4.7. Compile-time `OpenAPI` 3 document built by `utoipa`
/// from the `#[utoipa::path]` annotations on each handler plus the
/// `#[derive(ToSchema)]` on each DTO. A snapshot test pins the JSON so
/// any spec drift fails CI.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "notes-api",
        version = env!("CARGO_PKG_VERSION"),
        description = "Phase 4 capstone — Axum CRUD over the sqlx-notes data layer.",
        license(name = "MIT OR Apache-2.0"),
    ),
    paths(
        list_notes,
        create_note,
        get_note,
        update_note,
        delete_note,
    ),
    components(schemas(
        NoteDto,
        NotesPage,
        CreateBody,
        UpdateBody,
        ProblemDetails,
    )),
    tags(
        (name = "notes", description = "CRUD over the notes resource."),
    ),
)]
pub struct ApiDoc;

/// `GET /openapi.json` — serves the document. utoipa's JSON
/// serialization is deterministic, so the snapshot test in
/// `tests/openapi.rs` can pin it.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Pagination query for `GET /v1/notes`.
///
/// Phase 4 — E4.5 (keyset pagination):
///   * `limit` clamps to `[1, 100]` and defaults to `20`.
///   * `cursor` is an opaque base64url-encoded JSON value; the only
///     way to obtain one is to read `next` from a previous response.
///     The client never has to know its shape.
#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    /// Last (smallest) id from the previous page. Newest-first means
    /// "next" page is rows with `id < i`.
    i: i64,
}

fn encode_cursor(c: &Cursor) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(c).expect("Cursor must serialize"))
}

fn decode_cursor(s: &str) -> Result<Cursor, ApiError> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s.as_bytes())
        .map_err(|_| ApiError::BadCursor)?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::BadCursor)
}

/// One page of notes plus the opaque cursor a client uses to fetch the
/// next page. `next` is `None` when the caller has reached the end.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NotesPage {
    pub items: Vec<NoteDto>,
    pub next: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v1/notes",
    tag = "notes",
    params(
        ("limit"  = Option<u32>, Query, description = "page size, clamped to [1, 100], default 20"),
        ("cursor" = Option<String>, Query, description = "opaque cursor from a previous `next`"),
    ),
    responses(
        (status = 200, description = "one page of notes", body = NotesPage),
        (status = 400, description = "cursor was malformed", body = ProblemDetails),
    ),
)]
#[tracing::instrument(skip(s), fields(limit = ?q.limit, has_cursor = q.cursor.is_some()))]
async fn list_notes(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<NotesPage>, ApiError> {
    let limit = i64::from(q.limit.unwrap_or(20).clamp(1, 100));
    let after_id = match q.cursor.as_deref() {
        Some(s) => Some(decode_cursor(s)?.i),
        None => None,
    };

    // Ask the lib for one MORE than we'll return so we can tell whether
    // there's a next page without a second query.
    let mut items = sqlx_notes::list_keyset(&s.pool, after_id, limit + 1).await?;

    // `limit` is clamped to `[1, 100]` above so the casts are safe.
    let limit_usize = usize::try_from(limit).unwrap_or(0);
    let next = if items.len() > limit_usize {
        items.truncate(limit_usize);
        items
            .last()
            .map(|last| encode_cursor(&Cursor { i: last.id }))
    } else {
        None
    };

    Ok(Json(NotesPage {
        items: items.into_iter().map(NoteDto::from).collect(),
        next,
    }))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[schema(example = json!({"body": "remember the milk"}))]
pub struct CreateBody {
    pub body: String,
}

#[utoipa::path(
    post,
    path = "/v1/notes",
    tag = "notes",
    request_body = CreateBody,
    responses(
        (status = 201, description = "the note was created", body = NoteDto),
        (status = 400, description = "body is empty or too long", body = ProblemDetails),
        (status = 413, description = "request body exceeded 8 KiB", body = ProblemDetails),
    ),
)]
#[tracing::instrument(skip(s, body), fields(body_len = body.body.len()))]
async fn create_note(
    State(s): State<Arc<AppState>>,
    JsonBody(body): JsonBody<CreateBody>,
) -> Result<(StatusCode, Json<NoteDto>), ApiError> {
    if body.body.len() > MAX_BODY_BYTES {
        return Err(ApiError::PayloadTooLarge(body.body.len()));
    }
    let note = sqlx_notes::add(&s.pool, &body.body).await?;
    Ok((StatusCode::CREATED, Json(NoteDto::from(note))))
}

#[utoipa::path(
    get,
    path = "/v1/notes/{id}",
    tag = "notes",
    params(("id" = i64, Path, description = "id of the note")),
    responses(
        (status = 200, description = "the note", body = NoteDto),
        (status = 404, description = "no such id", body = ProblemDetails),
    ),
)]
#[tracing::instrument(skip(s), fields(note_id = id))]
async fn get_note(
    State(s): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<NoteDto>, ApiError> {
    let note = sqlx_notes::get(&s.pool, id).await?;
    Ok(Json(NoteDto::from(note)))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateBody {
    pub body: String,
}

#[utoipa::path(
    patch,
    path = "/v1/notes/{id}",
    tag = "notes",
    params(("id" = i64, Path, description = "id of the note")),
    request_body = UpdateBody,
    responses(
        (status = 200, description = "the updated note", body = NoteDto),
        (status = 400, description = "body invalid", body = ProblemDetails),
        (status = 404, description = "no such id", body = ProblemDetails),
        (status = 413, description = "request body exceeded 8 KiB", body = ProblemDetails),
    ),
)]
#[tracing::instrument(skip(s, payload), fields(note_id = id, body_len = payload.body.len()))]
async fn update_note(
    State(s): State<Arc<AppState>>,
    Path(id): Path<i64>,
    JsonBody(payload): JsonBody<UpdateBody>,
) -> Result<Json<NoteDto>, ApiError> {
    if payload.body.len() > MAX_BODY_BYTES {
        return Err(ApiError::PayloadTooLarge(payload.body.len()));
    }
    // E4.3 — body validation + UPDATE…RETURNING moved into the lib so
    // the handler stays one line per concern.
    let note = sqlx_notes::update(&s.pool, id, &payload.body).await?;
    Ok(Json(NoteDto::from(note)))
}

#[utoipa::path(
    delete,
    path = "/v1/notes/{id}",
    tag = "notes",
    params(("id" = i64, Path, description = "id of the note")),
    responses(
        (status = 204, description = "the note was deleted"),
        (status = 404, description = "no such id", body = ProblemDetails),
    ),
)]
#[tracing::instrument(skip(s), fields(note_id = id))]
async fn delete_note(
    State(s): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx_notes::delete(&s.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[schema(example = json!({"id": 42, "body": "remember the milk", "created_at": "2026-05-26T10:00:00Z"}))]
pub struct NoteDto {
    pub id: i64,
    pub body: String,
    pub created_at: String,
}

impl From<Note> for NoteDto {
    fn from(n: Note) -> Self {
        Self {
            id: n.id,
            body: n.body,
            created_at: n.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors mapped to RFC 7807 problem-details
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Notes(#[from] NotesError),

    #[error("invalid request body")]
    BadRequest(String),

    #[error("invalid cursor")]
    BadCursor,

    /// Phase 4 — E4.2. Rejected at the handler layer before we ever
    /// hand the body to the lib. 8 KiB is plenty for notes; tightening
    /// here means clippy doesn't have to scan a 50 MiB POST.
    #[error("payload too large: {0} bytes (max {MAX_BODY_BYTES})")]
    PayloadTooLarge(usize),

    /// Phase 4 — E4.8. The client sent something the JSON parser
    /// (or one of its content-type / body checks) refused. We carry
    /// the underlying reason so the problem-details body can tell
    /// the developer what went wrong, but with a stable status (400)
    /// instead of axum's default plain-text 415/422.
    #[error("invalid JSON: {0}")]
    BadJson(String),
}

/// Phase 4 — E4.8. A drop-in for `axum::Json<T>` that converts every
/// extractor rejection into our `ApiError::BadJson(_)` so the response
/// is the same RFC 7807 problem-details shape as every other error in
/// this service. Without this wrapper, axum returns a plain-text body
/// for malformed JSON, missing content-type, etc. — inconsistent with
/// the contract.
pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(JsonBody(v)),
            Err(JsonRejection::JsonDataError(e)) => Err(ApiError::BadJson(e.body_text())),
            Err(JsonRejection::JsonSyntaxError(e)) => Err(ApiError::BadJson(e.body_text())),
            Err(JsonRejection::MissingJsonContentType(e)) => Err(ApiError::BadJson(e.body_text())),
            Err(JsonRejection::BytesRejection(e)) => Err(ApiError::BadJson(e.body_text())),
            Err(rej) => Err(ApiError::BadJson(rej.body_text())),
        }
    }
}

/// Max accepted size of a notes body (Phase 4 — E4.2).
pub const MAX_BODY_BYTES: usize = 8 * 1024;

/// RFC 7807 problem-details — the wire shape every error path returns.
/// Made `pub` so utoipa can reference it from the `OpenAPI` doc.
#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({
    "type": "https://memberclub.test/problems/not-found",
    "title": "Not Found",
    "status": 404,
    "detail": "note 999 not found",
    "request_id": "01HXYZ…"
}))]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    #[schema(rename = "type")]
    pub kind: &'static str,
    pub title: &'static str,
    pub status: u16,
    pub detail: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, title, kind, detail): (StatusCode, &'static str, &'static str, String) =
            match &self {
                ApiError::Notes(NotesError::Empty | NotesError::TooLong(_)) => (
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "https://memberclub.test/problems/invalid-input",
                    self.to_string(),
                ),
                ApiError::Notes(NotesError::NotFound(_)) => (
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    "https://memberclub.test/problems/not-found",
                    self.to_string(),
                ),
                ApiError::Notes(NotesError::Db(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    "https://memberclub.test/problems/internal",
                    "database error".to_string(),
                ),
                ApiError::BadRequest(msg) => (
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "https://memberclub.test/problems/bad-request",
                    msg.clone(),
                ),
                ApiError::BadCursor => (
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "https://memberclub.test/problems/bad-cursor",
                    "cursor is malformed or expired; refetch the first page".to_string(),
                ),
                ApiError::PayloadTooLarge(_) => (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Payload Too Large",
                    "https://memberclub.test/problems/payload-too-large",
                    self.to_string(),
                ),
                ApiError::BadJson(_) => (
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "https://memberclub.test/problems/bad-json",
                    self.to_string(),
                ),
            };

        if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "request failed");
        } else {
            tracing::warn!(error = %self, status = %status, "client error");
        }

        let body = ProblemDetails {
            kind,
            title,
            status: status.as_u16(),
            detail,
        };
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

// Convenience for "I'd like to log/propagate the span"
#[allow(dead_code)]
fn current_span() -> Span {
    Span::current()
}

// ---------------------------------------------------------------------------
// Cursor encoding property tests (Phase 5 — E5.7)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod cursor_proptest {
    use super::{Cursor, decode_cursor, encode_cursor};
    use proptest::prelude::*;

    proptest! {
        /// The whole "opaque cursor" contract: a value goes in, the same
        /// value comes out. For every legal `i64`, encode then decode
        /// must reconstruct the original `i` exactly.
        #[test]
        fn encode_decode_round_trip(i in any::<i64>()) {
            let token = encode_cursor(&Cursor { i });
            let decoded = decode_cursor(&token).expect("encode output must decode");
            prop_assert_eq!(decoded.i, i);
        }

        /// Encoded cursors are base64url-no-pad — URL-safe and
        /// shell-safe (no `+`, `/`, `=`).
        #[test]
        fn encoded_token_is_url_safe(i in any::<i64>()) {
            let token = encode_cursor(&Cursor { i });
            for ch in token.chars() {
                prop_assert!(
                    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                    "cursor `{token}` contains non-url-safe char `{ch}`"
                );
            }
        }

        /// Random arbitrary strings (not produced by `encode_cursor`)
        /// must NOT successfully decode in ways that produce confusable
        /// id values. We assert that decode either fails cleanly OR
        /// returns a value — never panics, never overflows.
        #[test]
        fn decode_never_panics_on_arbitrary_input(s in ".{0,128}") {
            // The contract here is "no panic" — we don't care which
            // branch fires.
            let _ = decode_cursor(&s);
        }
    }
}
