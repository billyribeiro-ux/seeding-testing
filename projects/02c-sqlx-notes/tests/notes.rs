//! Integration tests for sqlx-notes.
//!
//! Uses an in-memory `SQLite` database per test so every test is hermetic.
//! Same shape we'll use against Postgres via testcontainers in Phase 4.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx_notes::{
    NotesError, add, delete, get, list, list_keyset, migrate, parse_created_at, update,
};

async fn fresh_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1) // ":memory:" is per-connection; one connection = one DB
        .connect("sqlite::memory:")
        .await
        .expect("connect to in-memory sqlite");
    migrate(&pool).await.expect("apply migrations");
    pool
}

#[tokio::test]
async fn list_is_empty_initially() {
    let pool = fresh_pool().await;
    let items = list(&pool).await.unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn add_persists_a_note() {
    let pool = fresh_pool().await;
    let created = add(&pool, "hello world").await.unwrap();
    assert_eq!(created.body, "hello world");

    let items = list(&pool).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, created.id);
}

#[tokio::test]
async fn list_is_newest_first() {
    let pool = fresh_pool().await;
    let a = add(&pool, "first").await.unwrap();
    let b = add(&pool, "second").await.unwrap();
    let items = list(&pool).await.unwrap();
    assert_eq!(
        items.iter().map(|n| n.id).collect::<Vec<_>>(),
        vec![b.id, a.id]
    );
}

#[tokio::test]
async fn add_trims_whitespace() {
    let pool = fresh_pool().await;
    let n = add(&pool, "   hi   ").await.unwrap();
    assert_eq!(n.body, "hi");
}

#[tokio::test]
async fn add_rejects_empty() {
    let pool = fresh_pool().await;
    let err = add(&pool, "    ").await.unwrap_err();
    assert!(matches!(err, NotesError::Empty));
}

#[tokio::test]
async fn add_rejects_too_long() {
    let pool = fresh_pool().await;
    let long: String = "a".repeat(4097);
    let err = add(&pool, &long).await.unwrap_err();
    assert!(matches!(err, NotesError::TooLong(4097)));
}

#[tokio::test]
async fn delete_removes_a_row() {
    let pool = fresh_pool().await;
    let n = add(&pool, "gone").await.unwrap();
    delete(&pool, n.id).await.unwrap();
    assert!(list(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn delete_unknown_id_returns_not_found() {
    let pool = fresh_pool().await;
    let err = delete(&pool, 999).await.unwrap_err();
    assert!(matches!(err, NotesError::NotFound(999)));
}

#[tokio::test]
async fn get_returns_the_row() {
    let pool = fresh_pool().await;
    let n = add(&pool, "find me").await.unwrap();
    let same = get(&pool, n.id).await.unwrap();
    assert_eq!(same, n);
}

#[tokio::test]
async fn get_unknown_id_is_not_found() {
    let pool = fresh_pool().await;
    let err = get(&pool, 1).await.unwrap_err();
    assert!(matches!(err, NotesError::NotFound(1)));
}

#[tokio::test]
async fn created_at_is_parseable_iso8601() {
    let pool = fresh_pool().await;
    let n = add(&pool, "stamped").await.unwrap();
    assert!(
        parse_created_at(&n.created_at).is_some(),
        "got {}",
        n.created_at
    );
}

#[tokio::test]
async fn check_constraint_blocks_empty_body_at_db_layer() {
    // Sanity check that the CHECK constraint in the migration is real.
    let pool = fresh_pool().await;
    let res = sqlx::query("INSERT INTO notes (body) VALUES ('')")
        .execute(&pool)
        .await;
    assert!(res.is_err(), "DB should refuse the empty body");
}

// ---------------------------------------------------------------------------
// list_keyset — Phase 4 E4.5 keyset pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_keyset_with_no_cursor_returns_newest_first_up_to_limit() {
    let pool = fresh_pool().await;
    let a = add(&pool, "first").await.unwrap();
    let b = add(&pool, "second").await.unwrap();
    let c = add(&pool, "third").await.unwrap();

    let page = list_keyset(&pool, None, 2).await.unwrap();
    // Newest-first: c, b.
    assert_eq!(
        page.iter().map(|n| n.id).collect::<Vec<_>>(),
        vec![c.id, b.id]
    );
    let _ = a; // suppress unused warning if any
}

#[tokio::test]
async fn list_keyset_with_cursor_skips_to_the_window() {
    let pool = fresh_pool().await;
    let a = add(&pool, "first").await.unwrap();
    let b = add(&pool, "second").await.unwrap();
    let c = add(&pool, "third").await.unwrap();

    // "Next page after seeing c.id" — should return b then a.
    let page = list_keyset(&pool, Some(c.id), 10).await.unwrap();
    assert_eq!(
        page.iter().map(|n| n.id).collect::<Vec<_>>(),
        vec![b.id, a.id]
    );
}

// ---------------------------------------------------------------------------
// update — Phase 4 E4.3
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_swaps_the_body() {
    let pool = fresh_pool().await;
    let n = add(&pool, "before").await.unwrap();
    let updated = update(&pool, n.id, "after").await.unwrap();
    assert_eq!(updated.id, n.id);
    assert_eq!(updated.body, "after");
}

#[tokio::test]
async fn update_rejects_empty_body() {
    let pool = fresh_pool().await;
    let n = add(&pool, "starting body").await.unwrap();
    let err = update(&pool, n.id, "   ").await.unwrap_err();
    assert!(matches!(err, NotesError::Empty));
    // Original row unchanged.
    let still_there = get(&pool, n.id).await.unwrap();
    assert_eq!(still_there.body, "starting body");
}

#[tokio::test]
async fn update_rejects_too_long_body() {
    let pool = fresh_pool().await;
    let n = add(&pool, "ok").await.unwrap();
    let long: String = "a".repeat(4097);
    let err = update(&pool, n.id, &long).await.unwrap_err();
    assert!(matches!(err, NotesError::TooLong(4097)));
}

#[tokio::test]
async fn update_unknown_id_returns_not_found() {
    let pool = fresh_pool().await;
    let err = update(&pool, 9999, "doesn't matter").await.unwrap_err();
    assert!(matches!(err, NotesError::NotFound(9999)));
}

#[tokio::test]
async fn list_keyset_at_end_of_list_returns_empty() {
    let pool = fresh_pool().await;
    let a = add(&pool, "first").await.unwrap();
    // Asking for rows older than the only row yields nothing.
    let page = list_keyset(&pool, Some(a.id), 5).await.unwrap();
    assert!(page.is_empty(), "no rows beyond the oldest");
}
