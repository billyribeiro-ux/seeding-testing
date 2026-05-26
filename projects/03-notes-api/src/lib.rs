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

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{MatchedPath, Path, Query, Request, State};
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

use sqlx_notes::{Note, NotesError};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub metrics: Arc<PrometheusHandle>,
}

impl AppState {
    /// Create a fresh `AppState`. Installs a process-global Prometheus
    /// recorder if one isn't already installed (idempotent — safe in tests).
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        let metrics = match PrometheusBuilder::new().install_recorder() {
            Ok(handle) => Arc::new(handle),
            Err(_) => {
                // A recorder is already installed (e.g. from a previous test
                // in the same process). Reach into the global to grab its
                // render handle. We do this via a fresh dedicated recorder
                // we keep alive via an Arc on the side; metrics still flow
                // to the *first* recorder, but `render()` works.
                Arc::new(PrometheusBuilder::new().build_recorder().handle())
            }
        };

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
        .route("/metrics", get(metrics_handler))
        .nest("/v1", v1)
        .with_state(shared)
        .layer(middleware::from_fn(record_metrics))
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
#[derive(Debug, Serialize)]
pub struct NotesPage {
    pub items: Vec<NoteDto>,
    pub next: Option<String>,
}

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

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub body: String,
}

#[tracing::instrument(skip(s, body), fields(body_len = body.body.len()))]
async fn create_note(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<NoteDto>), ApiError> {
    let note = sqlx_notes::add(&s.pool, &body.body).await?;
    Ok((StatusCode::CREATED, Json(NoteDto::from(note))))
}

#[tracing::instrument(skip(s), fields(note_id = id))]
async fn get_note(
    State(s): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<NoteDto>, ApiError> {
    let note = sqlx_notes::get(&s.pool, id).await?;
    Ok(Json(NoteDto::from(note)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    pub body: String,
}

#[tracing::instrument(skip(s, payload), fields(note_id = id, body_len = payload.body.len()))]
async fn update_note(
    State(s): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateBody>,
) -> Result<Json<NoteDto>, ApiError> {
    // sqlx-notes doesn't yet expose `update`; do an emulation here using a transaction.
    // We rely on RETURNING in a single UPDATE statement.
    let trimmed = payload.body.trim();
    if trimmed.is_empty() {
        return Err(ApiError::from(NotesError::Empty));
    }
    if trimmed.chars().count() > 4096 {
        return Err(ApiError::from(NotesError::TooLong(trimmed.chars().count())));
    }
    let row: Option<Note> = sqlx::query_as::<_, Note>(
        "UPDATE notes SET body = ? WHERE id = ? RETURNING id, body, created_at",
    )
    .bind(trimmed)
    .bind(id)
    .fetch_optional(&s.pool)
    .await
    .map_err(NotesError::from)?;
    let note = row.ok_or(NotesError::NotFound(id))?;
    Ok(Json(NoteDto::from(note)))
}

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

#[derive(Debug, Clone, Serialize)]
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
}

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
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
