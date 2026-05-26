//! Integration tests for the Phase 4 — E4.5 keyset pagination on
//! `GET /v1/notes`.
//!
//! What we prove:
//!
//!   1. With 10 rows and `?limit=3`, the first page returns the
//!      newest-three plus a `next` cursor; subsequent pages walk the
//!      list in id-descending order, and the final page returns
//!      `next = null`.
//!   2. Glueing the items from every page back together yields the
//!      same set as a single, no-limit query — no row is dropped
//!      or duplicated.
//!   3. A `cursor=` query parameter that isn't valid base64url JSON
//!      maps to `400 Bad Request` with a problem-details body.
//!   4. `limit` is clamped to `[1, 100]`: `?limit=0` is treated as 1,
//!      `?limit=10000` is treated as 100. No 500s, no broken contract.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use notes_api::{AppState, router};

async fn app() -> axum::Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx_notes::migrate(&pool).await.unwrap();
    router(AppState::new(pool))
}

async fn read_json(b: Body) -> Value {
    let bytes = to_bytes(b, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Insert `n` notes and return the order of ids that the API will
/// produce when paginating newest-first.
async fn seed(app: &axum::Router, n: usize) -> Vec<i64> {
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/notes")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "body": format!("note-{i}") }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = read_json(res.into_body()).await;
        ids.push(body["id"].as_i64().unwrap());
    }
    // Newest-first is the API's order — reverse the insert order.
    ids.reverse();
    ids
}

async fn fetch_page(app: &axum::Router, limit: u32, cursor: Option<&str>) -> Value {
    let uri = match cursor {
        Some(c) => format!("/v1/notes?limit={limit}&cursor={c}"),
        None => format!("/v1/notes?limit={limit}"),
    };
    let res = app
        .clone()
        .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "GET {uri} should be 200");
    read_json(res.into_body()).await
}

#[tokio::test]
async fn walks_in_id_descending_order_across_pages() {
    let app = app().await;
    let expected_order = seed(&app, 10).await;

    // First page (3 items).
    let p1 = fetch_page(&app, 3, None).await;
    let items: Vec<i64> = p1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_i64().unwrap())
        .collect();
    assert_eq!(items, expected_order[0..3]);

    let next = p1["next"]
        .as_str()
        .expect("3 < 10 — next cursor must be present");

    // Second page.
    let p2 = fetch_page(&app, 3, Some(next)).await;
    let items: Vec<i64> = p2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_i64().unwrap())
        .collect();
    assert_eq!(items, expected_order[3..6]);

    // Third page.
    let next = p2["next"].as_str().unwrap();
    let p3 = fetch_page(&app, 3, Some(next)).await;
    let items: Vec<i64> = p3["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_i64().unwrap())
        .collect();
    assert_eq!(items, expected_order[6..9]);

    // Fourth (last) page — only one item left and `next` is null.
    let next = p3["next"].as_str().unwrap();
    let p4 = fetch_page(&app, 3, Some(next)).await;
    let items: Vec<i64> = p4["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_i64().unwrap())
        .collect();
    assert_eq!(items, expected_order[9..10]);
    assert!(
        p4["next"].is_null(),
        "the final page must signal end-of-list with next = null"
    );
}

#[tokio::test]
async fn no_row_is_dropped_or_duplicated() {
    // Walk every page and confirm the union of items equals the full
    // seeded set (the keyset cursor invariant).
    let app = app().await;
    let expected: Vec<i64> = seed(&app, 17).await;

    let mut collected: Vec<i64> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let page = fetch_page(&app, 5, cursor.as_deref()).await;
        for item in page["items"].as_array().unwrap() {
            collected.push(item["id"].as_i64().unwrap());
        }
        match page["next"].as_str() {
            Some(c) => cursor = Some(c.to_string()),
            None => break,
        }
    }

    assert_eq!(
        collected, expected,
        "paginated walk must reconstruct the full ordered list exactly once"
    );
}

#[tokio::test]
async fn malformed_cursor_is_400_with_problem_details() {
    let app = app().await;
    let _ = seed(&app, 3).await;

    let res = app
        .clone()
        .oneshot(
            Request::get("/v1/notes?cursor=this-is-not-a-real-cursor")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Note: `this-is-not-a-real-cursor` contains hyphens, which ARE
    // valid base64url chars, so the base64 decoder may accept it and
    // hand serde_json garbage to parse. Either way: 400.
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], 400);
    assert!(
        body["type"]
            .as_str()
            .unwrap()
            .contains("/problems/bad-cursor"),
        "problem type must specifically name bad-cursor, got {}",
        body["type"]
    );
}

#[tokio::test]
async fn limit_clamps_to_1_at_the_low_end() {
    let app = app().await;
    let _ = seed(&app, 5).await;

    // limit=0 → clamped to 1 (the smallest legal page size).
    let page = fetch_page(&app, 0, None).await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert!(
        page["next"].as_str().is_some(),
        "clamped limit still yields a next cursor"
    );
}

#[tokio::test]
async fn limit_clamps_to_100_at_the_high_end() {
    let app = app().await;
    let _ = seed(&app, 5).await;

    // limit=10000 → clamped to 100. Only 5 rows exist, so we get 5 +
    // null next.
    let page = fetch_page(&app, 10_000, None).await;
    assert_eq!(page["items"].as_array().unwrap().len(), 5);
    assert!(page["next"].is_null());
}
