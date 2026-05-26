//! auth-demo binary entrypoint.

use std::net::SocketAddr;

use anyhow::Context;
use axum_extra::extract::cookie::Key;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::EnvFilter;

use auth_demo::{AppState, jwt::Jwt, migrate, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,auth_demo=debug,tower_http=info")),
        )
        .compact()
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let bind: SocketAddr = std::env::var("APP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3001".into())
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

    // Session cookie key — read from env (must be >= 64 bytes) or generate one for dev.
    let cookie_key = std::env::var("SESSION_SECRET")
        .ok()
        .filter(|s| s.len() >= 64)
        .map_or_else(Key::generate, |s| Key::from(s.as_bytes()));

    // JWT secret — read from env or generate one for dev.
    let jwt_secret: Vec<u8> = std::env::var("JWT_SECRET")
        .ok()
        .filter(|s| s.len() >= 32)
        .map_or_else(|| Jwt::random_secret().to_vec(), String::into_bytes);
    let jwt = Jwt::new(&jwt_secret);

    let app = router(AppState::new(pool, cookie_key, jwt));

    tracing::info!(%bind, "starting auth-demo");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
