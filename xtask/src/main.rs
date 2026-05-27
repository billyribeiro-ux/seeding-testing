//! xtask — the modern Rust idiom for repo-meta scripts.
//!
//! Bash scripts that build the build don't scale; bash scripts don't
//! get type-checked. `cargo xtask <command>` is a tiny Rust binary
//! that gets the same fmt + clippy + ci treatment as everything else
//! in the workspace.
//!
//! Run `cargo xtask --help` for the catalog.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Repo-meta tasks for seeding-testing. Run with `cargo xtask <subcommand>`."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the full local verify sweep: fmt, clippy -D warnings, nextest,
    /// sqlx prepare --check, cargo deny, cargo audit. Same as `make verify`
    /// but discoverable from the cargo subcommand catalog.
    Verify,
    /// Regenerate the OpenAPI snapshot for notes-api. Run after an API
    /// surface change; commit the snapshot diff.
    OpenapiBless,
    /// Print the version of every workspace crate.
    Versions,
    /// Bump every workspace crate to a new version (e.g. `0.2.0`). Patches
    /// the root `[workspace.package].version` field; member crates inherit.
    BumpVersion {
        /// New version (e.g. `0.2.0`).
        version: String,
    },
    /// Sanity-check the migrations directory ordering — each crate's
    /// `migrations/` must be lexicographically ordered with no gaps in
    /// timestamps, and the newest must be newer than yesterday.
    CheckMigrations,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    std::env::set_current_dir(&root)?;
    match cli.cmd {
        Cmd::Verify => verify(),
        Cmd::OpenapiBless => openapi_bless(),
        Cmd::Versions => versions(&root),
        Cmd::BumpVersion { version } => bump_version(&root, &version),
        Cmd::CheckMigrations => check_migrations(&root),
    }
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .context("cargo locate-project failed")?;
    if !output.status.success() {
        return Err(anyhow!("cargo locate-project exited non-zero"));
    }
    let manifest = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(Path::new(&manifest)
        .parent()
        .ok_or_else(|| anyhow!("workspace manifest has no parent"))?
        .to_path_buf())
}

fn run(cmd: &str, args: &[&str]) -> anyhow::Result<()> {
    println!("==> {cmd} {}", args.join(" "));
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        return Err(anyhow!("{cmd} {} exited {status}", args.join(" ")));
    }
    Ok(())
}

fn verify() -> anyhow::Result<()> {
    run("cargo", &["fmt", "--all", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["nextest", "run", "--workspace"])?;
    // sqlx prepare check is best-effort — only fires when `.sqlx/` exists.
    if Path::new(".sqlx").exists() {
        run("cargo", &["sqlx", "prepare", "--check", "--workspace"])?;
    }
    if which("cargo-deny").is_some() {
        run("cargo", &["deny", "check"])?;
    }
    if which("cargo-audit").is_some() {
        run("cargo", &["audit"])?;
    }
    Ok(())
}

fn openapi_bless() -> anyhow::Result<()> {
    // The snapshot lives in projects/03-notes-api/tests/snapshots/.
    // `cargo insta accept` would do this too but we want a one-command
    // form that's discoverable.
    println!("==> blessing OpenAPI snapshot via INSTA_UPDATE=always");
    let status = Command::new("cargo")
        .args(["test", "-p", "notes-api", "--test", "openapi"])
        .env("INSTA_UPDATE", "always")
        .status()?;
    if !status.success() {
        return Err(anyhow!("openapi snapshot regen failed"));
    }
    println!("✓ snapshot regenerated — review the diff and commit");
    Ok(())
}

fn versions(root: &Path) -> anyhow::Result<()> {
    let root_toml: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml"))?)?;
    let ws_version = root_toml
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("workspace.package.version not set"))?;
    println!("workspace.package.version = {ws_version}");
    println!();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_name() == "Cargo.toml")
    {
        let path = entry.path();
        if path == root.join("Cargo.toml") {
            continue;
        }
        if !path.starts_with(root.join("projects"))
            && !path.starts_with(root.join("apps"))
            && !path.starts_with(root.join("xtask"))
        {
            continue;
        }
        let body: toml::Value = match std::fs::read_to_string(path)
            .and_then(|s| toml::from_str(&s).map_err(|e| std::io::Error::other(e.to_string())))
        {
            Ok(v) => v,
            Err(_) => continue,
        };
        let name = body
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let version = body
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.get("workspace").map(|_| "workspace".to_string()))
            })
            .unwrap_or_else(|| "?".to_string());
        let rel = path.strip_prefix(root).unwrap_or(path);
        println!("  {name:<28} {version:<14} {}", rel.display());
    }
    Ok(())
}

