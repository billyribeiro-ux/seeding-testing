//! Build-time helper for Phase 4 — E4.1.
//!
//! Stamps the build with the short git SHA so `/version` can return it
//! at runtime. Falls back to `"dev"` when not in a git checkout (e.g.
//! crates.io publishing, vendored builds) — the route's contract is
//! that the field is always present, value never an empty string.

use std::process::Command;

fn main() {
    // Re-run if HEAD moves so cached builds don't lie.
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "dev".into(), |s| s.trim().to_string());

    println!("cargo:rustc-env=GIT_SHA={sha}");
}
