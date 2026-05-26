//! Integration tests for the auth surface — register / login / me / logout.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use memberclub_api::{AppState, auth::Jwt, migrate, router};

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
async fn health_ok() {
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
async fn metrics_endpoint_renders_prometheus_format() {
    let app = app().await;

    // Drive a few requests so the counters have non-zero values.
    let _ = app
        .clone()
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let _ = app
        .clone()
        .oneshot(Request::get("/v1/me").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let res = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(
        body.contains("http_requests_total"),
        "missing counter family; got:\n{body}"
    );
    assert!(
        body.contains("http_request_duration_seconds"),
        "missing histogram family; got:\n{body}"
    );
}

#[tokio::test]
async fn register_creates_user_and_logs_them_in() {
    let res = app()
        .await
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email":"alice@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        set_cookie.starts_with("session="),
        "expected session cookie on register, got: {set_cookie}"
    );

    let body = read_json(res.into_body()).await;
    assert_eq!(body["user"]["email"], "alice@example.com");
    assert_eq!(body["user"]["role"], "member");
    assert_eq!(body["user"]["tier"], "free");
    assert!(body["access_token"].as_str().unwrap().len() > 10);
}

#[tokio::test]
async fn register_rejects_duplicate_email() {
    let app = app().await;
    let body = json!({"email":"alice@example.com","password":"correct horse battery staple"});
    let res = app
        .clone()
        .oneshot(json_req("POST", "/v1/auth/register", &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app
        .oneshot(json_req("POST", "/v1/auth/register", &body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_with_wrong_password_is_401() {
    let app = app().await;
    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email":"bob@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    let res = app
        .oneshot(json_req(
            "POST",
            "/v1/auth/login",
            &json!({"email":"bob@example.com","password":"WRONG WRONG WRONG"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_returns_authenticated_user() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email":"carol@example.com","password":"correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = read_json(res.into_body()).await;
    let token = body["access_token"].as_str().unwrap().to_string();

    let res = app
        .oneshot(
            Request::get("/v1/me")
                .header("authorization", format!("Bearer {token}"))
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
async fn logout_invalidates_cookie() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email":"dave@example.com","password":"correct horse battery staple"}),
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
    assert!(cookie.starts_with("session="));

    // Cookie-authenticated /me works.
    let res = app
        .clone()
        .oneshot(
            Request::get("/v1/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Logout.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/logout")
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
            Request::get("/v1/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
