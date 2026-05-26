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