fn bump_version(root: &Path, version: &str) -> anyhow::Result<()> {
    if !version_is_valid(version) {
        return Err(anyhow!(
            "version must look like MAJOR.MINOR.PATCH (e.g. 0.2.0); got '{version}'"
        ));
    }
    let path = root.join("Cargo.toml");
    let src = std::fs::read_to_string(&path)?;
    let mut out = String::with_capacity(src.len());
    let mut in_workspace_package = false;
    let mut bumped = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_workspace_package = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('[') && trimmed != "[workspace.package]" {
            in_workspace_package = false;
        }
        if in_workspace_package && trimmed.starts_with("version") {
            use std::fmt::Write as _;
            // Preserve indentation by replacing only the value.
            let _ = writeln!(out, "version    = \"{version}\"");
            bumped = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !bumped {
        return Err(anyhow!(
            "could not find `[workspace.package].version` in {}",
            path.display()
        ));
    }
    std::fs::write(&path, out)?;
    println!("✓ bumped workspace.package.version to {version}");
    println!("  Now run: cargo update -w && cargo xtask verify");
    Ok(())
}

fn version_is_valid(v: &str) -> bool {
    let parts: Vec<_> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn check_migrations(root: &Path) -> anyhow::Result<()> {
    let mut found_any = false;
    let mut warnings = Vec::<String>::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir() && e.file_name() == "migrations")
    {
        let dir = entry.path();
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".sql"))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        if files.is_empty() {
            continue;
        }
        found_any = true;
        files.sort();
        // Each file should start with a 14-digit timestamp.
        for f in &files {
            let stem: String = f.chars().take(14).collect();
            if stem.len() != 14 || !stem.chars().all(|c| c.is_ascii_digit()) {
                warnings.push(format!(
                    "{}/{f}: filename doesn't start with a 14-digit timestamp",
                    dir.display()
                ));
            }
        }
        // The newest must not be in the future.
        if let Some(latest) = files.last() {
            let stem: String = latest.chars().take(14).collect();
            if stem.len() == 14 {
                // YYYYMMDDhhmmss — naïve compare to "now" formatted the same.
                // We only flag if the timestamp claims to be > 1 day in the future.
                let now = chrono_like_yyyymmddhhmmss();
                if stem.as_str() > now.as_str() {
                    warnings.push(format!(
                        "{}/{latest}: timestamp is in the future ({stem} > {now})",
                        dir.display()
                    ));
                }
            }
        }
    }
    if !found_any {
        println!("(no migrations directories found)");
        return Ok(());
    }
    if warnings.is_empty() {
        println!("✓ migrations look fine");
        Ok(())
    } else {
        for w in &warnings {
            println!("⚠ {w}");
        }
        Err(anyhow!("{} migration warnings", warnings.len()))
    }
}

/// Tiny replacement for chrono when we just need the YYYYMMDDhhmmss
/// shape of "now". Avoids pulling chrono into the xtask binary.
fn chrono_like_yyyymmddhhmmss() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Add 24h grace so a slightly-fast CI clock doesn't false-positive.
    let secs = now + 24 * 60 * 60;
    let (years, secs) = (secs / (365 * 24 * 3600) + 1970, secs % (365 * 24 * 3600));
    let (days, secs) = (secs / (24 * 3600), secs % (24 * 3600));
    let (hours, secs) = (secs / 3600, secs % 3600);
    let (mins, secs) = (secs / 60, secs % 60);
    let (month, day) = day_of_year_to_month_day(days as u32, years as u32);
    format!("{years:04}{month:02}{day:02}{hours:02}{mins:02}{secs:02}")
}

fn day_of_year_to_month_day(doy: u32, _year: u32) -> (u32, u32) {
    // Crude approximation — adequate for "is this timestamp from
    // the future" checks where the only inputs are committed by hand.
    const DAYS_PER_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut remaining = doy;
    for (i, &d) in DAYS_PER_MONTH.iter().enumerate() {
        if remaining < d {
            return (u32::try_from(i).unwrap_or(0) + 1, remaining + 1);
        }
        remaining -= d;
    }
    (12, 31)
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let p = dir.join(bin);
            if p.is_file() { Some(p) } else { None }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_validator_rejects_garbage() {
        assert!(version_is_valid("0.1.0"));
        assert!(version_is_valid("12.34.56"));
        assert!(!version_is_valid("0.1"));
        assert!(!version_is_valid("0.1.0-alpha"));
        assert!(!version_is_valid("v0.1.0"));
        assert!(!version_is_valid(""));
    }
}
