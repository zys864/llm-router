use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::post,
};

use crate::{
    access_log::{AccessLogEvent, RequestSummary},
    api::{
        authenticate_request,
        types::{ResponsesRequest, ResponsesResponse},
    },
    app_state::AppState,
    domain::request::UnifiedRequest,
    error::AppError,
    router::{Capability, resolve_route_plan},
    sse::sse_body,
    usage::UsageRecord,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/responses", post(create_response))
}

pub async fn create_response(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResponsesRequest>,
) -> Result<Response, AppError> {
    let caller = authenticate_request(&state, &headers)?;
    let request_id = format!("resp_{}", uuid::Uuid::new_v4().simple());
    let request_started_at = std::time::Instant::now();
    let plan = resolve_route_plan(
        &state.registry,
        &payload.model,
        if payload.stream.unwrap_or(false) {
            Capability::Streaming
        } else {
            Capability::Responses
        },
    )?;
    state
        .access_logger
        .append_warn(
            AccessLogEvent::request_started(
                request_id.clone(),
                "POST",
                "/v1/responses",
                "responses",
                payload.model.clone(),
                payload.stream.unwrap_or(false),
                caller.as_ref().map(|caller| caller.id.clone()),
                RequestSummary::from_responses_request(&payload),
            ),
            "request_started",
        )
        .await;
    if let Err(error) = state.quota.try_acquire_optional(caller.as_ref()).await {
        state
            .access_logger
            .append_warn(
                AccessLogEvent::request_finished(
                    request_id.clone(),
                    "responses",
                    payload.model.clone(),
                    payload.stream.unwrap_or(false),
                    error.status_code().as_u16(),
                    "error",
                    request_started_at.elapsed().as_millis(),
                    0,
                    None,
                    None,
                    None,
                    caller.as_ref().map(|caller| caller.id.clone()),
                ),
                "request_finished",
            )
            .await;
        return Err(error);
    }

    let mut attempts = 0usize;
    let mut last_error = None;

    for route in plan.targets {
        attempts += 1;
        let request =
            UnifiedRequest::from_responses(payload.clone(), route.clone(), request_id.clone())?
                .with_caller(caller.as_ref().map(|caller| caller.id.clone()));
        let provider = state.provider_factory.for_route(&route)?;

        if request.stream {
            let attempt_started_at = std::time::Instant::now();
            match provider.stream(request.clone()).await {
                Ok(stream) => {
                    state
                        .access_logger
                        .append_warn(
                            AccessLogEvent::upstream_attempt_success(
                                request_id.clone(),
                                attempts,
                                route.provider.as_str(),
                                route.upstream_name.clone(),
                                route.public_name.clone(),
                                attempt_started_at.elapsed().as_millis(),
                            ),
                            "upstream_attempt",
                        )
                        .await;
                    state
                        .access_logger
                        .append_warn(
                            AccessLogEvent::request_finished(
                                request_id.clone(),
                                "responses",
                                route.public_name.clone(),
                                true,
                                200,
                                "success",
                                request_started_at.elapsed().as_millis(),
                                attempts,
                                Some(route.provider.as_str().to_string()),
                                None,
                                None,
                                caller.as_ref().map(|caller| caller.id.clone()),
                            ),
                            "request_finished",
                        )
                        .await;
                    state
                        .usage_logger
                        .append(UsageRecord::success(
                            request_id.clone(),
                            request.model.clone(),
                            route.provider.as_str(),
                            attempts,
                            "responses",
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
                    state
                        .access_logger
                        .append_warn(
                            AccessLogEvent::upstream_attempt_failure(
                                request_id.clone(),
                                attempts,
                                route.provider.as_str(),
                                route.upstream_name.clone(),
                                route.public_name.clone(),
                                attempt_started_at.elapsed().as_millis(),
                                error.error_type(),
                                error.to_string(),
                            ),
                            "upstream_attempt",
                        )
                        .await;
                    last_error = Some(error);
                    continue;
                }
            }
        }

        let attempt_started_at = std::time::Instant::now();
        match provider.complete(request.clone()).await {
            Ok(response) => {
                let usage = response.usage.clone();
                state
                    .access_logger
                    .append_warn(
                        AccessLogEvent::upstream_attempt_success(
                            request_id.clone(),
                            attempts,
                            route.provider.as_str(),
                            route.upstream_name.clone(),
                            route.public_name.clone(),
                            attempt_started_at.elapsed().as_millis(),
                        ),
                        "upstream_attempt",
                    )
                    .await;
                state
                    .access_logger
                    .append_warn(
                        AccessLogEvent::request_finished(
                            request_id.clone(),
                            "responses",
                            route.public_name.clone(),
                            false,
                            200,
                            "success",
                            request_started_at.elapsed().as_millis(),
                            attempts,
                            Some(response.provider.clone()),
                            usage.as_ref().and_then(|usage| usage.input_tokens),
                            usage.as_ref().and_then(|usage| usage.output_tokens),
                            caller.as_ref().map(|caller| caller.id.clone()),
                        ),
                        "request_finished",
                    )
                    .await;
                state
                    .usage_logger
                    .append(UsageRecord::success(
                        request_id.clone(),
                        request.model.clone(),
                        response.provider.clone(),
                        attempts,
                        "responses",
                        false,
                        caller.as_ref().map(|caller| caller.id.clone()),
                    ))
                    .await?;
                return Ok(Json(ResponsesResponse::from_domain(
                    request_id.clone(),
                    response,
                    route.public_name,
                ))
                .into_response());
            }
            Err(error) => {
                state
                    .access_logger
                    .append_warn(
                        AccessLogEvent::upstream_attempt_failure(
                            request_id.clone(),
                            attempts,
                            route.provider.as_str(),
                            route.upstream_name.clone(),
                            route.public_name.clone(),
                            attempt_started_at.elapsed().as_millis(),
                            error.error_type(),
                            error.to_string(),
                        ),
                        "upstream_attempt",
                    )
                    .await;
                last_error = Some(error);
            }
        }
    }

    let status_code = last_error
        .as_ref()
        .map(|error| error.status_code().as_u16())
        .unwrap_or(axum::http::StatusCode::BAD_GATEWAY.as_u16());
    state
        .access_logger
        .append_warn(
            AccessLogEvent::request_finished(
                request_id.clone(),
                "responses",
                payload.model.clone(),
                payload.stream.unwrap_or(false),
                status_code,
                "error",
                request_started_at.elapsed().as_millis(),
                attempts,
                None,
                None,
                None,
                caller.as_ref().map(|caller| caller.id.clone()),
            ),
            "request_finished",
        )
        .await;
    state
        .usage_logger
        .append(UsageRecord::failure(
            request_id,
            payload.model,
            attempts,
            "responses",
            payload.stream.unwrap_or(false),
            "upstream_error",
            caller.as_ref().map(|caller| caller.id.clone()),
        ))
        .await?;
    Err(last_error.unwrap_or_else(|| AppError::upstream("all route attempts failed")))
}
