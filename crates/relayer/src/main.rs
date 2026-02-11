//! Dev PM Agent Relayer — HTTP + WebSocket backend.
//!
//! Required env: JWT_SECRET, EXECUTOR_API_KEY
//! Optional: HOST, PORT, DATABASE_PATH, JWT_TTL_SECS

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

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
    let models = Arc::new(RwLock::new(vec![
        "composer-1.5".to_string(),
        "opus-4.6-thinking".to_string(),
        "gpt-5.3-codex-high".to_string(),
        "gemini-3-pro".to_string(),
    ]));

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("invalid bind address");

    let state = api::AppState {
        db,
        relay,
        config,
        models,
        file_read_pending: Arc::new(RwLock::new(HashMap::new())),
        file_search_pending: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = api::router(state);

    tracing::info!("Relayer listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
