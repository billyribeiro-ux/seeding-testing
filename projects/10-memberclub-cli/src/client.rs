//! Thin HTTP client that talks to the MemberClub API.
//!
//! Every request that needs authentication carries an `Authorization:
//! Bearer <jwt>` header pulled from the stored credentials. Non-2xx
//! responses are converted into `CliError` variants so the caller can
//! print a useful message and `std::process::exit(1)` cleanly.

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::config::Credentials;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API returned {status}: {body}")]
    Api { status: u16, body: String },
    #[error("not logged in — run `memberclub login` first")]
    NotAuthenticated,
    #[error("config error: {0}")]
    Config(String),
}

/// The bare-minimum login response shape (matches `auth-demo`'s
/// `LoginResponse`).
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub user: UserDto,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize)]
pub struct UserDto {
    pub id: i64,
    pub email: String,
    #[serde(default)]
    pub is_email_verified: bool,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub totp_enabled: bool,
}

/// Wraps a `reqwest::Client` + a base URL. The auth token is passed in
/// per-call instead of stored on the struct so a single `ApiClient` can
/// serve multiple users in a future "switch profile" feature.
#[derive(Clone)]
pub struct ApiClient {
    http: Client,
    base_url: String,
}

impl ApiClient {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent(concat!("memberclub-cli/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client builder must succeed with rustls"),
            base_url: base_url.into(),
        }
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// POST /auth/login. Returns the parsed body on 200, an `Api` error
    /// otherwise.
    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResponse, CliError> {
        let url = format!("{}/auth/login", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&json!({ "email": email, "password": password }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json().await?)
        } else {
            Err(CliError::Api {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
    }

    /// GET /me, signed with the bearer token from `creds`.
    pub async fn me(&self, creds: &Credentials) -> Result<UserDto, CliError> {
        let url = format!("{}/me", self.base_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&creds.access_token)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json().await?)
        } else {
            Err(CliError::Api {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
    }

    /// POST /auth/logout. Invalidates the cookie session on the server;
    /// the local credentials file is cleared separately by the caller.
    pub async fn logout(&self, creds: &Credentials) -> Result<(), CliError> {
        let url = format!("{}/auth/logout", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&creds.access_token)
            .send()
            .await?;
        if resp.status().is_success()
            || resp.status() == reqwest::StatusCode::NO_CONTENT
            || resp.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            Ok(())
        } else {
            Err(CliError::Api {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
    }

    /// GET /v1/notes (the keyset-paginated notes endpoint). Returns the
    /// raw JSON value so the caller can pretty-print or pivot.
    pub async fn list_notes(
        &self,
        creds: &Credentials,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<serde_json::Value, CliError> {
        let mut url = format!("{}/v1/notes", self.base_url);
        let mut qs = Vec::new();
        if let Some(l) = limit {
            qs.push(format!("limit={l}"));
        }
        if let Some(c) = cursor {
            qs.push(format!("cursor={c}"));
        }
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&creds.access_token)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json().await?)
        } else {
            Err(CliError::Api {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
    }
}
