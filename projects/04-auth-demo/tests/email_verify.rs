//! Integration tests for the Phase 6.4 email-verification endpoints.
//!
//! Each test:
//!   1. Registers + logs in a user.
//!   2. Calls `POST /auth/verify-email/request` (bearer-authenticated) and
//!      pulls the plaintext token out of the response body — debug builds
//!      include it, see [`auth_demo::verify_email_request`].
//!   3. Calls `POST /auth/verify-email/confirm` with that token.
//!
//! Hermetic: every test runs against a fresh `sqlite::memory:` pool with
//! `max_connections(1)` (the only safe setting for in-memory SQLite — see
//! the troubleshooting matrix).

use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, email_verify, jwt::Jwt, migrate, router};

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

fn router_for(pool: SqlitePool) -> axum::Router {
    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    router(AppState::new(pool, key, jwt))
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

/// Register + login a user; return the bearer access token.
async fn signed_in(app: &axum::Router, email: &str) -> String {
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email": email, "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email": email, "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    body["access_token"].as_str().unwrap().to_string()
}

/// POST /auth/verify-email/request with the given bearer; return the
/// plaintext token from the response body (debug-build behavior).
async fn request_token(app: &axum::Router, bearer: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-email/request")
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    body["token"]
        .as_str()
        .expect("debug builds must echo the token in the response")
        .to_string()
}

async fn confirm(app: &axum::Router, token: &str) -> StatusCode {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/verify-email/confirm",
            &json!({ "token": token }),
        ))
        .await
        .unwrap();
    res.status()
}

async fn fetch_is_verified(pool: &SqlitePool, email: &str) -> bool {
    let (v,): (i64,) = sqlx::query_as("SELECT is_email_verified FROM users WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap();
    v == 1
}

#[tokio::test]
async fn request_returns_a_token() {
    let app = router_for(pool().await);
    let bearer = signed_in(&app, "alice@verify.test").await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-email/request")
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = read_json(res.into_body()).await;
    assert_eq!(body["sent"], json!(true));
    let token = body["token"]
        .as_str()
        .expect("debug builds must echo the token in the response");
    // 32 random bytes base64url-no-pad = 43 chars.
    assert_eq!(token.len(), 43);
}

#[tokio::test]
async fn confirm_with_valid_token_flips_is_email_verified_to_true() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let bearer = signed_in(&app, "bob@verify.test").await;

    assert!(
        !fetch_is_verified(&pool, "bob@verify.test").await,
        "fresh users start unverified"
    );

    let token = request_token(&app, &bearer).await;
    assert_eq!(confirm(&app, &token).await, StatusCode::NO_CONTENT);

    assert!(
        fetch_is_verified(&pool, "bob@verify.test").await,
        "is_email_verified must flip after a successful confirm"
    );
}

#[tokio::test]
async fn confirm_with_unknown_token_is_401() {
    let app = router_for(pool().await);
    // No `request` ever called — the token is fabricated.
    assert_eq!(
        confirm(&app, "totally-bogus-token-not-in-the-table").await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn confirm_with_expired_token_is_401() {
    // Issue a token directly into the DB with TTL=0, then sleep past it.
    // We bypass the HTTP request endpoint here because the HTTP endpoint
    // uses the 24-hour constant; the underlying `issue` function takes
    // any TTL we like.
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _bearer = signed_in(&app, "carol@verify.test").await;

    let user_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind("carol@verify.test")
        .fetch_one(&pool)
        .await
        .unwrap();

    let token = email_verify::issue(&pool, user_id, 0).await.unwrap();
    // expires_at is `now + 0s`. Sleep a beat so `expires_at > now` is false.
    tokio::time::sleep(Duration::from_millis(1100)).await;

    assert_eq!(confirm(&app, &token).await, StatusCode::UNAUTHORIZED);
    assert!(
        !fetch_is_verified(&pool, "carol@verify.test").await,
        "expired-token confirm must not flip the flag"
    );
}

#[tokio::test]
async fn confirm_is_single_use_second_call_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let bearer = signed_in(&app, "dave@verify.test").await;

    let token = request_token(&app, &bearer).await;
    assert_eq!(confirm(&app, &token).await, StatusCode::NO_CONTENT);
    // The same token a second time is now consumed.
    assert_eq!(confirm(&app, &token).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn request_invalidates_previous_unused_tokens() {
    let app = router_for(pool().await);
    let bearer = signed_in(&app, "eve@verify.test").await;

    let first = request_token(&app, &bearer).await;
    let second = request_token(&app, &bearer).await;
    assert_ne!(first, second, "resend must produce a fresh token");

    // The first (older) token is now invalidated by the resend.
    assert_eq!(confirm(&app, &first).await, StatusCode::UNAUTHORIZED);
    // The newest token still works.
    assert_eq!(confirm(&app, &second).await, StatusCode::NO_CONTENT);
}
