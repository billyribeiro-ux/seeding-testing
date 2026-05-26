//! Phase 4 — E4.7: `OpenAPI` 3 spec snapshot.
//!
//! The `/openapi.json` route serves a utoipa-derived document. We
//! snapshot it so any drift in the public API surface (a new route,
//! a renamed field, a different status code) fails CI until somebody
//! consciously re-blesses the snapshot via `cargo insta accept`.
//!
//! Why this matters: clients consume the spec to generate typed
//! bindings. A silent change is the worst kind — a stub broke and
//! nobody knew until customers complained.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use notes_api::{AppState, router};

async fn app() -> axum::Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx_notes::migrate(&pool).await.unwrap();
    router(AppState::new(pool))
}

#[tokio::test]
async fn openapi_json_endpoint_responds_200_with_valid_spec() {
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();

    // OpenAPI 3 root invariants — these would catch a broken doc even
    // before we look at the snapshot.
    assert!(
        doc["openapi"].as_str().is_some_and(|v| v.starts_with("3.")),
        "openapi field must declare a 3.x version, got {}",
        doc["openapi"]
    );
    assert_eq!(doc["info"]["title"], "notes-api");

    // Every CRUD route we annotated must show up.
    let paths = doc["paths"].as_object().unwrap();
    for required in ["/v1/notes", "/v1/notes/{id}"] {
        assert!(
            paths.contains_key(required),
            "OpenAPI doc is missing path {required}; got paths={:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }

    // Every DTO we registered must be in components.schemas.
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    for required in [
        "NoteDto",
        "NotesPage",
        "CreateBody",
        "UpdateBody",
        "ProblemDetails",
    ] {
        assert!(
            schemas.contains_key(required),
            "OpenAPI doc is missing schema {required}; got {:?}",
            schemas.keys().collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn openapi_spec_matches_committed_snapshot() {
    // The spec is generated at compile-time from utoipa annotations;
    // building it doesn't need a running server, but driving it
    // through the router gives us end-to-end coverage.
    let res = app()
        .await
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let mut doc: Value = serde_json::from_slice(&bytes).unwrap();

    // Strip the version from `info` so this test isn't a treadmill of
    // re-blessing every time the crate version bumps.
    if let Some(info) = doc["info"].as_object_mut() {
        info.remove("version");
    }

    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!(doc);
    });
}
