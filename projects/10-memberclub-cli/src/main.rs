//! `memberclub` — the CLI entry point.

use clap::{Parser, Subcommand};
use memberclub_cli::ConfigPaths;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "memberclub",
    version,
    about = "CLI for the MemberClub HTTP API.",
    long_about = "Sign in, inspect your account, browse notes, all from your terminal. \
                  Reads credentials from $XDG_CONFIG_HOME/memberclub/credentials.toml. \
                  Override the config path with --config-dir for tests or sandboxes."
)]
struct Cli {
    /// Override the config directory. Defaults to the platform-standard
    /// $XDG_CONFIG_HOME/memberclub/.
    #[arg(long, global = true)]
    config_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Sign in to a MemberClub instance. Saves the returned JWTs to
    /// the local credentials file.
    Login {
        /// Base URL of the API (e.g. https://api.memberclub.test).
        #[arg(long, default_value = "http://127.0.0.1:3001")]
        api_base_url: String,

        /// Email to sign in as.
        #[arg(long)]
        email: String,

        /// Password. Prefer the MEMBERCLUB_PASSWORD env var so it
        /// doesn't end up in shell history. The flag wins if both are
        /// set.
        #[arg(long, env = "MEMBERCLUB_PASSWORD", hide_env_values = true)]
        password: String,
    },

    /// Invalidate the session AND clear the local credentials file.
    Logout,

    /// Print the currently signed-in user.
    Whoami,

    /// Notes subcommands.
    #[command(subcommand)]
    Notes(NotesCmd),
}

#[derive(Debug, Subcommand)]
enum NotesCmd {
    /// List notes (keyset-paginated).
    List {
        /// Page size, clamped server-side to [1, 100]. Default 20.
        #[arg(long)]
        limit: Option<u32>,

        /// Opaque cursor from a previous `list`'s `--next-cursor=...`
        /// output. Omit to fetch the first page.
        #[arg(long)]
        cursor: Option<String>,
    },
}

fn config_paths(cli: &Cli) -> anyhow::Result<ConfigPaths> {
    if let Some(dir) = &cli.config_dir {
        return Ok(ConfigPaths::under(dir));
    }
    ConfigPaths::from_system().ok_or_else(|| {
        anyhow::anyhow!("could not determine the platform config directory; use --config-dir")
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let cli = Cli::parse();
    let paths = config_paths(&cli)?;

    match cli.cmd {
        Cmd::Login {
            api_base_url,
            email,
            password,
        } => memberclub_cli::commands::login(&paths, api_base_url, email, password).await,
        Cmd::Logout => memberclub_cli::commands::logout(&paths).await,
        Cmd::Whoami => memberclub_cli::commands::whoami(&paths).await,
        Cmd::Notes(NotesCmd::List { limit, cursor }) => {
            memberclub_cli::commands::notes_list(&paths, limit, cursor).await
        }
    }
}
