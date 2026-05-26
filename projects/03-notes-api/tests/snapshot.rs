//! Snapshot tests for the problem-details wire format.
//!
//! When the JSON shape changes intentionally, run `cargo insta accept` to
//! re-bless. Drift without intent will fail CI.

use axum::body::{Body, to_bytes};
use axum::http::Request;
use serde_json::Value;
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

async fn body_json(b: Body) -> Value {
    let bytes = to_bytes(b, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// `request_id` is a per-request UUID (Phase 4 — E4.4) — non-deterministic,
/// so we strip it from the snapshot but assert separately that it was
/// present and non-empty. The "every problem-details body carries one"
/// contract stays under test.
fn assert_request_id_then_strip(body: &mut Value) {
    let req_id = body["request_id"]
        .as_str()
        .expect("problem-details body must carry request_id (E4.4)");
    assert!(!req_id.is_empty(), "request_id must not be empty");
    body.as_object_mut()
        .expect("problem-details body must be a JSON object")
        .remove("request_id");
}

#[tokio::test]
async fn problem_details_404() {
    let res = app()
        .await
        .oneshot(Request::get("/v1/notes/999").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let mut body = body_json(res.into_body()).await;
    assert_request_id_then_strip(&mut body);
    insta::assert_json_snapshot!(body);
}

#[tokio::test]
async fn problem_details_400_empty_body() {
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"body":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let mut body = body_json(res.into_body()).await;
    assert_request_id_then_strip(&mut body);
    insta::assert_json_snapshot!(body);
}
