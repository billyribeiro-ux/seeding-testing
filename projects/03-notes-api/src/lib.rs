//! notes-api — an Axum service wrapping the `sqlx-notes` data layer.
//!
//! Architecture:
//!   `axum::Router` → handlers (this crate) → `sqlx-notes` (data layer) → `SQLite`
//!
//! The HTTP layer is deliberately thin: parse, call the lib, map errors to
//! problem-details. Business logic lives in `sqlx-notes`.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
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
}

impl AppState {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the complete `Router` for the service, including v1 routes,
/// health probe, and middleware layers.
pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/notes", get(list_notes).post(create_note))
        .route(
            "/notes/{id}",
            get(get_note).patch(update_note).delete(delete_note),
        );

    Router::new()
        .route("/healthz", get(health))
        .nest("/v1", v1)
        .with_state(Arc::new(state))
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

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    limit: Option<u32>,
}

async fn list_notes(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<NoteDto>>, ApiError> {
    let mut items = sqlx_notes::list(&s.pool).await?;
    if let Some(limit) = q.limit {
        items.truncate(limit as usize);
    }
    Ok(Json(items.into_iter().map(NoteDto::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub body: String,
}

async fn create_note(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<NoteDto>), ApiError> {
    let note = sqlx_notes::add(&s.pool, &body.body).await?;
    Ok((StatusCode::CREATED, Json(NoteDto::from(note))))
}

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
