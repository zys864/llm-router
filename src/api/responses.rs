use std::{convert::Infallible, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    response::{IntoResponse, Response},
    routing::post,
};
use bytes::Bytes;
use futures::stream;

use crate::{
    api::types::{ResponsesRequest, ResponsesResponse},
    app_state::AppState,
    domain::request::UnifiedRequest,
    error::AppError,
    router::resolve_model,
    sse::encode_events,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/responses", post(create_response))
}

pub async fn create_response(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResponsesRequest>,
) -> Result<Response, AppError> {
    let route = resolve_model(&state.registry, &payload.model)?;
    let request_id = format!("resp_{}", uuid::Uuid::new_v4().simple());
    let request = UnifiedRequest::from_responses(payload, route.clone(), request_id.clone())?;
    let provider = state.provider_factory.for_route(&route)?;

    if request.stream {
        return stream_response(provider, request).await;
    }

    let response = provider.complete(request).await?;
    Ok(Json(ResponsesResponse::from_domain(
        request_id,
        response,
        route.public_name,
    ))
    .into_response())
}

async fn stream_response(
    provider: Arc<dyn crate::providers::ProviderAdapter>,
    request: UnifiedRequest,
) -> Result<Response, AppError> {
    let model = request.route.public_name.clone();
    let events = provider.stream(request).await?;
    let chunks = encode_events(&model, events);
    let body_stream = stream::iter(
        chunks
            .into_iter()
            .map(|chunk| Ok::<Bytes, Infallible>(Bytes::from(chunk))),
    );

    Ok(Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(body_stream))
        .unwrap())
}
