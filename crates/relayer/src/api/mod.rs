//! HTTP API routes.

mod routes;

use axum::{routing::get, Router};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use crate::db::Db;
use crate::relay::RelayState;

/// Pending file read request: oneshot sender to receive executor response.
pub type FileReadPending = Arc<RwLock<HashMap<Uuid, oneshot::Sender<Result<String, String>>>>>;

/// Pending file search request: oneshot sender to receive executor response.
pub type FileSearchPending =
    Arc<RwLock<HashMap<Uuid, oneshot::Sender<Result<Vec<shared::FileSearchMatch>, String>>>>>;

/// Shared app state.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub relay: Arc<RelayState>,
    pub config: Arc<crate::config::Config>,
    pub models: Arc<RwLock<Vec<String>>>,
    /// Pending file read requests: request_id -> oneshot sender.
    pub file_read_pending: FileReadPending,
    /// Pending file search requests: request_id -> oneshot sender.
    pub file_search_pending: FileSearchPending,
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .nest("/api", routes::api_routes())
        .route("/ws", get(routes::ws_handler))
        .layer(cors)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
