//! webhook-receiver — a Stripe-compatible webhook receiver demonstrating the
//! three required defenses: HMAC-SHA-256 signature verification, idempotent
//! event storage via a UNIQUE event id, and timestamp-bounded replay rejection.
//!
//! The signature format mirrors Stripe's `Stripe-Signature` header:
//!     t=<unix_timestamp>,v1=<hex(hmac_sha256(secret, "{t}.{raw_body}"))>

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use sqlx::SqlitePool;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub secret: Arc<Vec<u8>>,
    pub max_skew: Duration,
}

impl AppState {
    #[must_use]
    pub fn new(pool: SqlitePool, secret: impl Into<Vec<u8>>) -> Self {
        Self {
            pool,
            secret: Arc::new(secret.into()),
            max_skew: Duration::from_secs(5 * 60),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/webhooks/stripe", post(webhook))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("missing or malformed Stripe-Signature header")]
    BadSignatureHeader,
    #[error("signature timestamp too old or in the future")]
    StaleTimestamp,
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("body is not valid JSON")]
    BadJson,
    #[error("event payload missing required field: {0}")]
    MissingField(&'static str),
    #[error("database error")]
    Db(#[from] sqlx::Error),
}

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}

impl axum::response::IntoResponse for WebhookError {
    fn into_response(self) -> axum::response::Response {
        let (status, title, kind) = match &self {
            WebhookError::BadSignatureHeader
            | WebhookError::StaleTimestamp
            | WebhookError::SignatureMismatch => (
                StatusCode::BAD_REQUEST,
                "Bad Request",
                "https://memberclub.test/problems/webhook-signature",
            ),
            WebhookError::BadJson | WebhookError::MissingField(_) => (
                StatusCode::BAD_REQUEST,
                "Bad Request",
                "https://memberclub.test/problems/webhook-payload",
            ),
            WebhookError::Db(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "https://memberclub.test/problems/internal",
            ),
        };
        if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "webhook failed");
        } else {
            tracing::warn!(error = %self, status = %status, "webhook rejected");
        }
        let body = ProblemDetails {
            kind,
            title,
            status: status.as_u16(),
            detail: self.to_string(),
        };
        let mut resp = (status, Json(body)).into_response();
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        resp
    }
}

// ---------------------------------------------------------------------------
// Signature verification
// ---------------------------------------------------------------------------

/// Parse a Stripe-style signature header into (timestamp, signature).
pub fn parse_signature_header(header: &str) -> Option<(u64, String)> {
    let mut t = None;
    let mut v1 = None;
    for part in header.split(',') {
        let part = part.trim();
        if let Some(ts) = part.strip_prefix("t=") {
            t = ts.parse::<u64>().ok();
        } else if let Some(sig) = part.strip_prefix("v1=") {
            v1 = Some(sig.to_string());
        }
    }
    Some((t?, v1?))
}

/// Compute the expected hex signature for (timestamp, raw_body) using the secret.
pub fn compute_signature(secret: &[u8], timestamp: u64, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}

