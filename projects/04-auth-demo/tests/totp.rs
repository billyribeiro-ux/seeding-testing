//! Integration tests for the TOTP / 2FA flow.
//!
//! Each test:
//!   1. Registers + logs in a user.
//!   2. Calls /auth/totp/enroll to get the secret + recovery codes.
//!   3. Optionally confirms enrollment by submitting a real code (generated
//!      via the test-only `current_code_for_secret` helper).
//!   4. Drives the login flow that's now gated on TOTP.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, migrate, router, totp};

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

/// Register + login. Returns the user's email and the access_token to use
/// as the bearer for subsequent calls.
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

#[tokio::test]
async fn enroll_returns_secret_and_eight_recovery_codes() {
    let app = app().await;
    let token = signed_in(&app, "alice@totp.test").await;

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = read_json(res.into_body()).await;
    assert!(body["secret"].as_str().unwrap().len() >= 16);
    assert!(
        body["provisioning_uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/")
    );
    let codes = body["recovery_codes"].as_array().unwrap();
    assert_eq!(codes.len(), 8);
    // Each code is hyphenated hex like a1b2-c3d4-e5f6-7890.
    for c in codes {
        assert_eq!(c.as_str().unwrap().len(), 19);
    }
}

#[tokio::test]
async fn confirm_enables_totp_then_login_requires_it() {
    let app = app().await;
    let token = signed_in(&app, "bob@totp.test").await;

    // Enroll
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let secret = body["secret"].as_str().unwrap().to_string();

    // Confirm with a real code derived from the secret
    let code = totp::current_code_for_secret(&secret).expect("derive code");
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/totp/confirm",
            &json!({"code": code}),
        ))
        .await
        .unwrap();
    // Bearer auth required, supplied via the cookie/bearer from signed_in.
    // (We re-issue the request with the token header below if needed.)
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "without auth header, must be 401"
    );

    // Re-confirm WITH the bearer
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Now password-only login is rejected with TotpRequired
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email": "bob@totp.test", "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Login with the TOTP code succeeds
    let code = totp::current_code_for_secret(&secret).expect("derive code");
    let res = app
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({
                "email": "bob@totp.test",
                "password": "correct horse battery staple",
                "totp": code,
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn recovery_code_logs_in_once_and_then_fails() {
    let app = app().await;
    let token = signed_in(&app, "carol@totp.test").await;

    // Enroll + capture secret + recovery codes
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let secret = body["secret"].as_str().unwrap().to_string();
    let recovery = body["recovery_codes"][0].as_str().unwrap().to_string();

    // Confirm enrollment
    let code = totp::current_code_for_secret(&secret).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // First login via recovery code: works
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({
                "email": "carol@totp.test",
                "password": "correct horse battery staple",
                "recovery_code": recovery,
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Same recovery code is now consumed — second attempt fails
    let res = app
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({
                "email": "carol@totp.test",
                "password": "correct horse battery staple",
                "recovery_code": recovery,
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn enroll_twice_conflicts() {
    let app = app().await;
    let token = signed_in(&app, "dave@totp.test").await;

    // First enroll succeeds
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let secret = body["secret"].as_str().unwrap().to_string();

    // Confirm
    let code = totp::current_code_for_secret(&secret).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Second enroll attempt = 409 TotpAlreadyEnabled
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn confirm_with_wrong_code_rejects() {
    let app = app().await;
    let token = signed_in(&app, "eve@totp.test").await;

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": "000000"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disable_with_correct_code_clears_totp() {
    let app = app().await;
    let token = signed_in(&app, "frank@totp.test").await;

    // Enroll + confirm
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/enroll")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let secret = body["secret"].as_str().unwrap().to_string();
    let code = totp::current_code_for_secret(&secret).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/confirm")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Disable with a fresh code
    let code = totp::current_code_for_secret(&secret).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/disable")
                .header("authorization", format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"code": code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Login no longer requires TOTP
    let res = app
        .oneshot(json_req(
            "POST",
            "/auth/login",
            &json!({"email": "frank@totp.test", "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
