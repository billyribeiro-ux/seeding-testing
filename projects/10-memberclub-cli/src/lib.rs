//! memberclub-cli — the curriculum's promised Rust CLI client for the
//! MemberClub HTTP API.
//!
//! The architecture is intentionally three layers:
//!
//!   * `config` — load + save `~/.config/memberclub/credentials.toml`.
//!     The token store the `login` subcommand writes to and every other
//!     subcommand reads from.
//!   * `client` — a thin `reqwest::Client` wrapper that signs every
//!     request with the stored JWT bearer token and maps non-2xx
//!     responses into a `CliError`.
//!   * `commands` — one async function per `clap` subcommand. These
//!     are the only things `main.rs` calls.
//!
//! The split lets us unit-test the client against a `wiremock` MockServer
//! without spinning up the real Axum service. The integration test in
//! `tests/cli.rs` drives the real binary via `assert_cmd` against a
//! mock server too.

pub mod client;
pub mod commands;
pub mod config;

pub use client::{ApiClient, CliError};
pub use config::{ConfigPaths, Credentials};
