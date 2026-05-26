//! Notes domain — DB ops + Axum handlers wired with policy gating.
//!
//! Each protected endpoint:
//!   1. Loads the note from the DB (404 if missing).
//!   2. Calls the matching `can_*` policy.
//!   3. On denial: writes an `audit_logs` row + returns 403 problem-details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::auth::{AuthenticatedUser, audit};
use crate::policy::{self, Forbidden};
use crate::{ApiError, AppState};

// ---------------------------------------------------------------------------
// Domain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Note {
    pub id: i64,
    pub owner_id: i64,
    pub org_id: i64,
    pub body: String,
    pub published_at: Option<String>,
    pub min_tier: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteDto {
    pub id: i64,
    pub owner_id: i64,
    pub org_id: i64,
    pub body: String,
    pub published_at: Option<String>,
    pub min_tier: String,
    pub created_at: String,
}

impl From<Note> for NoteDto {
    fn from(n: Note) -> Self {
        Self {
            id: n.id,
            owner_id: n.owner_id,
            org_id: n.org_id,
            body: n.body,
            published_at: n.published_at,
            min_tier: n.min_tier,
            created_at: n.created_at,
        }
    }
}

const NOTE_COLUMNS: &str = "id, owner_id, org_id, body, published_at, min_tier, created_at";

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notes", get(list_notes).post(create_note))
        .route(
            "/notes/{id}",
            get(get_note).patch(update_note).delete(delete_note),
        )
}

// ---------------------------------------------------------------------------
// Bodies
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub body: String,
    #[serde(default)]
    pub min_tier: Option<String>,
    #[serde(default)]
    pub published: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub min_tier: Option<String>,
    #[serde(default)]
    pub published: Option<bool>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[tracing::instrument(skip(s, user), fields(user_id = user.0.id))]
async fn list_notes(
    State(s): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<NoteDto>>, ApiError> {
    let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE owner_id = ? ORDER BY id DESC");
    let rows = sqlx::query_as::<_, Note>(&sql)
        .bind(user.0.id)
        .fetch_all(&s.pool)
        .await?;
    Ok(Json(rows.into_iter().map(NoteDto::from).collect()))
}

#[tracing::instrument(skip(s, user, body), fields(user_id = user.0.id, body_len = body.body.len()))]
async fn create_note(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<NoteDto>), ApiError> {
    let trimmed = body.body.trim();
    if trimmed.is_empty() {
        return Err(ApiError::EmptyBody);
    }
    if trimmed.chars().count() > 4096 {
        return Err(ApiError::TooLong);
    }
    let min_tier = body.min_tier.as_deref().unwrap_or("free").to_string();
    if !matches!(min_tier.as_str(), "free" | "pro" | "elite") {
        return Err(ApiError::InvalidTier);
    }
    let published_at = if body.published.unwrap_or(false) {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    let sql = format!(
        "INSERT INTO notes (owner_id, org_id, body, min_tier, published_at) \
         VALUES (?, ?, ?, ?, ?) RETURNING {NOTE_COLUMNS}"
    );
    let note = sqlx::query_as::<_, Note>(&sql)
        .bind(user.0.id)
        .bind(user.0.org_id)
        .bind(trimmed)
        .bind(&min_tier)
        .bind(published_at)
        .fetch_one(&s.pool)
        .await?;

    audit(
        &s.pool,
        Some(user.0.id),
        "note.created",
        Some(&format!("note_id={}", note.id)),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(NoteDto::from(note))))
}

#[tracing::instrument(skip(s, user), fields(user_id = user.0.id, note_id = id))]
async fn get_note(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<Json<NoteDto>, ApiError> {
    let note = load_note(&s.pool, id).await?;
    match policy::can_read_doc(&user.0, &note) {
        Ok(()) => Ok(Json(NoteDto::from(note))),
        Err(reason) => {
            deny(&s.pool, user.0.id, "note.read", note.id, reason).await?;
            Err(ApiError::Forbidden(reason))
        }
    }
}

#[tracing::instrument(skip(s, user, body), fields(user_id = user.0.id, note_id = id))]
async fn update_note(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<NoteDto>, ApiError> {
    let mut note = load_note(&s.pool, id).await?;
    if let Err(reason) = policy::can_write_doc(&user.0, &note) {
        deny(&s.pool, user.0.id, "note.update", note.id, reason).await?;
        return Err(ApiError::Forbidden(reason));
    }

    if let Some(b) = body.body.as_deref() {
        let trimmed = b.trim();
        if trimmed.is_empty() {
            return Err(ApiError::EmptyBody);
        }
        if trimmed.chars().count() > 4096 {
            return Err(ApiError::TooLong);
        }
        note.body = trimmed.to_string();
    }
    if let Some(t) = body.min_tier {
        if !matches!(t.as_str(), "free" | "pro" | "elite") {
            return Err(ApiError::InvalidTier);
        }
        note.min_tier = t;
    }
    if let Some(p) = body.published {
        note.published_at = if p {
            Some(
                note.published_at
                    .clone()
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            )
        } else {
            None
        };
    }

    let sql = format!(
        "UPDATE notes SET body = ?, min_tier = ?, published_at = ? \
         WHERE id = ? RETURNING {NOTE_COLUMNS}"
    );
    let updated = sqlx::query_as::<_, Note>(&sql)
        .bind(&note.body)
        .bind(&note.min_tier)
        .bind(&note.published_at)
        .bind(note.id)
        .fetch_one(&s.pool)
        .await?;
    audit(
        &s.pool,
        Some(user.0.id),
        "note.updated",
        Some(&format!("note_id={}", updated.id)),
    )
    .await?;
    Ok(Json(NoteDto::from(updated)))
}

#[tracing::instrument(skip(s, user), fields(user_id = user.0.id, note_id = id))]
async fn delete_note(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let note = load_note(&s.pool, id).await?;
    if let Err(reason) = policy::can_delete_doc(&user.0, &note) {
        deny(&s.pool, user.0.id, "note.delete", note.id, reason).await?;
        return Err(ApiError::Forbidden(reason));
    }
    sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(note.id)
        .execute(&s.pool)
        .await?;
    audit(
        &s.pool,
        Some(user.0.id),
        "note.deleted",
        Some(&format!("note_id={}", note.id)),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn load_note(pool: &SqlitePool, id: i64) -> Result<Note, ApiError> {
    let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?");
    let note = sqlx::query_as::<_, Note>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    note.ok_or(ApiError::NotFound)
}

async fn deny(
    pool: &SqlitePool,
    actor_id: i64,
    action: &str,
    note_id: i64,
    reason: Forbidden,
) -> Result<(), ApiError> {
    tracing::warn!(
        actor = actor_id,
        %action,
        note_id,
        reason = %reason,
        "policy denial"
    );
    audit(
        pool,
        Some(actor_id),
        &format!("denied.{action}"),
        Some(&format!("note_id={note_id} reason={reason}")),
    )
    .await
}
