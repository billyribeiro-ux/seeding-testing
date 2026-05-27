//! Integration tests for Phase 6 stretch E6.8 — OAuth (Google) flow.
//!
//! These tests stand up a wiremock `MockServer` that pretends to be
//! Google's authorize + token + userinfo endpoints. We inject the
//! mock URLs into `GoogleProvider::with_endpoints` and drive the
//! auth-demo router with `tower::ServiceExt::oneshot`, the same way
//! every other integration test in this crate does.
//!
//! What we prove:
//!
//!   1. **Happy path** — `/start` 302/303's to Google with a `state`
//!      query param; `/callback` exchanges the code, creates a user
//!      keyed by `google_sub`, and redirects to `return_to` with a
//!      session cookie set.
//!   2. **State CSRF** — `/callback` with a state we never issued is
//!      400 (and no user is created).
//!   3. **Single-use state** — replaying the same state is 400 the
//!      second time, because `consume_state` does a `DELETE ...
//!      RETURNING`.
//!   4. **Email linking** — if a user with the same email exists from
//!      a password signup, the second OAuth login attaches the
//!      `google_sub` to that row instead of creating a duplicate.
//!   5. **PKCE** — the body of the token-exchange POST contains the
//!      `code_verifier` we stored in `/start`. This is the integration
//!      check that proves we're actually doing PKCE end-to-end.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request as WmRequest, Respond, ResponseTemplate};

use auth_demo::{AppState, jwt::Jwt, migrate, oauth::GoogleProvider, router};

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

/// Build a wiremock server that answers the token endpoint with a
/// canned access_token, AND the userinfo endpoint with a canned
/// `{sub, email, ...}`.
async fn mock_google(sub: &str, email: &str, name: Option<&str>) -> MockServer {
    let server = MockServer::start().await;
    let body = json!({
        "access_token": "test-access-token",
        "token_type": "Bearer",
        "expires_in": 3600,
    });
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let mut info = json!({
        "sub": sub,
        "email": email,
        "email_verified": true,
    });
    if let Some(n) = name {
        info["name"] = json!(n);
    }
    Mock::given(method("GET"))
        .and(path("/userinfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(info))
        .mount(&server)
        .await;
    server
}

fn provider_for(server: &MockServer, redirect_uri: &str) -> GoogleProvider {
    GoogleProvider::with_endpoints(
        "client-test".to_string(),
        "secret-test".to_string(),
        redirect_uri.to_string(),
        format!("{}/discovery", server.uri()),
        format!("{}/authorize", server.uri()),
        format!("{}/token", server.uri()),
        format!("{}/userinfo", server.uri()),
    )
}

fn router_for(pool: SqlitePool, server: &MockServer) -> axum::Router {
    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    let provider = provider_for(server, "https://app.test/auth/oauth/google/callback");
    router(AppState::new(pool, key, jwt).with_oauth_google(provider))
}

fn router_no_oauth(pool: SqlitePool) -> axum::Router {
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

fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn extract_state(location: &str) -> String {
    let qs = location.split_once('?').unwrap().1;
    for kv in qs.split('&') {
        if let Some(v) = kv.strip_prefix("state=") {
            return v.to_string();
        }
    }
    panic!("no state= in {location}");
}

#[tokio::test]
async fn start_returns_302_with_state_param() {
    let pool = pool().await;
    let server = mock_google("sub-alice", "alice@oauth.test", Some("Alice")).await;
    let app = router_for(pool.clone(), &server);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start?return_to=/dashboard"))
        .await
        .unwrap();
    // axum's Redirect::to defaults to 303 See Other; both 302 and 303 satisfy
    // the OAuth spec's "redirect the user-agent" requirement.
    assert!(
        res.status().is_redirection(),
        "expected a 3xx redirect, got {}",
        res.status()
    );
    let loc = res
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.starts_with(&format!("{}/authorize", server.uri())));
    assert!(loc.contains("state="));
    assert!(loc.contains("code_challenge="));
    assert!(loc.contains("code_challenge_method=S256"));

    let state = extract_state(&loc);
    let (rt,): (Option<String>,) =
        sqlx::query_as("SELECT redirect_to FROM oauth_states WHERE state = ?")
            .bind(&state)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rt.as_deref(), Some("/dashboard"));
}

#[tokio::test]
async fn callback_happy_path_creates_user_and_redirects() {
    let pool = pool().await;
    let server = mock_google("sub-alice", "alice@oauth.test", Some("Alice")).await;
    let app = router_for(pool.clone(), &server);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start?return_to=/welcome"))
        .await
        .unwrap();
    let loc = res
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let state = extract_state(&loc);

    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=any-code&state={state}"
        )))
        .await
        .unwrap();
    assert!(
        res.status().is_redirection(),
        "expected redirect, got {}",
        res.status()
    );
    let loc = res
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(loc, "/welcome", "callback must honor the stored return_to");

    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("callback must set a session cookie")
        .to_str()
        .unwrap();
    assert!(set_cookie.starts_with("session="));
    assert!(set_cookie.to_lowercase().contains("httponly"));

    let (sub,): (Option<String>,) = sqlx::query_as("SELECT google_sub FROM users WHERE email = ?")
        .bind("alice@oauth.test")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sub.as_deref(), Some("sub-alice"));
}

