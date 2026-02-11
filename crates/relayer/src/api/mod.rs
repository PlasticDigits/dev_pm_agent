//! HTTP API routes.

mod routes;

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::db::Db;
use crate::relay::RelayState;

/// Shared app state.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub relay: Arc<RelayState>,
    pub config: Arc<crate::config::Config>,
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
