//! Integration tests for Phase 6 stretch E6.8 — passwordless magic-link login.
//!
//! What we prove:
//!
//!   1. `/auth/magic/request` with a known email returns 200 (debug
//!      builds echo the token) and writes exactly one `magic_links` row.
//!   2. `/auth/magic/confirm` with a valid token returns 200 + the same
//!      `LoginResponse` shape as the password login, and marks the
//!      token's `used_at`.
//!   3. Re-using a confirmed token is 401.
//!   4. An expired token is 401.
//!   5. `/auth/magic/request` for an unknown email is 204 and writes
//!      NO `magic_links` row — parallel to forgot-password.
//!   6. The unknown-email branch takes >= 80% of the 250 ms padding
//!      budget — same shape as forgot-password's constant-time test.
//!   7. Re-requesting a magic link invalidates any prior unused token.

use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, magic_link, migrate, router};

const PASSWORD: &str = "correct horse battery staple";

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

async fn register(app: &axum::Router, pool: &SqlitePool, email: &str) -> i64 {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email": email, "password": PASSWORD}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn request_magic_and_extract_token(app: &axum::Router, email: &str) -> String {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/magic/request",
            &json!({ "email": email }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    body["token"]
        .as_str()
        .expect("debug builds must echo the magic token")
        .to_string()
}

async fn confirm_magic(app: &axum::Router, token: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(json_req(
            "POST",
            "/auth/magic/confirm",
            &json!({ "token": token }),
        ))
        .await
        .unwrap()
}

#[tokio::test]
async fn request_for_known_user_issues_one_token() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "alice@magic.test").await;

    let token = request_magic_and_extract_token(&app, "alice@magic.test").await;
    // 32 random bytes b64url-no-pad => 43 chars.
    assert_eq!(token.len(), 43);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM magic_links WHERE used_at IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "exactly one unused magic_link row expected");
}

#[tokio::test]
async fn confirm_with_valid_token_returns_login_shape_and_marks_used() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let uid = register(&app, &pool, "bob@magic.test").await;

    let token = request_magic_and_extract_token(&app, "bob@magic.test").await;
    let res = confirm_magic(&app, &token).await;
    assert_eq!(res.status(), StatusCode::OK);

    // Set-Cookie carries a session cookie (HttpOnly).
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("magic-link confirm must set a session cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.starts_with("session="));
    assert!(set_cookie.to_lowercase().contains("httponly"));

    let body = read_json(res.into_body()).await;
    // Same shape as LoginResponse: user, access_token, refresh_token,
    // expires_in.
    assert_eq!(body["user"]["id"], json!(uid));
    assert_eq!(body["user"]["email"], json!("bob@magic.test"));
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
    assert!(body["expires_in"].is_i64());

    // The token row is now consumed.
    let (used_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM magic_links WHERE user_id = ? AND used_at IS NOT NULL",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(used_count, 1, "magic_link row must be marked used");
}

#[tokio::test]
async fn confirm_with_already_used_token_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "carol@magic.test").await;

    let token = request_magic_and_extract_token(&app, "carol@magic.test").await;
    let res = confirm_magic(&app, &token).await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = confirm_magic(&app, &token).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn confirm_with_expired_token_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let uid = register(&app, &pool, "dave@magic.test").await;

    // Issue with TTL=0, sleep just past it.
    let token = magic_link::issue(&pool, uid, 0).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let res = confirm_magic(&app, &token).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn request_for_unknown_email_is_204_with_no_db_write() {
    let pool = pool().await;
    let app = router_for(pool.clone());

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/magic/request",
            &json!({ "email": "ghost@magic.test" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM magic_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "no magic_link row should exist for an unknown email"
    );
}

#[tokio::test]
async fn request_for_malformed_email_is_204() {
    // Mirrors forgot-password's enumeration-safety contract: a malformed
    // input must look identical to an unknown email.
    let pool = pool().await;
    let app = router_for(pool);

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/magic/request",
            &json!({ "email": "definitely not an email" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn request_response_time_is_padded_to_a_constant_budget() {
    // Same shape as the forgot-password constant-time test: the cheap
    // branch (unknown email) must still spend at least 80% of the
    // 250 ms padding budget so an attacker can't enumerate accounts by
    // measuring latency.
    let pool = pool().await;
    let app = router_for(pool);

    let started = std::time::Instant::now();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/magic/request",
            &json!({ "email": "ghost@magic.test" }),
        ))
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(
        elapsed >= Duration::from_millis(200),
        "constant-time pad didn't kick in: magic/request returned in {elapsed:?}"
    );
}

#[tokio::test]
async fn request_invalidates_prior_unused_tokens() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "eve@magic.test").await;

    let first = request_magic_and_extract_token(&app, "eve@magic.test").await;
    let second = request_magic_and_extract_token(&app, "eve@magic.test").await;
    assert_ne!(first, second, "resend must produce a fresh token");

    // Older token is dead.
    let res = confirm_magic(&app, &first).await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "older magic-link token must be invalidated by the resend"
    );
    // Newest still works.
    let res = confirm_magic(&app, &second).await;
    assert_eq!(res.status(), StatusCode::OK);
}
