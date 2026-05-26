//! Integration tests for E6.7 — refresh-token reuse detection.
//!
//! What we prove:
//!
//!   1. `/auth/login` records a brand-new row in `refresh_tokens` with
//!      `parent_jti = NULL`. That's the root of the family.
//!   2. A legitimate `/auth/refresh` rotates: the presented row is
//!      marked `used_at`, a new row is inserted as its child with the
//!      same `family_id`, and the response carries new tokens.
//!   3. **Reuse detection:** if the client (or an attacker) replays a
//!      refresh JWT that has already been used, the entire family is
//!      revoked AND an `audit_logs` row is written. Subsequent refreshes
//!      from any descendant in the family also fail with `401`.
//!   4. A forged refresh JWT with a `jti` we never issued is `401` (and
//!      writes no audit row — we don't distinguish from reuse on the
//!      wire, but we DO distinguish in the DB so SOC eyes can tell).
//!   5. `password_reset::complete` also revokes every active refresh
//!      row for that user — mirrors the existing "revoke all sessions"
//!      behavior.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, migrate, router};

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

/// Register + login a user. Returns the (access_token, refresh_token)
/// pair from /auth/login.
async fn signed_in(app: &axum::Router, email: &str) -> (String, String) {
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({ "email": email, "password": PASSWORD }),
        ))
        .await
        .unwrap();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({ "email": email, "password": PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    (
        body["access_token"].as_str().unwrap().to_string(),
        body["refresh_token"].as_str().unwrap().to_string(),
    )
}

async fn refresh_once(app: &axum::Router, refresh_token: &str) -> (StatusCode, Option<String>) {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/refresh",
            &json!({ "refresh_token": refresh_token }),
        ))
        .await
        .unwrap();
    let status = res.status();
    if status != StatusCode::OK {
        return (status, None);
    }
    let body = read_json(res.into_body()).await;
    let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
    (status, Some(new_refresh))
}

#[tokio::test]
async fn login_creates_root_refresh_row() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _ = signed_in(&app, "alice@rt.test").await;

    let (count, root_count): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(CASE WHEN parent_jti IS NULL THEN 1 END)
         FROM refresh_tokens",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "login should insert exactly one row");
    assert_eq!(root_count, 1, "that row's parent_jti must be NULL (root)");
}

#[tokio::test]
async fn legitimate_refresh_rotates_and_chains() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let (_at, r0) = signed_in(&app, "bob@rt.test").await;

    let (status, r1) = refresh_once(&app, &r0).await;
    assert_eq!(status, StatusCode::OK);
    let r1 = r1.expect("legitimate refresh must return a new refresh_token");
    assert_ne!(r1, r0, "rotation must mint a new refresh token");

    // The DB now has TWO rows in the same family: the root (used) and
    // the child (unused), and parent_jti chains them.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);

    let (used, unused, families): (i64, i64, i64) = sqlx::query_as(
        "SELECT
           COUNT(CASE WHEN used_at    IS NOT NULL THEN 1 END),
           COUNT(CASE WHEN used_at    IS     NULL THEN 1 END),
           COUNT(DISTINCT family_id)
         FROM refresh_tokens",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(used, 1, "root should be marked used after one refresh");
    assert_eq!(unused, 1, "the new child should be unused");
    assert_eq!(families, 1, "both rows belong to the same family");

    // r1 keeps working — second rotation produces r2.
    let (status, r2) = refresh_once(&app, &r1).await;
    assert_eq!(status, StatusCode::OK);
    assert!(r2.is_some());
}

#[tokio::test]
async fn replay_of_used_refresh_kills_the_whole_family() {
    let pool = pool().await;
    let app = router_for(pool.clone());
    let (_at, r0) = signed_in(&app, "carol@rt.test").await;

    // Legitimate rotation.
    let (status, r1) = refresh_once(&app, &r0).await;
    assert_eq!(status, StatusCode::OK);
    let r1 = r1.unwrap();

    // ATTACKER (or just a buggy retry) replays the root — which is now
    // marked used. This MUST 401 and MUST revoke the whole family.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/refresh",
            &json!({ "refresh_token": r0 }),
        ))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "replay of a used refresh must 401"
    );

    let revoked_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE revoked_at IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        revoked_count >= 2,
        "reuse must revoke every row in the family, got {revoked_count} revoked"
    );

    // The legitimate user's r1 is now dead too — they have to re-login.
    let (status_after, _) = refresh_once(&app, &r1).await;
    assert_eq!(
        status_after,
        StatusCode::UNAUTHORIZED,
        "the legitimate child token is also dead — family-wide revoke"
    );

    // An audit row was written.
    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = ?")
        .bind("refresh_token.reuse_detected")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit_count, 1, "reuse detection must write an audit row");
}

#[tokio::test]
async fn forged_jti_refresh_is_401() {
    // Issue a JWT whose jti was never recorded in refresh_tokens. The
    // /auth/refresh handler should reject with 401 and NOT write an
    // audit row (only true reuse triggers the audit).
    let pool = pool().await;
    let app = router_for(pool.clone());
    let _ = signed_in(&app, "dave@rt.test").await;

    // Mint a refresh JWT directly with a totally different Jwt key,
    // because the API uses its own Jwt instance. Easier path: register
    // a SECOND user, snag their refresh, then drop their row from the
    // table. Now the JWT is valid but the jti is unknown.
    let (_at_e, r_e) = signed_in(&app, "eve@rt.test").await;
    sqlx::query(
        "DELETE FROM refresh_tokens WHERE user_id = (SELECT id FROM users WHERE email = ?)",
    )
    .bind("eve@rt.test")
    .execute(&pool)
    .await
    .unwrap();

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/refresh",
            &json!({ "refresh_token": r_e }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE action = ?")
        .bind("refresh_token.reuse_detected")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        audit_count, 0,
        "unknown-jti must NOT raise the reuse-detection audit; only true replay does"
    );
}

#[tokio::test]
async fn password_reset_revokes_all_refresh_tokens() {
    // Lesson 6.5 says reset must revoke every active session. The same
    // logic applies to refresh tokens — a stolen refresh JWT outlives
    // a session cookie revoke. After `password_reset::complete`, every
    // active refresh row for the user must be marked revoked.
    let pool = pool().await;
    let app = router_for(pool.clone());
    let (_at, r0) = signed_in(&app, "frank@rt.test").await;

    // Confirm there's an active refresh row.
    let active_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_before, 1);

    // Drive the reset flow through the HTTP endpoints (no SMTP — debug
    // builds echo the token in the response body).
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/forgot-password",
            &json!({ "email": "frank@rt.test" }),
        ))
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let token = body["token"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/reset-password",
            &json!({ "token": &token, "new_password": "a fresh password 123!" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let active_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        active_after, 0,
        "password reset must revoke every active refresh-token row"
    );

    // The old refresh now 401s — even though it was never USED, it was
    // REVOKED by the reset.
    let (status, _) = refresh_once(&app, &r0).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
