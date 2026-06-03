//! Phase 6 stretch E6.8 — OAuth 2.0 authorization-code flow (Google).
//!
//! This module is the server-side half of "Login with Google."
//! It implements the standard OAuth 2.0 authorization-code grant with
//! PKCE (RFC 7636) and the CSRF `state` parameter that pairs the
//! `/start` and `/callback` legs of the redirect dance.
//!
//! ## Lifecycle
//!
//! 1. `/auth/oauth/google/start` — the client (browser) hits this with
//!    an optional `return_to`. We generate a fresh `state` (32 random
//!    bytes, base64url, no padding) AND a PKCE `code_verifier` /
//!    `code_challenge` pair (S256). The (state, verifier, return_to)
//!    tuple is persisted in `oauth_states` with a short TTL, and the
//!    user is 302'd to Google's authorize endpoint with the challenge
//!    embedded.
//! 2. `/auth/oauth/google/callback?code=...&state=...` — Google has
//!    just redirected back. We look up the `state` row (and atomically
//!    consume it so it can't be replayed), then exchange the `code` at
//!    Google's token endpoint, sending the original `code_verifier`
//!    along. Google answers with an `access_token` (and usually an
//!    `id_token`); we resolve the user identity by hitting Google's
//!    userinfo endpoint with the access token.
//! 3. With `{sub, email}` in hand we **find or create** a user keyed by
//!    `google_sub`. If a password-only user with the same email already
//!    exists we *link* the Google sub to that row — no duplicate
//!    accounts. The caller's session cookie + JWT pair is the same
//!    shape `login` returns, so the SvelteKit client doesn't need to
//!    branch on auth method.
//!
//! ## Testability
//!
//! The two HTTP endpoints Google exposes — the discovery document and
//! the token URL — are injectable on [`GoogleProvider::new`] so the
//! integration tests can point at a wiremock `MockServer` that pretends
//! to be Google. The default constructor uses Google's real production
//! endpoints.

use base64::Engine;
use rand::TryRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The userinfo we need from an OAuth provider after a successful
/// code-exchange. We deliberately keep this narrow: anything richer
/// (avatar URL, locale, etc.) is the application's call — these four
/// fields are what `find_or_create_user` needs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct OauthUserInfo {
    /// The provider's durable, opaque user id (Google's `sub`). NEVER
    /// the email — emails change, this does not.
    pub sub: String,
    /// The user's primary email at the provider.
    pub email: String,
    /// Did the provider verify ownership of `email`? We trust this
    /// signal for account-linking on Google (their flow guarantees a
    /// verified email exists before the consent screen completes).
    pub email_verified: bool,
    /// Display name, if the provider returned one. Not required.
    pub name: Option<String>,
}

/// The provider interface — async by design so a real network round-trip
/// to Google's token endpoint slots in cleanly, and easy to mock in tests
/// without an HTTP transport at all.
#[allow(async_fn_in_trait)]
pub trait OauthProvider: Send + Sync {
    /// Build the authorize URL the user is redirected to. The
    /// `code_verifier` is hashed into a `code_challenge` (S256) and
    /// embedded; `state` is the CSRF token the provider will echo back
    /// to us on callback.
    fn authorize_url(&self, state: &str, code_verifier: &str) -> String;

    /// Exchange the authorization `code` for the user's identity.
    /// Implementations send `code_verifier` alongside `code` so the
    /// token endpoint can confirm the requester is the same party that
    /// initiated the flow.
    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OauthUserInfo, OauthError>;
}

