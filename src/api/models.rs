use std::sync::Arc;

use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};

use crate::{
    api::{authenticate_request, types::ModelsResponse},
    app_state::AppState,
    error::AppError,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models", get(list_models))
}

pub async fn list_models(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, AppError> {
    authenticate_request(&state, &headers)?;
    Ok(Json(ModelsResponse::from_registry(&state.registry)))
}
