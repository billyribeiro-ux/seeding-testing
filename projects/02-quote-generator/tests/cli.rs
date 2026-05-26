//! Integration tests for quote-generator.
//!
//! We spin up a local HTTP server via `wiremock` so tests never depend on the public
//! internet. The actual `quote-generator` binary is spawned via `assert_cmd`.

use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

fn bin() -> Command {
    Command::cargo_bin("quote-generator").expect("binary built")
}

#[tokio::test(flavor = "current_thread")]
async fn fetches_a_single_url() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("hello"))
        .mount(&server)
        .await;

    // Spawn the binary on a blocking thread so we don't deadlock the current-thread runtime.
    let url = format!("{}/q", server.uri());
    let out = tokio::task::spawn_blocking(move || {
        bin().arg(&url).assert().success().get_output().clone()
    })
    .await
    .unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("OK"), "stdout = {stdout}");
    assert!(stdout.contains("200"));
}

#[tokio::test(flavor = "current_thread")]
async fn empty_input_exits_with_code_2() {
    tokio::task::spawn_blocking(|| {
        bin()
            .assert()
            .code(2)
            .stderr(predicate::str::contains("no URLs provided"));
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn reads_urls_from_file() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("hi"))
        .mount(&server)
        .await;

    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "# this is a comment").unwrap();
    writeln!(f, "{}/a", server.uri()).unwrap();
    writeln!(f, "{}/b", server.uri()).unwrap();
    writeln!(f).unwrap(); // blank line
    writeln!(f, "{}/c", server.uri()).unwrap();
    let path = f.path().to_path_buf();

    let out = tokio::task::spawn_blocking(move || {
        bin()
            .arg("--file")
            .arg(&path)
            .assert()
            .success()
            .get_output()
            .clone()
    })
    .await
    .unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().filter(|l| l.starts_with("OK")).count(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn timeout_marks_slow_request() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(400))
                .set_body_string("slow"),
        )
        .mount(&server)
        .await;

    let url = format!("{}/slow", server.uri());
    let out = tokio::task::spawn_blocking(move || {
        bin()
            .arg(&url)
            .arg("--timeout")
            .arg("100ms")
            .assert()
            .code(1)
            .get_output()
            .clone()
    })
    .await
    .unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("TIME"), "stdout = {stdout}");
}
