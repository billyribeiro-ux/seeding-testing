//! webhook-receiver binary.

use std::net::SocketAddr;

use anyhow::Context;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::EnvFilter;

use webhook_receiver::{AppState, migrate, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,webhook_receiver=debug")),
        )
        .compact()
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let bind: SocketAddr = std::env::var("APP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3002".into())
        .parse()
        .context("APP_BIND must be host:port")?;
    let secret = std::env::var("STRIPE_WEBHOOK_SECRET")
        .context("STRIPE_WEBHOOK_SECRET must be set (matches `stripe listen` output)")?;

    let pool = SqlitePoolOptions::new()
        .max_connections(if database_url == "sqlite::memory:" {
            1
        } else {
            8
        })
        .connect(&database_url)
        .await?;
    migrate(&pool).await?;

    let app = router(AppState::new(pool, secret));
    tracing::info!(%bind, "starting webhook-receiver");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
