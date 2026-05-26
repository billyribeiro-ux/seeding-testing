//! sqlx-notes — the notes domain re-implemented with sqlx + `SQLite`.
//!
//! This is deliberately the same domain as `projects/02b-sqlite-notes-svelte/` so
//! a learner can compare ORM-style (Drizzle) with hand-written SQL (sqlx) line by line.
//!
//! The library is pure logic over a `SqlitePool`. There is no binary; the tests
//! drive the library against an in-memory database, which is the same pattern we'll
//! use against Postgres in Phase 4 (via testcontainers).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub body: String,
    /// Stored as ISO-8601 text; `SQLite` has no native timestamp type.
    pub created_at: String,
}

#[derive(Debug, Error)]
pub enum NotesError {
    #[error("body cannot be empty")]
    Empty,

    #[error("body cannot exceed 4096 characters (got {0})")]
    TooLong(usize),

    #[error("note {0} not found")]
    NotFound(i64),

    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub type NotesResult<T> = Result<T, NotesError>;

/// Apply pending migrations to the given pool. Idempotent.
pub async fn migrate(pool: &SqlitePool) -> NotesResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| NotesError::Db(sqlx::Error::Migrate(Box::new(e))))
}

/// List all notes, newest first.
pub async fn list(pool: &SqlitePool) -> NotesResult<Vec<Note>> {
    let rows = sqlx::query_as::<_, Note>("SELECT id, body, created_at FROM notes ORDER BY id DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Keyset-paginated list (Phase 4 — E4.5).
///
/// Returns up to `limit` notes with `id < after_id` (when `after_id` is
/// `Some`), newest first. The caller asks for `limit + 1` if it wants
/// to detect whether there's a next page — see how `notes-api` uses
/// this in its `list_notes` handler.
///
/// Why keyset and not OFFSET? OFFSET re-scans rows up to the offset on
/// every request and falls over once you're 100k rows deep. Keyset
/// reads at most `limit` rows via an index, period. The trade-off:
/// you can't jump to a numbered page directly — only "next" / "prev"
/// — which is fine for any UX that isn't a 1990s search-results page.
pub async fn list_keyset(
    pool: &SqlitePool,
    after_id: Option<i64>,
    limit: i64,
) -> NotesResult<Vec<Note>> {
    let rows = match after_id {
        Some(after) => {
            sqlx::query_as::<_, Note>(
                "SELECT id, body, created_at FROM notes
                 WHERE id < ?
                 ORDER BY id DESC
                 LIMIT ?",
            )
            .bind(after)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, Note>(
                "SELECT id, body, created_at FROM notes ORDER BY id DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows)
}

/// Add a new note. Returns the created row.
pub async fn add(pool: &SqlitePool, body: &str) -> NotesResult<Note> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(NotesError::Empty);
    }
    if trimmed.chars().count() > 4096 {
        return Err(NotesError::TooLong(trimmed.chars().count()));
    }

    let row = sqlx::query_as::<_, Note>(
        "INSERT INTO notes (body) VALUES (?) RETURNING id, body, created_at",
    )
    .bind(trimmed)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Update a note's body (Phase 4 — E4.3). Same validation rules as
/// [`add`]: trims whitespace, rejects empty bodies and bodies longer
/// than 4096 chars. Returns the updated row.
pub async fn update(pool: &SqlitePool, id: i64, body: &str) -> NotesResult<Note> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(NotesError::Empty);
    }
    if trimmed.chars().count() > 4096 {
        return Err(NotesError::TooLong(trimmed.chars().count()));
    }
    let row: Option<Note> = sqlx::query_as::<_, Note>(
        "UPDATE notes SET body = ? WHERE id = ? RETURNING id, body, created_at",
    )
    .bind(trimmed)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    row.ok_or(NotesError::NotFound(id))
}

/// Delete a note by id. Returns `Ok(())` if a row was deleted; `Err(NotFound)` otherwise.
pub async fn delete(pool: &SqlitePool, id: i64) -> NotesResult<()> {
    let result = sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(NotesError::NotFound(id));
    }
    Ok(())
}

/// Get a single note by id.
pub async fn get(pool: &SqlitePool, id: i64) -> NotesResult<Note> {
    let row = sqlx::query_as::<_, Note>("SELECT id, body, created_at FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    row.ok_or(NotesError::NotFound(id))
}

/// Helper: parse a SQLite-stored ISO-8601 string into a `chrono::DateTime<Utc>`.
/// We keep this in the library so tests + callers don't reimplement it.
#[must_use]
pub fn parse_created_at(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
