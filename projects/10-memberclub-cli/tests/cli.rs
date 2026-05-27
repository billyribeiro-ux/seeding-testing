//! Integration tests that drive the real `memberclub` binary through
//! `assert_cmd` against a `wiremock` HTTP mock. We never talk to a
//! real MemberClub instance — the API contract is mocked and pinned.

use std::process::Command;

use assert_cmd::assert::OutputAssertExt as _;
use assert_cmd::cargo::CommandCargoExt as _;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{bearer_token, body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a `Command` invoking the built binary with --config-dir
/// pointing at `tmp` so the test never touches the user's real config.
fn cmd(tmp: &TempDir) -> Command {
    let mut c = Command::cargo_bin("memberclub").expect("memberclub binary built");
    c.arg("--config-dir").arg(tmp.path());
    c
}

#[tokio::test]
async fn login_persists_credentials_and_prints_the_email() {
    let tmp = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(json!({
            "email":    "alice@example.test",
            "password": "correct horse battery staple",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "id": 1,
                "email": "alice@example.test",
                "is_email_verified": true,
                "is_admin": true,
                "totp_enabled": false,
            },
            "access_token":  "the-access-token",
            "refresh_token": "the-refresh-token",
            "expires_in":    900,
        })))
        .mount(&server)
        .await;

    let assert = tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.args([
            "login",
            "--api-base-url",
            server.uri().as_str(),
            "--email",
            "alice@example.test",
            "--password",
            "correct horse battery staple",
        ]);
        move || c.output().expect("memberclub login exits")
    })
    .await
    .unwrap();

    assert!(
        assert.status.success(),
        "login exited non-zero: stdout={} stderr={}",
        String::from_utf8_lossy(&assert.stdout),
        String::from_utf8_lossy(&assert.stderr),
    );
    let stdout = String::from_utf8_lossy(&assert.stdout);
    assert!(stdout.contains("Signed in as alice@example.test"));

    // Credentials file exists and parses.
    let body = std::fs::read_to_string(tmp.path().join("credentials.toml")).unwrap();
    assert!(body.contains("the-access-token"));
    assert!(body.contains("alice@example.test"));
}

#[tokio::test]
async fn whoami_uses_the_stored_bearer_token() {
    let tmp = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Login mock so we can populate the credentials.
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "id": 2,
                "email": "bob@example.test",
                "is_email_verified": false,
                "is_admin": false,
                "totp_enabled": true,
            },
            "access_token":  "bobs-access",
            "refresh_token": "bobs-refresh",
            "expires_in":    900,
        })))
        .mount(&server)
        .await;

    // /me requires the bearer to be exactly the access token we issued.
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(bearer_token("bobs-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 2,
            "email": "bob@example.test",
            "is_email_verified": false,
            "is_admin": false,
            "totp_enabled": true,
        })))
        .mount(&server)
        .await;

    // Login first.
    tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.args([
            "login",
            "--api-base-url",
            server.uri().as_str(),
            "--email",
            "bob@example.test",
            "--password",
            "any password 12+ chars",
        ]);
        move || c.assert().success()
    })
    .await
    .unwrap();

    // Then whoami.
    let assert = tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.arg("whoami");
        move || c.output().expect("whoami exits")
    })
    .await
    .unwrap();
    let stdout = String::from_utf8_lossy(&assert.stdout);
    assert!(
        assert.status.success(),
        "whoami failed: {}",
        String::from_utf8_lossy(&assert.stderr)
    );
    assert!(stdout.contains("bob@example.test"));
    assert!(stdout.contains("totp_enabled:    true"));
}

#[tokio::test]
async fn whoami_without_login_errors_with_useful_message() {
    let tmp = TempDir::new().unwrap();
    let assert = tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.arg("whoami");
        move || c.assert()
    })
    .await
    .unwrap();
    assert
        .failure()
        .stderr(contains("not logged in — run `memberclub login` first"));
}

#[tokio::test]
async fn logout_clears_the_credentials_file() {
    let tmp = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "id": 3,
                "email": "carol@example.test",
                "is_email_verified": true,
                "is_admin": false,
                "totp_enabled": false,
            },
            "access_token":  "carols-access",
            "refresh_token": "carols-refresh",
            "expires_in":    900,
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/auth/logout"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    // Login first.
    tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.args([
            "login",
            "--api-base-url",
            server.uri().as_str(),
            "--email",
            "carol@example.test",
            "--password",
            "long enough password",
        ]);
        move || c.assert().success()
    })
    .await
    .unwrap();
    assert!(tmp.path().join("credentials.toml").exists());

    // Now logout.
    tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.arg("logout");
        move || c.assert().success()
    })
    .await
    .unwrap();
    assert!(
        !tmp.path().join("credentials.toml").exists(),
        "logout must remove the credentials file"
    );

    // Second logout is idempotent (no file to clear, no token to revoke).
    let assert = tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.arg("logout");
        move || c.output().expect("second logout exits")
    })
    .await
    .unwrap();
    assert!(assert.status.success(), "double-logout must be idempotent");
    assert!(
        String::from_utf8_lossy(&assert.stdout).contains("Already signed out"),
        "stdout was: {}",
        String::from_utf8_lossy(&assert.stdout)
    );
}

#[tokio::test]
async fn notes_list_renders_a_page_and_emits_next_cursor() {
    let tmp = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "id": 4,
                "email": "dave@example.test",
                "is_email_verified": true,
                "is_admin": false,
                "totp_enabled": false,
            },
            "access_token":  "dave-access",
            "refresh_token": "dave-refresh",
            "expires_in":    900,
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/notes"))
        .and(bearer_token("dave-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"id": 3, "body": "third",  "created_at": "2026-05-26T12:00:00Z"},
                {"id": 2, "body": "second", "created_at": "2026-05-25T12:00:00Z"},
            ],
            "next": "opaque-cursor-123",
        })))
        .mount(&server)
        .await;

    // Login first.
    tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.args([
            "login",
            "--api-base-url",
            server.uri().as_str(),
            "--email",
            "dave@example.test",
            "--password",
            "long enough password",
        ]);
        move || c.assert().success()
    })
    .await
    .unwrap();

    let assert = tokio::task::spawn_blocking({
        let mut c = cmd(&tmp);
        c.args(["notes", "list"]);
        move || c.output().expect("notes list exits")
    })
    .await
    .unwrap();
    let stdout = String::from_utf8_lossy(&assert.stdout);
    assert!(
        assert.status.success(),
        "notes list failed: {}",
        String::from_utf8_lossy(&assert.stderr)
    );
    assert!(stdout.contains("third"), "stdout was: {stdout}");
    assert!(stdout.contains("second"));
    assert!(stdout.contains("--next-cursor=opaque-cursor-123"));
}
