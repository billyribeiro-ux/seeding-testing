//! Phase 8 — Stripe billing for MemberClub.
//!
//! Three handlers under `/v1/billing/*` plus a webhook receiver under
//! `/webhooks/stripe` (mounted outside `/v1` so a future rate-limit on
//! `/v1` doesn't throttle Stripe). The handlers follow the rules from
//! `docs/01-architecture-decisions/0008-stripe-is-the-rail.md`:
//!
//!   * Our DB is the source of truth. Every event we care about gets
//!     mirrored. The webhook is the only mutator.
//!   * Money is `(i64 cents, 3-letter currency)`. Never a float.
//!   * Every webhook delivery is logged in `stripe_events` BEFORE we
//!     touch any other table. Replay → no-op (idempotent).
//!
//! The handler does NOT call out to Stripe at request-time; that would
//! pin user-facing latency to Stripe's availability. Read paths (`GET
//! /v1/me/subscription`) read the mirrored DB row.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use sqlx::SqlitePool;

use crate::auth::AuthenticatedUser;
use crate::{ApiError, AppState};

/// Max age the Stripe-Signature timestamp is allowed to be before we
/// reject as a replay. 5 minutes mirrors Stripe's own SDK default.
const REPLAY_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Returned to the SvelteKit form action that initiates Checkout.
#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutBody {
    /// Stripe Price id (e.g. `price_pro_monthly`). Validated server-
    /// side so the client can't pick a price we don't sell.
    pub price_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PortalBody {
    /// Where Stripe should send the user after they close the portal.
    pub return_url: String,
}

#[derive(Debug, Serialize)]
pub struct PortalResponse {
    pub url: String,
}

/// `POST /v1/billing/checkout` — create a Checkout Session for the
/// authenticated user.
///
/// In production this would call `POST https://api.stripe.com/v1/checkout/sessions`
/// with the user's mirrored `stripe_customer_id` (creating one first
/// if absent). The dev/test build below returns a deterministic
/// placeholder URL so integration tests don't need an internet
/// connection. The shape of the response — `{"url": ...}` — matches
/// production exactly.
pub async fn create_checkout(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Json(body): Json<CheckoutBody>,
) -> Result<Json<CheckoutResponse>, ApiError> {
    if !allowed_price(&body.price_id) {
        return Err(ApiError::BadRequest(format!(
            "unknown price_id: {}",
            body.price_id
        )));
    }

    // Ensure we have a stripe_customer_id mirrored. Production would
    // call POST /v1/customers if missing; tests use a deterministic
    // placeholder.
    let stripe_customer_id = ensure_stripe_customer(&s.pool, user.0.id, &user.0.email).await?;

    // Production: HTTP call to Stripe. Dev/test: stable placeholder.
    let url = format!(
        "https://checkout.stripe.test/c/pay/{stripe_customer_id}_{}",
        body.price_id
    );
    Ok(Json(CheckoutResponse { url }))
}

/// `POST /v1/billing/portal` — Customer Portal URL for self-serve
/// (cancel, update payment method, view invoices).
pub async fn create_portal(
    State(s): State<AppState>,
    user: AuthenticatedUser,
    Json(body): Json<PortalBody>,
) -> Result<Json<PortalResponse>, ApiError> {
    let stripe_customer_id = ensure_stripe_customer(&s.pool, user.0.id, &user.0.email).await?;
    let url = format!(
        "https://billing.stripe.test/p/session/{stripe_customer_id}?return_to={}",
        body.return_url
    );
    Ok(Json(PortalResponse { url }))
}