#[tokio::test]
async fn callback_with_unknown_state_is_400() {
    let pool = pool().await;
    let server = mock_google("sub-x", "x@oauth.test", None).await;
    let app = router_for(pool.clone(), &server);

    let res = app
        .clone()
        .oneshot(get_req(
            "/auth/oauth/google/callback?code=any-code&state=never-issued",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn callback_consumes_state_so_replay_is_400() {
    let pool = pool().await;
    let server = mock_google("sub-rep", "replay@oauth.test", None).await;
    let app = router_for(pool.clone(), &server);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start"))
        .await
        .unwrap();
    let loc = res
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let state = extract_state(&loc);

    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=c&state={state}"
        )))
        .await
        .unwrap();
    assert!(res.status().is_redirection());

    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=c&state={state}"
        )))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "state must be one-shot — replaying it is a 400"
    );
}

#[tokio::test]
async fn callback_links_existing_email_user_no_duplicate() {
    let pool = pool().await;
    let server = mock_google("sub-link", "linked@oauth.test", None).await;
    let app = router_for(pool.clone(), &server);

    let _ = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/auth/register",
            &json!({
                "email": "linked@oauth.test",
                "password": "correct horse battery staple",
            }),
        ))
        .await
        .unwrap();
    let (count_before,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_before, 1);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start"))
        .await
        .unwrap();
    let state = extract_state(
        res.headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=c&state={state}"
        )))
        .await
        .unwrap();
    assert!(res.status().is_redirection());

    let (count_after,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_after, 1, "no duplicate user; the row was linked");
    let (sub,): (Option<String>,) = sqlx::query_as("SELECT google_sub FROM users WHERE email = ?")
        .bind("linked@oauth.test")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sub.as_deref(), Some("sub-link"));
}

/// Capture every body posted to the /token endpoint so we can assert
/// that `code_verifier=...` actually rides along.
struct CaptureResponder {
    tx: tokio::sync::mpsc::UnboundedSender<String>,
}

impl Respond for CaptureResponder {
    fn respond(&self, req: &WmRequest) -> ResponseTemplate {
        let body = std::str::from_utf8(&req.body).unwrap_or("").to_string();
        let _ = self.tx.send(body);
        ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "test-access-token",
            "token_type": "Bearer",
            "expires_in": 3600,
        }))
    }
}

#[tokio::test]
async fn pkce_verifier_is_sent_to_token_endpoint() {
    let pool = pool().await;
    let server = MockServer::start().await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("code_verifier="))
        .respond_with(CaptureResponder { tx })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/userinfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sub": "sub-pkce",
            "email": "pkce@oauth.test",
            "email_verified": true,
        })))
        .mount(&server)
        .await;

    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    let provider = provider_for(&server, "https://app.test/cb");
    let app = router(AppState::new(pool.clone(), key, jwt).with_oauth_google(provider));

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start"))
        .await
        .unwrap();
    let state = extract_state(
        res.headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
    );

    let (verifier,): (String,) =
        sqlx::query_as("SELECT code_verifier FROM oauth_states WHERE state = ?")
            .bind(&state)
            .fetch_one(&pool)
            .await
            .unwrap();

    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=c&state={state}"
        )))
        .await
        .unwrap();
    assert!(res.status().is_redirection());

    let captured = rx
        .recv()
        .await
        .expect("token endpoint should have been hit");
    let needle = format!("code_verifier={verifier}");
    assert!(
        captured.contains(&needle),
        "expected token-exchange body to carry {needle}; got: {captured}"
    );
    assert!(captured.contains("grant_type=authorization_code"));
}

#[tokio::test]
async fn endpoints_404_when_provider_not_configured() {
    let pool = pool().await;
    let app = router_no_oauth(pool);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(get_req("/auth/oauth/google/callback?code=c&state=s"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn callback_lands_user_in_authenticated_state() {
    // After the redirect dance, the session cookie on the response must
    // be enough to access /me.
    let pool = pool().await;
    let server = mock_google("sub-me", "me@oauth.test", Some("Me")).await;
    let app = router_for(pool, &server);

    let res = app
        .clone()
        .oneshot(get_req("/auth/oauth/google/start"))
        .await
        .unwrap();
    let state = extract_state(
        res.headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let res = app
        .clone()
        .oneshot(get_req(&format!(
            "/auth/oauth/google/callback?code=c&state={state}"
        )))
        .await
        .unwrap();
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let me = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/me")
                .header(header::COOKIE, set_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let body = read_json(me.into_body()).await;
    assert_eq!(body["email"], json!("me@oauth.test"));
}
