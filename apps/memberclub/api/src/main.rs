//! memberclub-api binary entrypoint.

use std::net::SocketAddr;

use anyhow::Context;
use axum_extra::extract::cookie::Key;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::EnvFilter;

use memberclub_api::{AppState, auth::Jwt, migrate, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,memberclub_api=debug,tower_http=info")),
        )
        .with_target(false)
        .compact()
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let bind: SocketAddr = std::env::var("APP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3002".into())
        .parse()
        .context("APP_BIND must be host:port")?;

    let pool = SqlitePoolOptions::new()
        .max_connections(if database_url == "sqlite::memory:" {
            1
        } else {
            8
        })
        .connect(&database_url)
        .await
        .context("connect to sqlite")?;
    migrate(&pool).await?;

    let cookie_key = std::env::var("SESSION_SECRET")
        .ok()
        .filter(|s| s.len() >= 64)
        .map_or_else(Key::generate, |s| Key::from(s.as_bytes()));

    let jwt_secret: Vec<u8> = std::env::var("JWT_SECRET")
        .ok()
        .filter(|s| s.len() >= 32)
        .map_or_else(|| Jwt::random_secret().to_vec(), String::into_bytes);
    let jwt = Jwt::new(&jwt_secret);

    let app = router(AppState::new(pool, cookie_key, jwt));

    tracing::info!(%bind, "starting memberclub-api");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum::serve")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("ctrl-c received; shutting down"),
        () = terminate => tracing::info!("SIGTERM received; shutting down"),
    }
}
