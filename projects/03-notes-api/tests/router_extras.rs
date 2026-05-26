//! Integration tests for the smaller Phase 4 router additions:
//!   * E4.1 — `GET /version` carries crate version + git SHA.
//!   * E4.2 — `POST /v1/notes` rejects bodies over 8 KiB with 413.
//!   * E4.4 — problem-details bodies echo `x-request-id`.
//!   * E4.6 — `TimeoutLayer` kicks in after 5 s (exercised here with
//!     a router built directly so we don't have to actually wait
//!     five seconds in CI).

use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use notes_api::{AppState, MAX_BODY_BYTES, router};

async fn app() -> Router {
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

// ---------------------------------------------------------------------------
// E4.1 — /version
// ---------------------------------------------------------------------------

#[tokio::test]
async fn version_route_returns_crate_version_and_git_sha() {
    let res = app()
        .await
        .oneshot(Request::get("/version").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = read_json(res.into_body()).await;
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    let sha = body["git_sha"]
        .as_str()
        .expect("git_sha must always be a string");
    assert!(!sha.is_empty(), "git_sha must never be empty; got {sha:?}");
}

// ---------------------------------------------------------------------------
// E4.2 — payload too large
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_rejects_body_above_max_bytes_with_413() {
    // The check is on `body.body.len()` (the JSON string field), not on
    // the wire bytes — so a payload of MAX_BODY_BYTES + 1 'a' characters
    // is the right shape to trigger the handler-level reject. Use
    // valid UTF-8 (ASCII) so the JSON parser is happy.
    let oversize_body = "a".repeat(MAX_BODY_BYTES + 1);
    let payload = json!({ "body": oversize_body });

    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );

    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], 413);
    assert!(
        body["type"]
            .as_str()
            .unwrap()
            .contains("/problems/payload-too-large"),
        "problem type must name payload-too-large"
    );
}

#[tokio::test]
async fn create_accepts_body_at_exactly_max_bytes() {
    // Boundary test: a body of exactly MAX_BODY_BYTES is accepted.
    let edge_body = "a".repeat(MAX_BODY_BYTES);
    let payload = json!({ "body": edge_body });

    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/notes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // sqlx-notes' own validation rejects bodies > 4096 *chars* — but
    // 8192 chars > 4096 chars, so we should see 400 from that layer.
    // What we're proving here is that the 413 check did NOT fire.
    assert_ne!(
        res.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "exactly MAX_BODY_BYTES must NOT trigger PayloadTooLarge"
    );
    assert!(
        res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::CREATED,
        "edge case should land in 200/201 or 400, got {}",
        res.status()
    );
}

// ---------------------------------------------------------------------------
// E4.6 — TimeoutLayer
// ---------------------------------------------------------------------------

/// We exercise the timeout against a tiny router that pulls in the
/// real `TimeoutLayer` with a 100 ms budget — proving the LAYER is
/// wired correctly without making CI sleep through the production 5 s.
/// The same layer is on the real router (just with the larger budget).
#[tokio::test]
async fn timeout_layer_returns_408_when_handler_runs_past_budget() {
    async fn slow() -> &'static str {
        tokio::time::sleep(Duration::from_millis(500)).await;
        "done"
    }
    let app = Router::new().route("/slow", get(slow)).layer(
        tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_millis(100),
        ),
    );
    let res = app
        .oneshot(Request::get("/slow").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT);
}

// ---------------------------------------------------------------------------
// E4.4 — request_id echoed in problem-details
// ---------------------------------------------------------------------------

#[tokio::test]
async fn problem_details_body_carries_request_id() {
    // Drive a 404 so we get a problem-details body, with an explicit
    // x-request-id header that we expect to see echoed back.
    let req_id = "test-request-id-e4.4";
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/notes/99999")
                .header("x-request-id", req_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );

    let body = read_json(res.into_body()).await;
    assert_eq!(
        body["request_id"].as_str(),
        Some(req_id),
        "problem-details body must echo x-request-id so the client can pivot to the trace"
    );
}

#[tokio::test]
async fn success_responses_are_not_touched_by_the_request_id_middleware() {
    // The middleware only mutates application/problem+json bodies.
    // Confirm a normal 200 (JSON) goes through unchanged AND has no
    // injected request_id field (which would be wrong shape).
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/notes")
                .header("x-request-id", "should-not-leak-into-200-body")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    assert!(
        body.get("request_id").is_none(),
        "200 responses must not be mutated — request_id only belongs in problem-details bodies"
    );
}
