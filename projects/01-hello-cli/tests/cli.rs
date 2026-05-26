//! Integration tests for the hello-cli binary.
//!
//! These spawn the actual binary (built by Cargo for tests) and assert on its
//! stdout, stderr, and exit code. They're the closest thing to "what the user
//! sees" without an end-to-end browser test.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn bin() -> Command {
    Command::cargo_bin("hello-cli").expect("binary built")
}

#[test]
fn prints_help() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Count lines, words, and characters",
        ));
}

#[test]
fn prints_version() {
    bin()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello-cli"));
}

#[test]
fn reads_a_file() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "alpha beta gamma").unwrap();
    writeln!(f, "delta epsilon").unwrap();

    bin()
        .arg(f.path())
        .assert()
        .success()
        // 2 lines, 5 words, "alpha beta gamma\ndelta epsilon\n" = 31 chars
        .stdout(predicate::str::contains("      2"))
        .stdout(predicate::str::contains("      5"))
        .stdout(predicate::str::contains("     31"));
}

#[test]
fn reads_stdin_when_no_file() {
    bin()
        .write_stdin("one two three\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("      1"))
        .stdout(predicate::str::contains("      3"));
}

#[test]
fn missing_file_exits_with_code_2() {
    bin()
        .arg("/path/that/definitely/does/not/exist/nope.txt")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no such file"));
}

#[test]
fn only_lines_flag_prints_one_column() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "line one").unwrap();
    writeln!(f, "line two").unwrap();

    let output = bin()
        .arg("--lines")
        .arg(f.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let s = String::from_utf8(output).unwrap();
    let trimmed = s.trim();
    // Single column ("      2"), no other numbers. After trimming whitespace we expect "2".
    assert_eq!(trimmed, "2");
}
