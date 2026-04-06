use std::{collections::HashMap, path::Path, sync::Arc};

use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use serde::Serialize;

use crate::{
    api::authenticate_request,
    app_state::AppState,
    config::ModelPricing,
    error::AppError,
    usage_aggregate::{UsageAggregator, UsageSummary},
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/models", get(models))
        .route("/admin/callers", get(callers))
        .route("/admin/usage/summary", get(usage_summary))
}

#[derive(Serialize)]
pub struct AdminModelsResponse {
    pub data: Vec<AdminModelObject>,
}

#[derive(Serialize)]
pub struct AdminModelObject {
    pub id: String,
    pub pricing: Option<ModelPricing>,
    pub target_count: usize,
}

#[derive(Serialize)]
pub struct AdminCallersResponse {
    pub data: Vec<AdminCallerObject>,
}

#[derive(Serialize)]
pub struct AdminCallerObject {
    pub id: String,
    pub max_requests: u64,
    pub requests_used: u64,
    pub requests_remaining: u64,
}

#[derive(Serialize)]
pub struct AdminUsageSummaryResponse {
    pub total_requests: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub by_caller: HashMap<String, usize>,
    pub by_model: HashMap<String, usize>,
    pub by_provider: HashMap<String, usize>,
    pub estimated_cost_total: f64,
}

pub async fn models(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AdminModelsResponse>, AppError> {
    authenticate_request(&state, &headers)?;
    Ok(Json(AdminModelsResponse {
        data: state
            .registry
            .all()
            .iter()
            .map(|record| AdminModelObject {
                id: record.public_name.clone(),
                pricing: record.pricing.clone(),
                target_count: record.targets.len(),
            })
            .collect(),
    }))
}

pub async fn callers(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AdminCallersResponse>, AppError> {
    authenticate_request(&state, &headers)?;
    let snapshot = state.quota.snapshot().await;
    Ok(Json(AdminCallersResponse {
        data: snapshot
            .into_iter()
            .map(|(id, (used, limit))| AdminCallerObject {
                id,
                max_requests: limit,
                requests_used: used,
                requests_remaining: limit.saturating_sub(used),
            })
            .collect(),
    }))
}

pub async fn usage_summary(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AdminUsageSummaryResponse>, AppError> {
    authenticate_request(&state, &headers)?;
    let summary = if let Some(path) = &state.config.usage_log_path {
        if Path::new(path).exists() {
            UsageAggregator::from_path(Path::new(path))
                .await?
                .summarize()?
        } else {
            UsageSummary::default()
        }
    } else {
        UsageSummary::default()
    };

    Ok(Json(AdminUsageSummaryResponse {
        total_requests: summary.total_requests,
        success_count: summary.success_count,
        failure_count: summary.failure_count,
        by_caller: summary.by_caller,
        by_model: summary.by_model,
        by_provider: summary.by_provider,
        estimated_cost_total: summary.estimated_cost_total,
    }))
}
