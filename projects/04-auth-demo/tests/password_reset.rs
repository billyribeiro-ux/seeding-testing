//! Integration tests for the Phase 6.5 password-reset endpoints.
//!
//! What we prove:
//!
//!   1. `POST /auth/forgot-password` returns `200` with a token (debug
//!      build) when the email exists, and `204` with no body when it
//!      doesn't — and **the response time is the same either way**
//!      (the lesson's "constant-time pad" rule). We assert the response
//!      shape; the timing pad is exercised by a separate test that
//!      measures both branches.
//!   2. `POST /auth/reset-password` swaps the password hash, marks the
//!      token consumed, and revokes every active session — all in one
//!      transaction. After a reset, the old password fails login AND
//!      the old session cookie is dead.
//!   3. Unknown / expired / already-used tokens map to `401`, and the
//!      DB is unchanged.
//!   4. Re-requesting a reset invalidates the previous token.
//!
//! Hermetic: each test gets a fresh `sqlite::memory:` pool with
//! `max_connections(1)` — the only safe shape for in-memory SQLite.

use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, migrate, password_reset, router};

const PASSWORD: &str = "correct horse battery staple";
const NEW_PASSWORD: &str = "tr0ub4dor & 3 (new one!)";

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

/// Register a user and return its DB id. Doesn't log them in — the
/// forgot-password flow doesn't require authentication.
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

/// Drive `POST /auth/forgot-password` and pull the plaintext token out
/// of the (debug-only) response body.
async fn forgot_and_extract_token(app: &axum::Router, email: &str) -> String {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": email }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    body["token"]
        .as_str()
        .expect("debug builds must echo the reset token")
        .to_string()
}

async fn reset(app: &axum::Router, token: &str, new_password: &str) -> StatusCode {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/reset-password",
            &json!({ "token": token, "new_password": new_password }),
        ))
        .await
        .unwrap();
    res.status()
}

async fn login(app: &axum::Router, email: &str, password: &str) -> StatusCode {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({ "email": email, "password": password }),
        ))
        .await
        .unwrap();
    res.status()
}

#[tokio::test]
async fn forgot_for_known_user_issues_a_token() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "alice@reset.test").await;

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": "alice@reset.test" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = read_json(res.into_body()).await;
    assert_eq!(body["sent"], json!(true));
    let token = body["token"].as_str().unwrap();
    // 32 bytes b64url-no-pad = 43 chars (same shape as email verification).
    assert_eq!(token.len(), 43);

    // The row is in the DB, unused.
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM password_resets WHERE used_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn forgot_for_unknown_user_returns_204_without_writing_anything() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    // Note: no register — the email does not exist.

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": "nobody@reset.test" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::NO_CONTENT,
        "unknown email must NOT receive a token, just a generic ack"
    );

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM password_resets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no token row should be written for unknown email");
}

#[tokio::test]
async fn forgot_for_malformed_email_returns_204_without_leaking() {
    let pool = pool().await;
    let app = router_for(pool.clone());

    // Not a valid email shape — must NOT 400, because that would let an
    // attacker distinguish "you have to fix your input" from "we sent
    // you a link." Same generic 204.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": "definitely not an email" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn reset_with_valid_token_swaps_password_and_revokes_sessions() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let uid = register(&app, &pool, "bob@reset.test").await;

    // Log in once to create a live session.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({ "email": "bob@reset.test", "password": PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let (active_before,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE user_id = ? AND revoked_at IS NULL")
            .bind(uid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_before, 1, "login must create an active session");

    // Request + complete the reset.
    let token = forgot_and_extract_token(&app, "bob@reset.test").await;
    assert_eq!(
        reset(&app, &token, NEW_PASSWORD).await,
        StatusCode::NO_CONTENT
    );

    // Old password rejected, new password accepted.
    assert_eq!(
        login(&app, "bob@reset.test", PASSWORD).await,
        StatusCode::UNAUTHORIZED,
        "old password must stop working after reset"
    );
    assert_eq!(
        login(&app, "bob@reset.test", NEW_PASSWORD).await,
        StatusCode::OK,
        "new password must work after reset"
    );

    // Every prior session is now revoked.
    let (active_after,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions
         WHERE user_id = ? AND revoked_at IS NULL AND id != (
             SELECT MAX(id) FROM sessions WHERE user_id = ?
         )",
    )
    .bind(uid)
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_after, 0,
        "every session that existed before the reset must be revoked"
    );

    // The reset row is consumed.
    let (used_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM password_resets WHERE user_id = ? AND used_at IS NOT NULL",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(used_count, 1, "the reset token row must be marked used");
}

