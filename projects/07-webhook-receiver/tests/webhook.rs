//! Integration tests for webhook-receiver.
//!
//! Each test crafts a Stripe-style signature with a known secret, sends the
//! request, and asserts: signature verify works, replay is idempotent,
//! tampering is rejected, stale timestamps are rejected.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use webhook_receiver::{AppState, compute_signature, migrate, router};

const TEST_SECRET: &[u8] = b"whsec_test_secret_for_unit_tests_only_do_not_use_in_prod";

async fn app() -> (axum::Router, sqlx::SqlitePool) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let app = router(AppState::new(pool.clone(), TEST_SECRET.to_vec()));
    (app, pool)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn signed_request(secret: &[u8], timestamp: u64, body: &str) -> Request<Body> {
    let sig = compute_signature(secret, timestamp, body.as_bytes());
    Request::builder()
        .method("POST")
        .uri("/webhooks/stripe")
        .header("content-type", "application/json")
        .header("stripe-signature", format!("t={timestamp},v1={sig}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn sample_event(id: &str, ts: i64) -> String {
    format!(
        r#"{{"id":"{id}","type":"checkout.session.completed","created":{ts},"data":{{"object":{{}}}}}}"#
    )
}

#[tokio::test]
async fn rejects_missing_signature_header() {
    let (app, _) = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("content-type", "application/json")
                .body(Body::from(sample_event("evt_1", 1)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_tampered_signature() {
    let (app, _) = app().await;
    let ts = now_secs();
    let body = sample_event("evt_2", ts as i64);
    let mut req = signed_request(TEST_SECRET, ts, &body);
    // Tamper with the signature.
    let bad = format!("t={ts},v1=0000000000000000000000000000000000000000000000000000000000000000");
    req.headers_mut()
        .insert("stripe-signature", bad.parse().unwrap());
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_stale_timestamp() {
    let (app, _) = app().await;
    let stale = now_secs() - 10 * 60; // 10 minutes ago
    let body = sample_event("evt_3", stale as i64);
    let req = signed_request(TEST_SECRET, stale, &body);
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn accepts_valid_signature() {
    let (app, pool) = app().await;
    let ts = now_secs();
    let body = sample_event("evt_4", ts as i64);
    let req = signed_request(TEST_SECRET, ts, &body);
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // The event row was created AND processed.
    let row: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT stripe_event_id, processed_at FROM stripe_events WHERE stripe_event_id = 'evt_4'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0.as_deref(), Some("evt_4"));
    assert!(row.1.is_some(), "processed_at must be set after handling");
}

#[tokio::test]
async fn duplicate_delivery_is_idempotent() {
    let (app, pool) = app().await;
    let ts = now_secs();
    let body = sample_event("evt_5", ts as i64);

    // First delivery: 200.
    let res = app
        .clone()
        .oneshot(signed_request(TEST_SECRET, ts, &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Second delivery, same body, same signature: 200, no double-insert.
    let res = app
        .clone()
        .oneshot(signed_request(TEST_SECRET, ts, &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Third delivery, same id but new ts (re-signed): 200, still no double-insert.
    let ts2 = ts + 1;
    let body2 = sample_event("evt_5", ts2 as i64); // same id
    let res = app
        .oneshot(signed_request(TEST_SECRET, ts2, &body2))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stripe_events WHERE stripe_event_id = 'evt_5'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "exactly one row regardless of retries");
}

#[tokio::test]
async fn resumes_event_stored_but_not_processed_after_a_crash() {
    // Simulates an earlier delivery that stored the receipt row but crashed
    // before `mark_processed`. The retry must RESUME (process it), not skip
    // it as a finished duplicate — otherwise the side effect is lost.
    let (app, pool) = app().await;
    let ts = now_secs();
    let body = sample_event("evt_crash", ts as i64);

    let meta = webhook_receiver::parse_event_meta(body.as_bytes()).unwrap();
    let id = webhook_receiver::store_event(&pool, &meta, body.as_bytes())
        .await
        .unwrap()
        .expect("freshly stored");
    let processed_before: Option<String> =
        sqlx::query_scalar("SELECT processed_at FROM stripe_events WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        processed_before.is_none(),
        "precondition: not yet processed"
    );

    let res = app
        .oneshot(signed_request(TEST_SECRET, ts, &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let (count, processed): (i64, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*), MAX(processed_at) FROM stripe_events WHERE stripe_event_id = 'evt_crash'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "still exactly one row");
    assert!(
        processed.is_some(),
        "the crashed-mid-handler event must now be processed on retry"
    );
}

#[tokio::test]
async fn rejects_malformed_json() {
    let (app, _) = app().await;
    let ts = now_secs();
    let body = "this is not json";
    let req = signed_request(TEST_SECRET, ts, body);
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_missing_event_id() {
    let (app, _) = app().await;
    let ts = now_secs();
    let body = r#"{"type":"x","created":1}"#;
    let req = signed_request(TEST_SECRET, ts, body);
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_is_open() {
    let (app, _) = app().await;
    let res = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
