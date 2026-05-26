//! Integration tests for the Phase 6 stretch — RS256 JWT + JWKS endpoint
//! (E6.5).
//!
//! What we prove:
//!
//!   1. `GET /.well-known/jwks.json` returns 200 with a well-formed
//!      JWKS body (one RSA key, RS256, `use=sig`, populated `kid`/`n`/`e`).
//!   2. When the RS256 signer is NOT configured, the endpoint still
//!      returns 200 with `{"keys":[]}` — clients can read the document
//!      unconditionally.
//!   3. A token issued by the same `JwtRs256` instance can be verified
//!      end-to-end by parsing the JWK published at `/.well-known/jwks.json`
//!      (round-trip via the public-key bytes published on the wire).
//!   4. The token header carries the matching `kid`.
//!
//! Hermetic: the same `JwtRs256` instance is shared across these tests
//! via `OnceLock` because RSA-2048 keygen is ~hundred-ms expensive.

use std::sync::OnceLock;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum_extra::extract::cookie::Key;
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde_json::Value;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

use auth_demo::{AppState, jwt::Jwt, jwt_rs256::JwtRs256, migrate, router};

fn shared_rs256() -> JwtRs256 {
    static SHARED: OnceLock<JwtRs256> = OnceLock::new();
    SHARED
        .get_or_init(|| JwtRs256::generate().expect("RSA keygen"))
        .clone()
}

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

fn router_with_rs256(pool: SqlitePool, rs256: Option<JwtRs256>) -> axum::Router {
    let cookie_key = Key::generate();
    let hs_jwt = Jwt::new(&Jwt::random_secret());
    let mut state = AppState::new(pool, cookie_key, hs_jwt);
    if let Some(rs) = rs256 {
        state = state.with_jwt_rs256(rs);
    }
    router(state)
}

async fn read_json(b: Body) -> Value {
    let bytes = to_bytes(b, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn jwks_endpoint_publishes_one_rsa_key_when_signer_is_configured() {
    let app = router_with_rs256(pool().await, Some(shared_rs256()));

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = read_json(res.into_body()).await;
    let keys = body["keys"].as_array().expect("`keys` must be an array");
    assert_eq!(keys.len(), 1, "one active key");

    let k = &keys[0];
    assert_eq!(k["kty"], "RSA");
    assert_eq!(k["alg"], "RS256");
    assert_eq!(k["use"], "sig");
    assert_eq!(k["kid"], shared_rs256().kid());
    // `n` and `e` must be base64url-no-pad, non-empty.
    let n = k["n"].as_str().unwrap();
    let e = k["e"].as_str().unwrap();
    assert!(!n.is_empty());
    assert!(!e.is_empty());
    for (label, v) in [("n", n), ("e", e)] {
        assert!(
            v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "{label} must be base64url-no-pad: {v}"
        );
    }
}

#[tokio::test]
async fn jwks_endpoint_returns_empty_set_when_signer_is_not_configured() {
    // No RS256 attached → keys: []. Endpoint still answers 200 so the
    // verifier can read it unconditionally.
    let app = router_with_rs256(pool().await, None);

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = read_json(res.into_body()).await;
    assert_eq!(
        body["keys"].as_array().unwrap().len(),
        0,
        "empty key set when no RS256 signer is configured"
    );
}

#[tokio::test]
async fn rs256_token_round_trips_via_the_published_jwks() {
    // The full "another service verifies our JWT" flow:
    //   - our service issues an RS256 token.
    //   - a downstream verifier (this test) reads JWKS.
    //   - it extracts (n, e), reconstructs the public key, and verifies.
    let rs256 = shared_rs256();
    let app = router_with_rs256(pool().await, Some(rs256.clone()));

    let token = rs256.issue_access(42).unwrap();
    let header = decode_header(&token).unwrap();
    let token_kid = header.kid.expect("RS256 issuer must set kid");

    // Pull JWKS from the wire.
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = read_json(res.into_body()).await;
    let jwk = body["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["kid"] == token_kid)
        .expect("JWKS must publish the kid the token was signed with");

    // Decode n, e and rebuild the public key.
    let n_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(jwk["n"].as_str().unwrap())
        .unwrap();
    let e_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(jwk["e"].as_str().unwrap())
        .unwrap();

    // Build a DecodingKey straight from the JWK components — this is
    // exactly what a downstream verifier would do.
    let dec = DecodingKey::from_rsa_raw_components(&n_bytes, &e_bytes);
    let mut v = Validation::new(Algorithm::RS256);
    v.set_audience(&["auth-demo-api"]);
    v.set_issuer(&["auth-demo"]);
    v.leeway = 60;

    let claims = decode::<auth_demo::jwt::Claims>(&token, &dec, &v)
        .unwrap()
        .claims;
    assert_eq!(claims.sub, "42");
    assert_eq!(claims.purpose, "access");
}
