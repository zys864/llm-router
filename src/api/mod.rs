use std::sync::Arc;

use axum::Router;

use axum::http::HeaderMap;

use crate::{admin, app_state::AppState, auth::AuthenticatedCaller, error::AppError};

pub mod chat_completions;
pub mod models;
pub mod responses;
pub mod types;

pub fn v1_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(models::router())
        .merge(chat_completions::router())
        .merge(responses::router())
        .with_state(state)
}

pub fn admin_router(state: Arc<AppState>) -> Router {
    Router::new().merge(admin::router()).with_state(state)
}

pub fn authenticate_request(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<Option<AuthenticatedCaller>, AppError> {
    state.auth.authenticate_header(headers)
}