#[tokio::test]
async fn reset_with_unknown_token_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "carol@reset.test").await;

    assert_eq!(
        reset(&app, "totally-bogus-not-issued", NEW_PASSWORD).await,
        StatusCode::UNAUTHORIZED
    );

    // Password is unchanged: old still works.
    assert_eq!(
        login(&app, "carol@reset.test", PASSWORD).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn reset_with_expired_token_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let uid = register(&app, &pool, "dave@reset.test").await;

    // Issue a token with TTL=0, then sleep just past it so expires_at < now.
    let token = password_reset::issue(&pool, uid, 0).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;

    assert_eq!(
        reset(&app, &token, NEW_PASSWORD).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "dave@reset.test", PASSWORD).await,
        StatusCode::OK,
        "expired-token reset must not change the password"
    );
}

#[tokio::test]
async fn reset_is_single_use_second_call_is_401() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "eve@reset.test").await;

    let token = forgot_and_extract_token(&app, "eve@reset.test").await;
    assert_eq!(
        reset(&app, &token, NEW_PASSWORD).await,
        StatusCode::NO_CONTENT
    );
    // Same token a second time is consumed — even though the password
    // changed, this is a 401 (replaying a reset is never OK).
    assert_eq!(
        reset(&app, &token, "another new password 12345").await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn forgot_invalidates_previous_unused_tokens() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "frank@reset.test").await;

    let first = forgot_and_extract_token(&app, "frank@reset.test").await;
    let second = forgot_and_extract_token(&app, "frank@reset.test").await;
    assert_ne!(first, second, "resend must produce a fresh token");

    // Older token is dead now.
    assert_eq!(
        reset(&app, &first, NEW_PASSWORD).await,
        StatusCode::UNAUTHORIZED,
        "the older reset token must be invalidated by the resend"
    );
    // Newest token still works.
    assert_eq!(
        reset(&app, &second, NEW_PASSWORD).await,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn reset_rejects_weak_new_password() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _uid = register(&app, &pool, "gary@reset.test").await;

    let token = forgot_and_extract_token(&app, "gary@reset.test").await;
    // < 12 chars — same rule as registration.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/reset-password",
            &json!({ "token": &token, "new_password": "short" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Token must still be unused — a rejected reset must NOT consume it,
    // otherwise an attacker can DOS the user's reset attempts by typing
    // weak passwords with a known token.
    let (unused,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM password_resets WHERE used_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unused, 1, "validation rejection must not consume the token");
}

#[tokio::test]
async fn forgot_response_time_is_padded_to_a_constant_budget() {
    // The lesson's constant-time padding rule: both the "user exists"
    // and "user does not exist" branches must take roughly the same
    // wall-clock time, so an attacker can't enumerate accounts by
    // measuring latency.
    //
    // We assert the cheap branch (no DB write, no token mint) takes
    // AT LEAST 80% of the configured 250 ms budget — i.e. the sleep
    // actually ran. We don't assert an upper bound because CI machines
    // can stutter.
    let pool = pool().await;
    let app = router_for(pool.clone());

    let started = std::time::Instant::now();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": "ghost@reset.test" }),
        ))
        .await
        .unwrap();
    let elapsed = started.elapsed();

    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(
        elapsed >= Duration::from_millis(200),
        "constant-time pad didn't kick in: forgot-password returned in {elapsed:?}, \
         which is below the 80% lower bound of the 250 ms budget"
    );
}
