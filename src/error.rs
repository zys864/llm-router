use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("model `{0}` not found")]
    ModelNotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider `{0}` is not configured")]
    ProviderNotConfigured(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("timeout")]
    Timeout,
}

impl AppError {
    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound(model.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::Upstream(message.into())
    }

    pub fn request_id(&self) -> String {
        format!("req_{}", uuid::Uuid::new_v4().simple())
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::ModelNotFound(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::ProviderNotConfigured(_) => StatusCode::BAD_GATEWAY,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            Self::ModelNotFound(_) | Self::Validation(_) => "invalid_request_error",
            Self::ProviderNotConfigured(_) | Self::Upstream(_) | Self::Timeout => "server_error",
        }
    }

    pub fn into_response_body(self, request_id: impl Into<String>) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorBody {
                message: self.to_string(),
                r#type: self.error_type().to_string(),
            },
            request_id: request_id.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let request_id = self.request_id();
        (status, Json(self.into_response_body(request_id))).into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
    pub request_id: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub message: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_model_not_found_error() {
        let body = AppError::model_not_found("missing").into_response_body("req_123");
        assert_eq!(body.error.message, "model `missing` not found");
        assert_eq!(body.error.r#type, "invalid_request_error");
        assert_eq!(body.request_id, "req_123");
    }
}
