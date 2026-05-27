//! Subcommand implementations. Each is an `async fn` `main.rs` calls
//! once `clap` has parsed the arguments. Side-effects are isolated to
//! `print!` / `eprintln!` so a future "json output mode" flag could be
//! added by switching the format at the printers, not the logic.

use chrono::{Duration, Utc};

use crate::client::ApiClient;
use crate::config::{self, ConfigPaths, Credentials};

/// `memberclub login --email alice@x.com --password ...`
///
/// Calls POST /auth/login, stashes the returned tokens on disk, prints
/// a short success line.
pub async fn login(
    paths: &ConfigPaths,
    base_url: String,
    email: String,
    password: String,
) -> anyhow::Result<()> {
    let client = ApiClient::new(&base_url);
    let resp = client.login(&email, &password).await?;
    let expires_at = Utc::now() + Duration::seconds(resp.expires_in);
    let creds = Credentials {
        api_base_url: base_url,
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        access_expires_at: expires_at.to_rfc3339(),
        user_email: resp.user.email.clone(),
    };
    config::save(paths, &creds)?;
    println!("Signed in as {}", resp.user.email);
    Ok(())
}

/// `memberclub logout`. Invalidates the session server-side AND clears
/// the local credentials file. Idempotent if you're already logged out.
pub async fn logout(paths: &ConfigPaths) -> anyhow::Result<()> {
    let Some(creds) = config::load(paths)? else {
        println!("Already signed out.");
        return Ok(());
    };
    let client = ApiClient::new(&creds.api_base_url);
    // Best-effort — even if the server rejects the token (expired,
    // already revoked), clear the local file.
    let _ = client.logout(&creds).await;
    config::clear(paths)?;
    println!("Signed out.");
    Ok(())
}

/// `memberclub whoami`. Calls /me, prints the email + roles.
pub async fn whoami(paths: &ConfigPaths) -> anyhow::Result<()> {
    let creds = require_creds(paths)?;
    let client = ApiClient::new(&creds.api_base_url);
    let me = client.me(&creds).await?;
    println!("email:           {}", me.email);
    println!("email_verified:  {}", me.is_email_verified);
    println!("admin:           {}", me.is_admin);
    println!("totp_enabled:    {}", me.totp_enabled);
    Ok(())
}

/// `memberclub notes list [--limit N] [--cursor C]`. Prints the page
/// as a small table — id, created_at, body (truncated to 60 chars).
pub async fn notes_list(
    paths: &ConfigPaths,
    limit: Option<u32>,
    cursor: Option<String>,
) -> anyhow::Result<()> {
    let creds = require_creds(paths)?;
    let client = ApiClient::new(&creds.api_base_url);
    let page = client.list_notes(&creds, limit, cursor.as_deref()).await?;
    let items = page["items"].as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        println!("(no notes)");
    } else {
        for note in &items {
            let id = note["id"].as_i64().unwrap_or_default();
            let body = note["body"].as_str().unwrap_or("");
            let trimmed = if body.chars().count() > 60 {
                format!("{}…", body.chars().take(60).collect::<String>())
            } else {
                body.to_string()
            };
            let created = note["created_at"].as_str().unwrap_or("");
            println!("{id:>5}  {created}  {trimmed}");
        }
    }
    if let Some(next) = page["next"].as_str() {
        println!();
        println!("--next-cursor={next}");
    }
    Ok(())
}

/// Helper: error out cleanly when there's no saved credentials file.
fn require_creds(paths: &ConfigPaths) -> anyhow::Result<Credentials> {
    config::load(paths)?
        .ok_or_else(|| anyhow::anyhow!("not logged in — run `memberclub login` first"))
}
