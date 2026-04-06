pub mod access_log;
pub mod admin;
pub mod api;
pub mod app_state;
pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod models;
pub mod pricing;
pub mod providers;
pub mod quota;
pub mod router;
pub mod sse;
pub mod usage;
pub mod usage_aggregate;

use std::sync::Arc;

use api::router as api_router;
use app_state::AppState;
use axum::{Router, routing::get};
use config::AppConfig;
use providers::ProviderFactory;
use tower_http::trace::TraceLayer;
use {
    access_log::AccessLogger, auth::AuthService, quota::QuotaStore, usage::UsageLogger,
    usage_aggregate::UsageAggregator,
};

pub async fn build_app(config: AppConfig) -> Router {
    let registry = models::ModelRegistry::from_configs(&config.models);
    let provider_factory = ProviderFactory::from_config(&config)
        .expect("failed to initialize provider factory");
    let auth = AuthService::new(config.proxy_keys.clone());
    let quota = QuotaStore::new(config.proxy_keys.clone());
    let usage_logger = UsageLogger::new(config.usage_log_path.clone().map(Into::into))
        .await
        .expect("failed to initialize usage logger");
    let access_logger = AccessLogger::new(config.access_log_path.clone().map(Into::into))
        .await
        .expect("failed to initialize access logger");
    if let Some(path) = &config.usage_log_path {
        if std::path::Path::new(path).exists() {
            let aggregator = UsageAggregator::from_path(std::path::Path::new(path))
                .await
                .expect("failed to recover usage log");
            quota
                .seed_usage(aggregator.recover_success_counts())
                .await
                .expect("failed to seed quota state");
        }
    }
    let state = Arc::new(AppState::new(
        config,
        registry,
        provider_factory,
        auth,
        quota,
        usage_logger,
        access_logger,
    ));

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(api_router(state))
        .layer(TraceLayer::new_for_http())
}
