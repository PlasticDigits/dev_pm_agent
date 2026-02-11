//! Dev PM Agent Relayer — HTTP + WebSocket backend.
//!
//! Required env: JWT_SECRET, EXECUTOR_API_KEY
//! Optional: HOST, PORT, DATABASE_PATH, JWT_TTL_SECS

use std::sync::Arc;

use relayer::{api, config, db, relay};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = config::Config::from_env().map_err(|e| anyhow::anyhow!("config: {}", e))?;
    let config = Arc::new(config);

    let db = db::Db::open(&config.database_path)?;
    db.run_migrations()?;
    let db = Arc::new(db);

    let relay = Arc::new(relay::RelayState::new());

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("invalid bind address");

    let state = api::AppState { db, relay, config };

    let app = api::router(state);

    tracing::info!("Relayer listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}