/// Constant-time hex comparison.
fn timing_safe_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify the signature header and timestamp freshness. Returns `(timestamp)` on success.
pub fn verify_signature(
    headers: &HeaderMap,
    body: &[u8],
    secret: &[u8],
    max_skew: Duration,
) -> Result<u64, WebhookError> {
    let header = headers
        .get("Stripe-Signature")
        .or_else(|| headers.get("stripe-signature"))
        .and_then(|h| h.to_str().ok())
        .ok_or(WebhookError::BadSignatureHeader)?;

    let (timestamp, provided) =
        parse_signature_header(header).ok_or(WebhookError::BadSignatureHeader)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let skew = now.abs_diff(timestamp);
    if skew > max_skew.as_secs() {
        return Err(WebhookError::StaleTimestamp);
    }

    let expected = compute_signature(secret, timestamp, body);
    if !timing_safe_eq(&expected, &provided) {
        return Err(WebhookError::SignatureMismatch);
    }
    Ok(timestamp)
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// What we extract from the payload before storing. We don't deserialize the
/// whole Stripe schema — just the fields we need to make the row.
#[derive(Debug)]
pub struct EventMeta {
    pub stripe_event_id: String,
    pub event_type: String,
    pub created: i64,
}

pub fn parse_event_meta(body: &[u8]) -> Result<EventMeta, WebhookError> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| WebhookError::BadJson)?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or(WebhookError::MissingField("id"))?
        .to_string();
    let event_type = v
        .get("type")
        .and_then(|x| x.as_str())
        .ok_or(WebhookError::MissingField("type"))?
        .to_string();
    let created = v
        .get("created")
        .and_then(|x| x.as_i64())
        .ok_or(WebhookError::MissingField("created"))?;
    Ok(EventMeta {
        stripe_event_id: id,
        event_type,
        created,
    })
}

/// Store the event idempotently. Returns `Some(row_id)` if newly inserted,
/// `None` if the event was already stored (duplicate).
pub async fn store_event(
    pool: &SqlitePool,
    meta: &EventMeta,
    payload: &[u8],
) -> Result<Option<i64>, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        "INSERT INTO stripe_events (stripe_event_id, event_type, created_at_stripe, payload)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (stripe_event_id) DO NOTHING
         RETURNING id",
    )
    .bind(&meta.stripe_event_id)
    .bind(&meta.event_type)
    .bind(meta.created)
    .bind(std::str::from_utf8(payload).unwrap_or(""))
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Look up an already-stored event that has **not** yet been marked
/// processed, returning its row id (or `None` if it was fully processed).
///
/// This is what makes the receiver crash-safe. Storing the receipt row and
/// running the side effect are two steps, and a crash can land between them.
/// Because Stripe delivers at-least-once, the next delivery must *resume* an
/// unprocessed row rather than treat it as a finished duplicate — otherwise
/// the side effect is lost forever while Stripe sees a 200 and stops retrying.
pub async fn pending_event_id(
    pool: &SqlitePool,
    stripe_event_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM stripe_events
         WHERE stripe_event_id = ? AND processed_at IS NULL",
    )
    .bind(stripe_event_id)
    .fetch_optional(pool)
    .await
}

/// Mark the event processed.
pub async fn mark_processed(pool: &SqlitePool, row_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE stripe_events SET processed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(row_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn webhook(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, WebhookError> {
    // 1. Signature first — never parse before verifying.
    verify_signature(&headers, &body, &s.secret, s.max_skew)?;

    // 2. Parse just enough metadata.
    let meta = parse_event_meta(&body)?;

    // 3. Idempotent insert. A fresh event gives us a new row id. A conflict
    //    means we've stored this id before — but "stored" is not "processed":
    //    a prior delivery may have crashed between the insert and step 5, so
    //    we resume an unprocessed row instead of dropping the side effect.
    let row_id = if let Some(id) = store_event(&s.pool, &meta, &body).await? {
        id
    } else if let Some(id) = pending_event_id(&s.pool, &meta.stripe_event_id).await? {
        // Stored but never finished — a previous attempt crashed. Resume it.
        id
    } else {
        // Already fully processed: ack the retry so Stripe stops resending.
        tracing::info!(event_id = %meta.stripe_event_id, "duplicate event ignored");
        return Ok(StatusCode::OK);
    };

    // 4. Handle the event. This MUST be idempotent: because step 5 runs after
    //    it, a crash in between means the next delivery re-runs this handler.
    //    In real life this dispatches by type; here we just acknowledge —
    //    Phase 8 lessons cover the dispatching.
    tracing::info!(event_id = %meta.stripe_event_id, event_type = %meta.event_type, "processing");

    // 5. Mark processed — only now is the event considered done.
    mark_processed(&s.pool, row_id).await?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Migration helper
// ---------------------------------------------------------------------------

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