/// Anything that can go wrong end-to-end in the OAuth dance. We map
/// these to a generic 400 / 401 in the HTTP layer — never echoing the
/// underlying message to the wire, which would help an attacker probe
/// the integration.
#[derive(Debug, thiserror::Error)]
pub enum OauthError {
    #[error("HTTP transport error: {0}")]
    Transport(String),
    #[error("provider rejected token exchange (HTTP {0})")]
    TokenExchange(u16),
    #[error("provider response was not valid JSON: {0}")]
    BadJson(String),
    #[error("provider response missing required field: {0}")]
    MissingField(&'static str),
}

/// The concrete Google implementation.
///
/// `discovery_url` and `token_url` are injectable so tests can swap in
/// a wiremock server. In production the defaults are Google's real
/// endpoints. We don't actually FETCH the discovery document in this
/// implementation — we already know Google's authorize, token, and
/// userinfo URLs — but holding the discovery URL means a forward-compat
/// path is open AND tests can prove we point at the right hostname.
#[derive(Debug, Clone)]
pub struct GoogleProvider {
    /// Google's authorize endpoint. Test-overridable via the constructor.
    pub authorize_url: String,
    /// Google's token endpoint. The most important override hook for
    /// tests — wiremock answers here with a canned access token.
    pub token_url: String,
    /// Google's userinfo endpoint. Wiremock also serves a canned
    /// `{sub, email, email_verified, name}` here in tests.
    pub userinfo_url: String,
    /// `discovery_url` is kept for completeness (and to make the
    /// constructor signature match the task description), even though
    /// we don't fetch it at runtime. Tests inject the wiremock base URL
    /// here for symmetry.
    pub discovery_url: String,
    /// Our app's OAuth client_id (registered with Google's console).
    pub client_id: String,
    /// Our app's OAuth client_secret. Confidential — never logged.
    pub client_secret: String,
    /// The redirect URI we registered. Google rejects the exchange if
    /// this doesn't match the one we sent on /authorize.
    pub redirect_uri: String,
    /// reqwest client. Reused so we get connection pooling.
    http: reqwest::Client,
}

impl GoogleProvider {
    /// Google's well-known endpoints — May 2026. The discovery document
    /// at `https://accounts.google.com/.well-known/openid-configuration`
    /// publishes these; we hard-code them rather than fetch on every
    /// boot to keep the cold start dependency-free.
    pub const DEFAULT_DISCOVERY_URL: &'static str =
        "https://accounts.google.com/.well-known/openid-configuration";
    pub const DEFAULT_AUTHORIZE_URL: &'static str = "https://accounts.google.com/o/oauth2/v2/auth";
    pub const DEFAULT_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    pub const DEFAULT_USERINFO_URL: &'static str =
        "https://openidconnect.googleapis.com/v1/userinfo";

    /// Construct a Google provider pointed at the **production**
    /// endpoints. Use `with_endpoints` to override for tests.
    #[must_use]
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            authorize_url: Self::DEFAULT_AUTHORIZE_URL.to_string(),
            token_url: Self::DEFAULT_TOKEN_URL.to_string(),
            userinfo_url: Self::DEFAULT_USERINFO_URL.to_string(),
            discovery_url: Self::DEFAULT_DISCOVERY_URL.to_string(),
            client_id,
            client_secret,
            redirect_uri,
            http: reqwest::Client::new(),
        }
    }

    /// Test-only constructor: override every endpoint. Used by
    /// `tests/oauth.rs` to point at a wiremock `MockServer`.
    #[must_use]
    pub fn with_endpoints(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        discovery_url: String,
        authorize_url: String,
        token_url: String,
        userinfo_url: String,
    ) -> Self {
        Self {
            authorize_url,
            token_url,
            userinfo_url,
            discovery_url,
            client_id,
            client_secret,
            redirect_uri,
            http: reqwest::Client::new(),
        }
    }
}

