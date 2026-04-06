use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::post,
};

use crate::{
    api::{
        authenticate_request,
        types::{ChatCompletionRequest, ChatCompletionResponse},
    },
    app_state::AppState,
    domain::request::UnifiedRequest,
    error::AppError,
    router::{Capability, resolve_route_plan},
    sse::sse_body,
    usage::UsageRecord,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/chat/completions", post(create_chat_completion))
}

pub async fn create_chat_completion(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    let caller = authenticate_request(&state, &headers)?;
    let request_id = format!("chatcmpl_{}", uuid::Uuid::new_v4().simple());
    let plan = resolve_route_plan(
        &state.registry,
        &payload.model,
        if payload.stream.unwrap_or(false) {
            Capability::Streaming
        } else {
            Capability::ChatCompletions
        },
    )?;
    state.quota.try_acquire_optional(caller.as_ref()).await?;

    let mut attempts = 0usize;
    let mut last_error = None;

    for route in plan.targets {
        attempts += 1;
        let request =
            UnifiedRequest::from_chat(payload.clone(), route.clone(), request_id.clone())?
                .with_caller(caller.as_ref().map(|caller| caller.id.clone()));
        let provider = state.provider_factory.for_route(&route)?;

        if request.stream {
            match provider.stream(request.clone()).await {
                Ok(stream) => {
                    state
                        .usage_logger
                        .append(UsageRecord::success(
                            request_id.clone(),
                            request.model.clone(),
                            route.provider.as_str(),
                            attempts,
                            "chat_completions",
                            true,
                            caller.as_ref().map(|caller| caller.id.clone()),
                        ))
                        .await?;
                    return Ok(Response::builder()
                        .header("content-type", "text/event-stream")
                        .body(sse_body(route.public_name.clone(), stream))
                        .unwrap());
                }
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            }
        }

        match provider.complete(request.clone()).await {
            Ok(response) => {
                state
                    .usage_logger
                    .append(UsageRecord::success(
                        request_id.clone(),
                        request.model.clone(),
                        response.provider.clone(),
                        attempts,
                        "chat_completions",
                        false,
                        caller.as_ref().map(|caller| caller.id.clone()),
                    ))
                    .await?;
                return Ok(Json(ChatCompletionResponse::from_domain(
                    request_id.clone(),
                    response,
                    route.public_name,
                ))
                .into_response());
            }
            Err(error) => last_error = Some(error),
        }
    }

    state
        .usage_logger
        .append(UsageRecord::failure(
            request_id,
            payload.model,
            attempts,
            "chat_completions",
            payload.stream.unwrap_or(false),
            "upstream_error",
            caller.as_ref().map(|caller| caller.id.clone()),
        ))
        .await?;
    Err(last_error.unwrap_or_else(|| AppError::upstream("all route attempts failed")))
}
