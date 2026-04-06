pub mod api;
pub mod app_state;
pub mod config;
pub mod domain;
pub mod error;
pub mod models;
pub mod providers;
pub mod router;
pub mod sse;

use std::sync::Arc;

use api::router as api_router;
use app_state::AppState;
use axum::{Router, routing::get};
use config::AppConfig;
use providers::ProviderFactory;

pub fn build_app(config: AppConfig) -> Router {
    let registry = models::ModelRegistry::from_configs(&config.models);
    let provider_factory = ProviderFactory::from_config(&config);
    let state = Arc::new(AppState::new(config, registry, provider_factory));

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(api_router(state))
}
