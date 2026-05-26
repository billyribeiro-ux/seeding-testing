//! Integration tests for the notes surface — CRUD + policy gating.
//!
//! Each test owns its own in-memory SQLite pool. We register users via the
//! public API, then promote them (role / tier / org_id) by writing directly
//! to the pool — there is intentionally no public endpoint to change a
//! user's role/tier/tenant from outside, so tests reach in via sqlx.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum_extra::extract::cookie::Key;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use memberclub_api::{AppState, auth::Jwt, migrate, router};

struct Harness {
    app: axum::Router,
    pool: SqlitePool,
}

async fn harness() -> Harness {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let key = Key::generate();
    let jwt = Jwt::new(&Jwt::random_secret());
    let app = router(AppState::new(pool.clone(), key, jwt));
    Harness { app, pool }
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

fn bearer(method: &str, uri: &str, token: &str, body: Option<&Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = if let Some(v) = body {
        b = b.header(header::CONTENT_TYPE, "application/json");
        Body::from(v.to_string())
    } else {
        Body::empty()
    };
    b.body(body).unwrap()
}

/// Register a user via the public API and return (user_id, access_token).
async fn register(app: &axum::Router, email: &str) -> (i64, String) {
    let res = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/v1/auth/register",
            &json!({"email": email, "password": "correct horse battery staple"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = read_json(res.into_body()).await;
    let id = body["user"]["id"].as_i64().unwrap();
    let token = body["access_token"].as_str().unwrap().to_string();
    (id, token)
}

async fn set_role(pool: &SqlitePool, user_id: i64, role: &str) {
    sqlx::query("UPDATE users SET role = ? WHERE id = ?")
        .bind(role)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn set_org(pool: &SqlitePool, user_id: i64, org_id: i64) {
    sqlx::query("UPDATE users SET org_id = ? WHERE id = ?")
        .bind(org_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn notes_crud_happy_path() {
    let h = harness().await;
    let (_uid, token) = register(&h.app, "owner@example.com").await;

    // Create.
    let res = h
        .app
        .clone()
        .oneshot(bearer(
            "POST",
            "/v1/notes",
            &token,
            Some(&json!({"body": "first note"})),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = read_json(res.into_body()).await;
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["body"], "first note");

    // List.
    let res = h
        .app
        .clone()
        .oneshot(bearer("GET", "/v1/notes", &token, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = read_json(res.into_body()).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Get.
    let res = h
        .app
        .clone()
        .oneshot(bearer("GET", &format!("/v1/notes/{id}"), &token, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Patch.
    let res = h
        .app
        .clone()
        .oneshot(bearer(
            "PATCH",
            &format!("/v1/notes/{id}"),
            &token,
            Some(&json!({"body": "updated"})),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated = read_json(res.into_body()).await;
    assert_eq!(updated["body"], "updated");

    // Delete.
    let res = h
        .app
        .clone()
        .oneshot(bearer("DELETE", &format!("/v1/notes/{id}"), &token, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Subsequent get → 404.
    let res = h
        .app
        .oneshot(bearer("GET", &format!("/v1/notes/{id}"), &token, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Policy gating
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cannot_read_another_users_unpublished_note() {
    let h = harness().await;
    let (_owner_id, owner_token) = register(&h.app, "owner@example.com").await;
    let (_other_id, other_token) = register(&h.app, "other@example.com").await;

    // Owner creates a *draft* (unpublished) note.
    let res = h
        .app
        .clone()
        .oneshot(bearer(
            "POST",
            "/v1/notes",
            &owner_token,
            Some(&json!({"body": "secret draft"})),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let id = read_json(res.into_body()).await["id"].as_i64().unwrap();

    // The other user (same tenant, not the owner) cannot read it.
    let res = h
        .app
        .oneshot(bearer(
            "GET",
            &format!("/v1/notes/{id}"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let body = read_json(res.into_body()).await;
    assert_eq!(body["status"], 403);
}

#[tokio::test]
async fn admin_can_read_any_note() {
    let h = harness().await;
    let (_owner_id, owner_token) = register(&h.app, "owner@example.com").await;
    let (admin_id, admin_token) = register(&h.app, "admin@example.com").await;
    set_role(&h.pool, admin_id, "admin").await;

    let res = h
        .app
        .clone()
        .oneshot(bearer(
            "POST",
            "/v1/notes",
            &owner_token,
            Some(&json!({"body": "draft only owner should normally see"})),
        ))
        .await
        .unwrap();
    let id = read_json(res.into_body()).await["id"].as_i64().unwrap();

    // Admin reads even another user's draft.
    let res = h
        .app
        .oneshot(bearer(
            "GET",
            &format!("/v1/notes/{id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn cross_tenant_access_blocked() {
    let h = harness().await;
    let (_owner_id, owner_token) = register(&h.app, "tenant1@example.com").await;
    let (other_id, other_token) = register(&h.app, "tenant2@example.com").await;

    // Move the second user to a different tenant.
    set_org(&h.pool, other_id, 99).await;

    // The owner creates a *published* note in tenant 1.
    let res = h
        .app
        .clone()
        .oneshot(bearer(
            "POST",
            "/v1/notes",
            &owner_token,
            Some(&json!({"body": "tenant 1 broadcast", "published": true})),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let id = read_json(res.into_body()).await["id"].as_i64().unwrap();

    // Tenant-2 user is blocked, even though the note is published.
    let res = h
        .app
        .oneshot(bearer(
            "GET",
            &format!("/v1/notes/{id}"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