/// `POST /webhooks/stripe` — receive an event.
///
/// 1. Verify the `Stripe-Signature` header (timestamp + HMAC-SHA-256).
/// 2. Parse the JSON body, pull out `id` + `type`.
/// 3. INSERT into `stripe_events` (ON CONFLICT DO NOTHING). If the
///    row already existed, return 200 immediately — we've seen it.
/// 4. Dispatch by `type`: mirror the relevant fields into our tables.
/// 5. Mark `processed_at` and commit.
///
/// All of step 3+4+5 is one transaction so a partial mirror never lands.
pub async fn webhook(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("missing Stripe-Signature".into()))?;

    let secret = s
        .stripe_webhook_secret
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("webhook receiver is disabled (no secret)".into()))?;

    if let Err(e) = verify_signature(sig, &body, secret, REPLAY_WINDOW) {
        tracing::warn!(error = %e, "stripe webhook signature rejected");
        return Err(ApiError::Unauthorized);
    }

    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("invalid JSON: {e}")))?;
    let event_id = parsed
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("event has no id".into()))?
        .to_string();
    let event_type = parsed
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("event has no type".into()))?
        .to_string();

    let mut tx = s.pool.begin().await?;

    // Idempotency gate. If the event_id is already in `stripe_events`,
    // the INSERT does nothing and `rows_affected() == 0` — we exit
    // 200 OK without touching anything else.
    let inserted = sqlx::query(
        "INSERT INTO stripe_events (stripe_event_id, event_type) VALUES (?, ?)
         ON CONFLICT(stripe_event_id) DO NOTHING",
    )
    .bind(&event_id)
    .bind(&event_type)
    .execute(&mut *tx)
    .await?;

    if inserted.rows_affected() == 0 {
        tx.commit().await?;
        tracing::info!(%event_id, %event_type, "stripe event replayed; no-op");
        return Ok(StatusCode::OK);
    }

    // Mirror the event into our tables. We support exactly the events
    // the curriculum names; anything else is logged but not processed.
    match event_type.as_str() {
        "checkout.session.completed" => mirror_checkout_completed(&mut tx, &parsed).await?,
        "customer.subscription.created"
        | "customer.subscription.updated"
        | "customer.subscription.deleted" => mirror_subscription(&mut tx, &parsed).await?,
        "invoice.payment_succeeded" | "invoice.payment_failed" => {
            mirror_invoice(&mut tx, &parsed).await?;
        }
        other => {
            tracing::info!(event_type = %other, "stripe event not handled; recorded only");
        }
    }

    sqlx::query(
        "UPDATE stripe_events SET processed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE stripe_event_id = ?",
    )
    .bind(&event_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(StatusCode::OK)
}

/// Return the router with `/v1/billing/*` + the public `/webhooks/stripe`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/billing/checkout", post(create_checkout))
        .route("/v1/billing/portal", post(create_portal))
        .route("/webhooks/stripe", post(webhook))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The whitelist of Stripe Price ids we're willing to checkout. In
/// production this would be loaded from config or a database table;
/// here it's a static slice so a typo on the client can't silently
/// charge for the wrong plan.
fn allowed_price(price_id: &str) -> bool {
    matches!(
        price_id,
        "price_pro_monthly"
            | "price_pro_yearly"
            | "price_elite_monthly"
            | "price_elite_yearly"
            | "price_lifetime"
    )
}

