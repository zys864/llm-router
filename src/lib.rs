pub mod access_log;
pub mod admin;
pub mod api;
pub mod app_state;
pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod models;
pub mod outbound_audit;
pub mod pricing;
pub mod providers;
pub mod quota;
pub mod router;
pub mod sse;
pub mod usage;
pub mod usage_aggregate;

use std::sync::Arc;

use api::{admin_router, v1_router};
use app_state::AppState;
use axum::{Router, routing::get};
use config::AppConfig;
use providers::ProviderFactory;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use {
    access_log::AccessLogger, auth::AuthService, outbound_audit::OutboundAuditLogger,
    quota::QuotaStore, usage::UsageLogger, usage_aggregate::UsageAggregator,
};

pub async fn build_app(config: AppConfig) -> Router {
    let registry = models::ModelRegistry::from_configs(&config.models);
    let provider_factory =
        ProviderFactory::from_config(&config).expect("failed to initialize provider factory");
    let auth = AuthService::new(config.proxy_keys.clone());
    let quota = QuotaStore::new(config.proxy_keys.clone());
    let outbound_audit_logger =
        OutboundAuditLogger::new(config.outbound_audit_log_path.clone().map(Into::into))
            .await
            .expect("failed to initialize outbound audit logger");
    let usage_logger = UsageLogger::new(
        config.usage_log_path.clone().map(Into::into),
        outbound_audit_logger.clone(),
    )
    .await
    .expect("failed to initialize usage logger");
    let access_logger = AccessLogger::new(
        config.access_log_path.clone().map(Into::into),
        outbound_audit_logger.clone(),
    )
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
        outbound_audit_logger,
    ));

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(v1_router(state.clone()).layer(api_trace_layer()))
        .merge(admin_router(state))
}

#[cfg(test)]
fn should_trace_route(path: &str) -> bool {
    path.starts_with("/v1/")
}

fn api_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    DefaultMakeSpan,
    DefaultOnRequest,
    DefaultOnResponse,
> {
    TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(false),
        )
        .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn app_builds_without_panicking() {
        let _ = build_app(AppConfig::default()).await;
    }

    #[test]
    fn trace_logging_only_targets_v1_routes() {
        assert!(should_trace_route("/v1/models"));
        assert!(should_trace_route("/v1/chat/completions"));
        assert!(!should_trace_route("/healthz"));
        assert!(!should_trace_route("/admin/models"));
    }
}