impl OauthProvider for GoogleProvider {
    fn authorize_url(&self, state: &str, code_verifier: &str) -> String {
        // S256 PKCE: code_challenge = base64url(sha256(code_verifier))
        let challenge = code_challenge_s256(code_verifier);
        // `openid email profile` is the canonical Google scope tuple for
        // a userinfo-style identity flow.
        let params = [
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", state),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            // Google-specific — surface a refresh_token only when the
            // user explicitly consents. The default behavior (no prompt)
            // is fine for an interactive web sign-in.
            ("access_type", "online"),
        ];
        // Manual querystring assembly so we don't pull in url::form_urlencoded;
        // the values here are all generated/known-safe.
        let qs: String = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{}", self.authorize_url, qs)
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OauthUserInfo, OauthError> {
        // POST x-www-form-urlencoded body, per RFC 6749 §4.1.3.
        let form = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("code_verifier", code_verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", self.redirect_uri.as_str()),
        ];

        let token_resp = self
            .http
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| OauthError::Transport(e.to_string()))?;

        let status = token_resp.status();
        if !status.is_success() {
            return Err(OauthError::TokenExchange(status.as_u16()));
        }

        let token: TokenResponse = token_resp
            .json()
            .await
            .map_err(|e| OauthError::BadJson(e.to_string()))?;

        // Some test setups (and Google itself, when `openid` is in the
        // scope set) return an id_token whose payload already carries
        // {sub, email, email_verified, name}. If we have one we trust
        // it (we just got it over TLS from the token endpoint), which
        // saves a userinfo round-trip. Otherwise we hit /userinfo with
        // the access_token.
        if let Some(id_token) = token.id_token.as_deref()
            && let Some(info) = parse_id_token_payload(id_token)
        {
            return Ok(info);
        }

        let user: GoogleUserInfo = self
            .http
            .get(&self.userinfo_url)
            .bearer_auth(&token.access_token)
            .send()
            .await
            .map_err(|e| OauthError::Transport(e.to_string()))?
            .error_for_status()
            .map_err(|e| OauthError::Transport(e.to_string()))?
            .json()
            .await
            .map_err(|e| OauthError::BadJson(e.to_string()))?;

        Ok(OauthUserInfo {
            sub: user.sub,
            email: user.email,
            email_verified: user.email_verified.unwrap_or(false),
            name: user.name,
        })
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: String,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    name: Option<String>,
}

/// Extract `{sub, email, email_verified, name}` from a JWT's middle
/// segment without verifying the signature — we only call this when
/// we just got the JWT from a TLS-protected token endpoint, where the
/// transport itself is the trust anchor.
fn parse_id_token_payload(id_token: &str) -> Option<OauthUserInfo> {
    let segs: Vec<&str> = id_token.split('.').collect();
    if segs.len() < 2 {
        return None;
    }
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(segs[1])
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    Some(OauthUserInfo {
        sub: v.get("sub")?.as_str()?.to_string(),
        email: v.get("email")?.as_str()?.to_string(),
        email_verified: v
            .get("email_verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        name: v
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    })
}

// ---------------------------------------------------------------------------
// PKCE helpers
// ---------------------------------------------------------------------------

/// Generate a fresh PKCE `code_verifier`. RFC 7636 §4.1 calls for a
/// high-entropy random string of 43-128 unreserved characters; we use
/// 32 random bytes base64url-encoded (= 43 chars), the same shape as
/// our other tokens.
#[must_use]
pub fn new_code_verifier() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Derive the S256 `code_challenge` for a given verifier (RFC 7636 §4.2).
#[must_use]
pub fn code_challenge_s256(verifier: &str) -> String {
    let mut h = Sha256::new();
    h.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(h.finalize())
}

/// Generate the CSRF `state` token. 32 random bytes base64url-encoded —
/// matches the other token shapes in this crate.
#[must_use]
pub fn new_state() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut buf)
        .expect("OS RNG must work");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Percent-encode a value for a `query=string`. Conservative whitelist
/// (RFC 3986 unreserved + nothing else); we'd reach for `url` if the
/// values could contain anything beyond ASCII letters/digits.
fn urlencode(v: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(b));
            }
            _ => {
                // Cannot fail — writing to a String never errors.
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// DB helpers — store/consume the (state, verifier, return_to) tuple.
// ---------------------------------------------------------------------------

use chrono::{Duration, Utc};
use sqlx::SqlitePool;

/// Default TTL for a stored oauth state row. 10 minutes is plenty —
/// the user has to land on Google, log in, and bounce back.
pub const OAUTH_STATE_TTL_SECS: i64 = 60 * 10;

/// Persist a `(state, verifier, return_to)` tuple. Called from the
/// /start handler after we generate the state + verifier.
pub async fn store_state(
    pool: &SqlitePool,
    state: &str,
    code_verifier: &str,
    return_to: Option<&str>,
    ttl_secs: i64,
) -> Result<(), sqlx::Error> {
    let expires_at = (Utc::now() + Duration::seconds(ttl_secs)).to_rfc3339();
    sqlx::query(
        "INSERT INTO oauth_states (state, code_verifier, redirect_to, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(state)
    .bind(code_verifier)
    .bind(return_to)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically consume an oauth state row: returns the original
/// `(code_verifier, return_to)` pair on a hit, and `None` on a miss
/// (unknown state, expired, or already consumed in a previous call).
/// `DELETE ... RETURNING` is the SQLite idiom for "claim it exactly once."
pub async fn consume_state(
    pool: &SqlitePool,
    state: &str,
) -> Result<Option<(String, Option<String>)>, sqlx::Error> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "DELETE FROM oauth_states
         WHERE state = ?
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING code_verifier, redirect_to",
    )
    .bind(state)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Find an existing user by Google sub, or by email if the sub is new.
/// In the latter case we link the sub onto the existing row. Returns
/// the resulting `user_id`.
pub async fn find_or_create_user(
    pool: &SqlitePool,
    info: &OauthUserInfo,
) -> Result<i64, sqlx::Error> {
    // Already linked? Return the user_id.
    if let Some(uid) = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE google_sub = ?")
        .bind(&info.sub)
        .fetch_optional(pool)
        .await?
    {
        return Ok(uid);
    }

    // Same email already exists from a password signup? Link instead of
    // creating a duplicate.
    if let Some(uid) = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(&info.email)
        .fetch_optional(pool)
        .await?
    {
        sqlx::query(
            "UPDATE users
             SET google_sub = ?,
                 is_email_verified = CASE WHEN ? = 1 THEN 1 ELSE is_email_verified END,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(&info.sub)
        .bind(i64::from(info.email_verified))
        .bind(uid)
        .execute(pool)
        .await?;
        return Ok(uid);
    }

    // Brand-new user. OAuth-only — no password yet, so we plant a
    // sentinel hash that argon2's `verify` will reject. The user can
    // later set a password via the reset flow if they want one.
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, google_sub, is_email_verified)
         VALUES (?, '!oauth-only', ?, ?)
         RETURNING id",
    )
    .bind(&info.email)
    .bind(&info.sub)
    .bind(i64::from(info.email_verified))
    .fetch_one(pool)
    .await?;
    Ok(uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_challenge_matches_rfc7636_example() {
        // RFC 7636 §4.6 worked example.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(code_challenge_s256(verifier), expected);
    }

    #[test]
    fn new_verifier_and_state_are_url_safe_43_chars() {
        let v = new_code_verifier();
        let s = new_state();
        for token in [&v, &s] {
            assert_eq!(token.len(), 43);
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            );
        }
    }

    #[test]
    fn authorize_url_embeds_state_and_challenge() {
        let g = GoogleProvider::new(
            "client-xyz".into(),
            "secret".into(),
            "https://app.test/cb".into(),
        );
        let url = g.authorize_url("the-state", "verifier-value");
        assert!(url.starts_with(GoogleProvider::DEFAULT_AUTHORIZE_URL));
        assert!(url.contains("state=the-state"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("client_id=client-xyz"));
        assert!(url.contains(&format!(
            "code_challenge={}",
            code_challenge_s256("verifier-value")
        )));
    }
}
