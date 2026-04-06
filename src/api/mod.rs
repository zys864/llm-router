use std::sync::Arc;

use axum::Router;

use crate::app_state::AppState;

pub mod chat_completions;
pub mod models;
pub mod responses;
pub mod types;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(models::router())
        .merge(chat_completions::router())
        .merge(responses::router())
        .with_state(state)
}
