use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::get};

use crate::{api::types::ModelsResponse, app_state::AppState, error::AppError};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models", get(list_models))
}

pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, AppError> {
    Ok(Json(ModelsResponse::from_registry(&state.registry)))
}
