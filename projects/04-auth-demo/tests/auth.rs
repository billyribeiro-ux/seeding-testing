//! Integration tests for the auth-demo dual-mode auth flow.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, migrate, router};

async fn app() -> axum::Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
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

#[tokio::test]
async fn health_is_ok() {
    let res = app()
        .await
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn register_creates_user() {
    let res = app()
        .await
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"alice@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["email"], "alice@example.com");
    assert!(body["id"].is_number());
}

#[tokio::test]
async fn register_rejects_weak_password() {
    let res = app()
        .await
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"alice@example.com","password":"shortpw"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_rejects_invalid_email() {
    let res = app()
        .await
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"not-an-email","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_then_register_same_email_conflicts() {
    let app = app().await;
    let body = json!({"email":"alice@example.com","password":"correct horse battery staple"});
    let _ = app
        .clone()
        .oneshot(json_req("POST", "/auth/register", &body))
        .await
        .unwrap();
    let res = app
        .oneshot(json_req("POST", "/auth/register", &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_and_me_via_bearer() {
    let app = app().await;

    // register
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"bob@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();

    // login
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"bob@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    let token = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert!(!token.is_empty());

    // /me with bearer
    let res = app
        .clone()
        .oneshot(
            Request::get("/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me = read_json(res.into_body()).await;
    assert_eq!(me["email"], "bob@example.com");

    // refresh
    let res = app
        .oneshot(json_req(
            "POST",
            "/auth/refresh",
            &json!({"refresh_token": refresh}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    assert!(body["access_token"].as_str().unwrap().len() > 10);
}

#[tokio::test]
async fn login_and_me_via_cookie() {
    let app = app().await;
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"carol@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();

    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"carol@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    // Pull the cookie header value (everything up to the first ';').
    let cookie_value: String = set_cookie.split(';').next().unwrap_or_default().to_string();
    assert!(cookie_value.starts_with("session="));

    let res = app
        .oneshot(
            Request::get("/me")
                .header("cookie", &cookie_value)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me = read_json(res.into_body()).await;
    assert_eq!(me["email"], "carol@example.com");
}

#[tokio::test]
async fn me_without_any_auth_is_401() {
    let res = app()
        .await
        .oneshot(Request::get("/me").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_with_wrong_password_is_401() {
    let app = app().await;
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"dave@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let res = app
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"dave@example.com","password":"WRONG WRONG WRONG"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_for_unknown_email_is_401_not_404() {
    let res = app()
        .await
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"nobody@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    // 401 — same as wrong password. No enumeration.
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_invalidates_cookie() {
    let app = app().await;
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"eve@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"eve@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .to_string();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Old cookie no longer authenticates.
    let res = app
        .oneshot(
            Request::get("/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// E7.5 — first-user-becomes-admin. The first account to register on a
/// fresh DB is auto-promoted to `is_admin = true`; the second is a
/// normal member. This is the "bootstrap the owner" pattern: it avoids
/// shipping a hard-coded admin password while still leaving the system
/// usable on first boot.
#[tokio::test]
async fn first_registered_user_is_admin() {
    let app = app().await;

    // First user → admin.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"founder@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["is_admin"], true, "first user should be admin");

    // Second user → normal member.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({"email":"member@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = read_json(res.into_body()).await;
    assert_eq!(body["is_admin"], false, "second user should NOT be admin");

    // Confirm via /me using bearer for the first user — defends against
    // the response-body lying while the DB row is wrong.
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email":"founder@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let login = read_json(res.into_body()).await;
    let token = login["access_token"].as_str().unwrap().to_string();

    let res = app
        .oneshot(
            Request::get("/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me = read_json(res.into_body()).await;
    assert_eq!(me["is_admin"], true);
}
