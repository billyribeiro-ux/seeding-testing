//! Integration tests for notes-api.
//!
//! We construct the Router with a fresh in-memory `SQLite` pool per test and drive
//! it via `tower::ServiceExt::oneshot` — no real HTTP listener, no port binding.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
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

async fn read_json(b: Body) -> Value {
    let bytes = to_bytes(b, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn health_returns_ok() {
    let res = app()
        .await
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn list_initially_empty() {
    let res = app()
        .await
        .oneshot(Request::get("/v1/notes").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn create_then_list() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/notes",
            &serde_json::json!({ "body": "hello" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = read_json(res.into_body()).await;
    assert_eq!(created["body"], "hello");
    assert!(created["id"].is_number());

    let res = app
        .oneshot(Request::get("/v1/notes").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let list = read_json(res.into_body()).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn create_rejects_empty_body_with_problem_details() {
    let res = app()
        .await
        .oneshot(json_request(
            "POST",
            "/v1/notes",
            &serde_json::json!({ "body": "   " }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], 400);
    assert_eq!(body["title"], "Bad Request");
    assert!(body["detail"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
async fn get_unknown_id_returns_404_problem_details() {
    let res = app()
        .await
        .oneshot(Request::get("/v1/notes/999").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], 404);
    assert!(body["detail"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn patch_updates_a_note() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/notes",
            &serde_json::json!({ "body": "old" }),
        ))
        .await
        .unwrap();
    let created = read_json(res.into_body()).await;
    let id = created["id"].as_i64().unwrap();

    let res = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            &format!("/v1/notes/{id}"),
            &serde_json::json!({ "body": "new" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated = read_json(res.into_body()).await;
    assert_eq!(updated["body"], "new");

    let res = app
        .oneshot(
            Request::get(format!("/v1/notes/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let again = read_json(res.into_body()).await;
    assert_eq!(again["body"], "new");
}

#[tokio::test]
async fn delete_then_get_is_404() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/notes",
            &serde_json::json!({ "body": "doomed" }),
        ))
        .await
        .unwrap();
    let created = read_json(res.into_body()).await;
    let id = created["id"].as_i64().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/notes/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .oneshot(
            Request::get(format!("/v1/notes/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_respects_limit() {
    let app = app().await;
    for i in 0..5 {
        let res = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/notes",
                &serde_json::json!({ "body": format!("note {i}") }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
    let res = app
        .oneshot(
            Request::get("/v1/notes?limit=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn request_id_header_propagates() {
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("x-request-id", "test-req-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let req_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(req_id, "test-req-123");
}