async fn ensure_stripe_customer(
    pool: &SqlitePool,
    user_id: i64,
    email: &str,
) -> Result<String, ApiError> {
    // Look up an existing mirror first.
    let row: Option<(String,)> =
        sqlx::query_as("SELECT stripe_customer_id FROM stripe_customers WHERE user_id = ? LIMIT 1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    if let Some((id,)) = row {
        return Ok(id);
    }
    // Mint a deterministic placeholder. Production would replace this
    // call with `POST https://api.stripe.com/v1/customers` and use the
    // returned id.
    let id = format!("cus_test_{user_id}_{}", short_hash(email));
    sqlx::query("INSERT INTO stripe_customers (user_id, stripe_customer_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(&id)
        .execute(pool)
        .await?;
    Ok(id)
}

fn short_hash(s: &str) -> String {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let digest = h.finalize();
    hex::encode(&digest[..6])
}

// ---------------------------------------------------------------------------
// Event mirrors
// ---------------------------------------------------------------------------

async fn mirror_checkout_completed(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &Value,
) -> Result<(), ApiError> {
    // We expect `data.object` to be a Checkout Session with `customer`
    // and `metadata.user_id`. The metadata is set when WE create the
    // session, so we know it's there.
    let obj = event
        .pointer("/data/object")
        .ok_or_else(|| ApiError::BadRequest("event has no data.object".into()))?;
    let customer = obj
        .get("customer")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("checkout session has no customer".into()))?;
    let user_id = obj
        .pointer("/metadata/user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| ApiError::BadRequest("checkout session has no metadata.user_id".into()))?;
    // Idempotent mirror — if the customer row already exists from an
    // earlier event, this is a no-op.
    sqlx::query(
        "INSERT INTO stripe_customers (user_id, stripe_customer_id) VALUES (?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(customer)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn mirror_subscription(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &Value,
) -> Result<(), ApiError> {
    let obj = event
        .pointer("/data/object")
        .ok_or_else(|| ApiError::BadRequest("subscription event has no data.object".into()))?;
    let sub_id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("subscription has no id".into()))?;
    let customer = obj
        .get("customer")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("subscription has no customer".into()))?;
    let status = obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let plan_id = obj
        .pointer("/items/data/0/price/id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let cancel = obj
        .get("cancel_at_period_end")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let period_end = obj
        .get("current_period_end")
        .and_then(serde_json::Value::as_i64)
        .and_then(|s| DateTime::<Utc>::from_timestamp(s, 0))
        .map(|d| d.to_rfc3339());

    // Find the user via the mirrored customer table.
    let user_id: Option<(i64,)> =
        sqlx::query_as("SELECT user_id FROM stripe_customers WHERE stripe_customer_id = ?")
            .bind(customer)
            .fetch_optional(&mut **tx)
            .await?;
    let user_id = user_id
        .ok_or_else(|| ApiError::BadRequest("subscription customer is unknown to us".into()))?
        .0;

    sqlx::query(
        "INSERT INTO stripe_subscriptions
            (stripe_subscription_id, user_id, plan_id, status, current_period_end, cancel_at_period_end)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(stripe_subscription_id) DO UPDATE SET
            status = excluded.status,
            plan_id = excluded.plan_id,
            current_period_end = excluded.current_period_end,
            cancel_at_period_end = excluded.cancel_at_period_end,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(sub_id)
    .bind(user_id)
    .bind(plan_id)
    .bind(status)
    .bind(period_end)
    .bind(i64::from(cancel))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn mirror_invoice(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &Value,
) -> Result<(), ApiError> {
    let obj = event
        .pointer("/data/object")
        .ok_or_else(|| ApiError::BadRequest("invoice event has no data.object".into()))?;
    let invoice_id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("invoice has no id".into()))?;
    let customer = obj
        .get("customer")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("invoice has no customer".into()))?;
    let amount_cents = obj
        .get("amount_paid")
        .and_then(serde_json::Value::as_i64)
        .or_else(|| obj.get("amount_due").and_then(serde_json::Value::as_i64))
        .unwrap_or(0);
    let currency = obj
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("usd")
        .to_uppercase();
    let status = obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let hosted_url = obj
        .get("hosted_invoice_url")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let user_id: Option<(i64,)> =
        sqlx::query_as("SELECT user_id FROM stripe_customers WHERE stripe_customer_id = ?")
            .bind(customer)
            .fetch_optional(&mut **tx)
            .await?;
    let user_id = user_id
        .ok_or_else(|| ApiError::BadRequest("invoice customer is unknown to us".into()))?
        .0;

    sqlx::query(
        "INSERT INTO stripe_invoices
            (stripe_invoice_id, user_id, amount_cents, currency, status, hosted_invoice_url)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(stripe_invoice_id) DO UPDATE SET
            amount_cents = excluded.amount_cents,
            currency = excluded.currency,
            status = excluded.status,
            hosted_invoice_url = excluded.hosted_invoice_url",
    )
    .bind(invoice_id)
    .bind(user_id)
    .bind(amount_cents)
    .bind(&currency)
    .bind(status)
    .bind(hosted_url)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Signature verification — mirror of projects/07-webhook-receiver
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
enum SigError {
    #[error("header malformed: missing t= or v1=")]
    MalformedHeader,
    #[error("timestamp out of range")]
    StaleTimestamp,
    #[error("signature does not match")]
    Mismatch,
}

/// Verify the `Stripe-Signature` header.
///
/// The header is `t=<unix-ts>,v1=<hex-hmac>[,v1=<hex-hmac>]…`. We:
///   1. Parse `t` and `v1` values.
///   2. Reject if `now - t > window` (replay defense).
///   3. Compute HMAC-SHA-256 of `<t>.<raw-body>` with the secret.
///   4. Constant-time-compare against the supplied `v1` values
///      (Stripe rotates signing secrets by sending both old + new).
fn verify_signature(
    header: &str,
    body: &[u8],
    secret: &str,
    window: std::time::Duration,
) -> Result<(), SigError> {
    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<String> = Vec::new();
    for part in header.split(',') {
        let (k, v) = part.split_once('=').ok_or(SigError::MalformedHeader)?;
        match k.trim() {
            "t" => {
                timestamp = Some(v.trim().parse().map_err(|_| SigError::MalformedHeader)?);
            }
            "v1" => {
                signatures.push(v.trim().to_string());
            }
            _ => {}
        }
    }
    let t = timestamp.ok_or(SigError::MalformedHeader)?;
    if signatures.is_empty() {
        return Err(SigError::MalformedHeader);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SigError::MalformedHeader)?
        .as_secs() as i64;
    if (now - t).unsigned_abs() > window.as_secs() {
        return Err(SigError::StaleTimestamp);
    }

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| SigError::MalformedHeader)?;
    mac.update(format!("{t}.").as_bytes());
    mac.update(body);
    let want = mac.finalize().into_bytes();

    for sig in &signatures {
        if let Ok(bytes) = hex::decode(sig)
            && bool::from(subtle::ConstantTimeEq::ct_eq(
                want.as_slice(),
                bytes.as_slice(),
            ))
        {
            return Ok(());
        }
    }
    Err(SigError::Mismatch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &str, t: i64, body: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{t}.").as_bytes());
        mac.update(body.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("t={t},v1={sig}")
    }

    fn now_unix() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    #[test]
    fn valid_signature_passes() {
        let t = now_unix();
        let body = r#"{"id":"evt_1","type":"x"}"#;
        let header = sign("whsec_test", t, body);
        verify_signature(&header, body.as_bytes(), "whsec_test", REPLAY_WINDOW).unwrap();
    }

    #[test]
    fn tampered_body_fails() {
        let t = now_unix();
        let header = sign("whsec_test", t, "original");
        assert!(matches!(
            verify_signature(&header, b"tampered", "whsec_test", REPLAY_WINDOW),
            Err(SigError::Mismatch)
        ));
    }

    #[test]
    fn stale_timestamp_fails() {
        let t = now_unix() - 60 * 60; // 1 hour ago
        let header = sign("whsec_test", t, "{}");
        assert!(matches!(
            verify_signature(&header, b"{}", "whsec_test", REPLAY_WINDOW),
            Err(SigError::StaleTimestamp)
        ));
    }

    #[test]
    fn malformed_header_fails() {
        assert!(matches!(
            verify_signature("garbage", b"{}", "whsec_test", REPLAY_WINDOW),
            Err(SigError::MalformedHeader)
        ));
    }

    #[test]
    fn allowed_price_whitelist_only() {
        assert!(allowed_price("price_pro_monthly"));
        assert!(allowed_price("price_lifetime"));
        assert!(!allowed_price("price_free"));
        assert!(!allowed_price("'; DROP TABLE users; --"));
    }
}
