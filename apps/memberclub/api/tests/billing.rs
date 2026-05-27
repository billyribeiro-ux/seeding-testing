//! Integration tests for the Phase 8 billing slice in memberclub-api.
//!
//! What we prove:
//!
//!   1. `POST /v1/billing/checkout` requires auth, validates the
//!      price_id whitelist, and lazily mints a stripe_customer row.
//!   2. `POST /v1/billing/portal` returns the portal URL with the
//!      caller's return_to embedded.
//!   3. `POST /webhooks/stripe` ALWAYS verifies the signature; replays
//!      are idempotent (no DB churn on the second delivery); each
//!      supported event type mirrors into the right table.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

use memberclub_api::auth::Jwt;
use memberclub_api::{AppState, migrate, router};

const WEBHOOK_SECRET: &str = "whsec_test_secret_long_enough_value";

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

fn router_for(pool: SqlitePool, with_webhook_secret: bool) -> axum::Router {
    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    let mut state = AppState::new(pool, key, jwt);
    if with_webhook_secret {
        state = state.with_stripe_webhook_secret(WEBHOOK_SECRET);
    }
    router(state)
}

async fn read_json(b: Body) -> Value {
    let bytes = to_bytes(b, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_req(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn register_and_login(app: &axum::Router, email: &str) -> String {
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email": email, "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/login",
            &json!({"email": email, "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    body["access_token"].as_str().unwrap().to_string()
}

fn sign_stripe(body: &[u8], secret: &str) -> (i64, String) {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{t}.").as_bytes());
    mac.update(body);
    let sig = hex::encode(mac.finalize().into_bytes());
    (t, format!("t={t},v1={sig}"))
}

// ---------------------------------------------------------------------------
// Checkout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn checkout_requires_auth() {
    let app = router_for(pool().await, false);
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/billing/checkout",
            &json!({"price_id": "price_pro_monthly"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn checkout_rejects_unknown_price_id() {
    let pool = pool().await;
    let app = router_for(pool.clone(), false);
    let token = register_and_login(&app, "alice@billing.test").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/billing/checkout")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"price_id": "price_free"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn checkout_returns_url_and_mints_stripe_customer() {
    let pool = pool().await;
    let app = router_for(pool.clone(), false);
    let token = register_and_login(&app, "bob@billing.test").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/billing/checkout")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"price_id": "price_pro_monthly"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let url = body["url"].as_str().unwrap();
    assert!(url.starts_with("https://checkout.stripe."), "got {url}");

    // A stripe_customers row was minted for this user.
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM stripe_customers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

// ---------------------------------------------------------------------------
// Portal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn portal_returns_url_with_return_to() {
    let pool = pool().await;
    let app = router_for(pool.clone(), false);
    let token = register_and_login(&app, "carol@billing.test").await;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/billing/portal")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"return_url": "https://app.memberclub.test/account"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let url = body["url"].as_str().unwrap();
    assert!(url.contains("return_to=https://app.memberclub.test/account"));
}

// ---------------------------------------------------------------------------
// Webhook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_requires_signature() {
    let app = router_for(pool().await, true);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .body(Body::from(r#"{"id":"evt_1","type":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn webhook_rejects_bad_signature() {
    let app = router_for(pool().await, true);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("stripe-signature", "t=1,v1=deadbeef")
                .body(Body::from(r#"{"id":"evt_1","type":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn webhook_mirrors_checkout_session_completed_then_replay_is_noop() {
    let pool = pool().await;
    let app = router_for(pool.clone(), true);
    // First register the user we'll attach to the customer mirror.
    let _ = register_and_login(&app, "dave@billing.test").await;
    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE email = ?")
        .bind("dave@billing.test")
        .fetch_one(&pool)
        .await
        .unwrap();

    let body = json!({
        "id": "evt_checkout_1",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": "cs_test_1",
                "customer": "cus_test_dave",
                "metadata": { "user_id": uid.to_string() },
            }
        }
    })
    .to_string();
    let (_t, sig) = sign_stripe(body.as_bytes(), WEBHOOK_SECRET);

    // First delivery: should land + mint a stripe_customers row.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("stripe-signature", &sig)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM stripe_customers WHERE stripe_customer_id = 'cus_test_dave'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    // Replay (same body, same signature): should 200 OK with no DB churn.
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("stripe-signature", &sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let (events,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM stripe_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(events, 1, "replay must NOT insert a second event row");
}

#[tokio::test]
async fn webhook_mirrors_subscription_lifecycle() {
    let pool = pool().await;
    let app = router_for(pool.clone(), true);
    let _ = register_and_login(&app, "eve@billing.test").await;
    let (uid,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE email = ?")
        .bind("eve@billing.test")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Pre-seed a customer mirror so the subscription event finds the user.
    sqlx::query("INSERT INTO stripe_customers (user_id, stripe_customer_id) VALUES (?, ?)")
        .bind(uid)
        .bind("cus_test_eve")
        .execute(&pool)
        .await
        .unwrap();

    // 1) created — status active.
    let body1 = json!({
        "id": "evt_sub_1",
        "type": "customer.subscription.created",
        "data": { "object": {
            "id": "sub_test_1",
            "customer": "cus_test_eve",
            "status": "active",
            "cancel_at_period_end": false,
            "current_period_end": 1_900_000_000_i64,
            "items": { "data": [ { "price": { "id": "price_pro_monthly" } } ] }
        }}
    })
    .to_string();
    let (_, sig1) = sign_stripe(body1.as_bytes(), WEBHOOK_SECRET);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("stripe-signature", &sig1)
                .body(Body::from(body1))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2) updated — same subscription, status canceled.
    let body2 = json!({
        "id": "evt_sub_2",
        "type": "customer.subscription.updated",
        "data": { "object": {
            "id": "sub_test_1",
            "customer": "cus_test_eve",
            "status": "canceled",
            "cancel_at_period_end": true,
            "current_period_end": 1_900_000_000_i64,
            "items": { "data": [ { "price": { "id": "price_pro_monthly" } } ] }
        }}
    })
    .to_string();
    let (_, sig2) = sign_stripe(body2.as_bytes(), WEBHOOK_SECRET);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/stripe")
                .header("stripe-signature", &sig2)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify the mirrored row reflects the latest event.
    let (sub_count, status, cancel): (i64, String, i64) = sqlx::query_as(
        "SELECT COUNT(*), MAX(status), MAX(cancel_at_period_end) FROM stripe_subscriptions",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sub_count, 1, "still one subscription row (UPSERT)");
    assert_eq!(status, "canceled");
    assert_eq!(cancel, 1);
}
