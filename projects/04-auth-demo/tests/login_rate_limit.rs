//! Integration tests for the Phase 6.7 per-IP rate limit on `/auth/login`.
//!
//! What we prove:
//!
//!   1. With `RateLimit { login_per_minute: 5 }`, the **first five**
//!      requests from a single IP all clear the layer (they fail at
//!      `InvalidCredentials = 401` because we send a bad password, but
//!      they DO reach the handler). The **sixth** request from the same
//!      IP is short-circuited by the governor at `429 Too Many Requests`
//!      with a `Retry-After` header.
//!   2. The limit is **per-IP**: a sixth request from a different
//!      `X-Forwarded-For` value still gets through (with a 401, because
//!      the password is still wrong).
//!   3. The layer is **scoped to `/auth/login`**: `/auth/register` (a
//!      neighbouring auth route) still answers normally after the
//!      login budget is exhausted.
//!
//! Tests use `X-Forwarded-For` to drive the `SmartIpKeyExtractor`
//! because the `tower::ServiceExt::oneshot` path doesn't set a
//! `ConnectInfo<SocketAddr>` extension. In production this header is
//! set by the load balancer in front of the API.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, RateLimit, jwt::Jwt, migrate, router};

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

fn router_with_rl(pool: SqlitePool, per_minute: u32) -> axum::Router {
    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    let state = AppState::new(pool, key, jwt).with_rate_limit(RateLimit {
        login_per_minute: per_minute,
    });
    router(state)
}

fn login_req(body: &Value, xff: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", xff)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn first_five_attempts_reach_the_handler_sixth_is_429() {
    let pool = pool().await;
    let app = router_with_rl(pool, 5);

    // Use bad credentials so we don't pollute the DB; the layer fires
    // before that matters anyway.
    let body = json!({
        "email": "ghost@rl.test",
        "password": "doesn't matter for this test, 12+ chars"
    });

    // The first 5 requests reach the handler. With a non-existent user
    // the handler returns 401 InvalidCredentials, NOT 429.
    for i in 1..=5 {
        let res = app
            .clone()
            .oneshot(login_req(&body, "10.0.0.7"))
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "attempt #{i} should hit the handler and return 401, got {}",
            res.status()
        );
    }

    // 6th request from the same IP — the governor short-circuits.
    let res = app
        .clone()
        .oneshot(login_req(&body, "10.0.0.7"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th attempt from same IP must be rate-limited"
    );
    assert!(
        res.headers().contains_key(header::RETRY_AFTER),
        "429 response must carry Retry-After so clients can back off correctly"
    );
}

#[tokio::test]
async fn other_ips_are_unaffected_by_one_ips_burst() {
    let pool = pool().await;
    let app = router_with_rl(pool, 5);
    let body = json!({
        "email": "ghost@rl.test",
        "password": "doesn't matter for this test, 12+ chars"
    });

    // Burn the budget for IP A.
    for _ in 0..5 {
        let _ = app
            .clone()
            .oneshot(login_req(&body, "203.0.113.10"))
            .await
            .unwrap();
    }
    let blocked = app
        .clone()
        .oneshot(login_req(&body, "203.0.113.10"))
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);

    // IP B has its own bucket — first attempt still gets through.
    let res = app
        .clone()
        .oneshot(login_req(&body, "198.51.100.99"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "a different IP must have its own bucket, not be blocked by IP A's burst"
    );
}

#[tokio::test]
async fn other_routes_are_not_rate_limited_by_the_login_layer() {
    let pool = pool().await;
    let app = router_with_rl(pool, 5);
    let bad = json!({
        "email": "ghost@rl.test",
        "password": "doesn't matter for this test, 12+ chars"
    });

    // Burn the login budget.
    for _ in 0..6 {
        let _ = app
            .clone()
            .oneshot(login_req(&bad, "192.0.2.42"))
            .await
            .unwrap();
    }
    // /auth/register still answers — it doesn't share the login bucket.
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "192.0.2.42")
        .body(Body::from(
            json!({
                "email": "real-user@rl.test",
                "password": "correct horse battery staple"
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "/auth/register must not share /auth/login's bucket"
    );
}

#[tokio::test]
async fn disabled_rate_limit_is_truly_disabled() {
    // RateLimit::default() = login_per_minute = 0 = NO layer.
    // Twenty rapid requests should all reach the handler (and 401).
    let pool = pool().await;
    let app = router_with_rl(pool, 0);
    let body = json!({
        "email": "ghost@rl.test",
        "password": "doesn't matter for this test, 12+ chars"
    });

    for i in 0..20 {
        let res = app
            .clone()
            .oneshot(login_req(&body, "10.0.0.7"))
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "with rate limit disabled, attempt #{i} must reach the handler"
        );
    }
}
