use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::Mutex,
};

use crate::{
    api::types::{ChatCompletionRequest, ChatMessageContent, ResponsesRequest},
    error::AppError,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSummary {
    pub message_count: usize,
    pub system_message_count: usize,
    pub user_message_count: usize,
    pub assistant_message_count: usize,
    pub other_message_count: usize,
    pub input_text_chars: usize,
    pub has_system_prompt: bool,
    pub has_temperature: bool,
    pub has_top_p: bool,
    pub has_max_tokens: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum AccessLogEvent {
    #[serde(rename = "request_started")]
    RequestStarted {
        timestamp: String,
        request_id: String,
        method: String,
        path: String,
        api_kind: String,
        caller_id: Option<String>,
        public_model: String,
        stream: bool,
        request_summary: RequestSummary,
    },
    #[serde(rename = "upstream_attempt")]
    UpstreamAttempt {
        timestamp: String,
        request_id: String,
        attempt: usize,
        provider: String,
        upstream_model: String,
        public_model: String,
        result: String,
        latency_ms: u128,
        error_kind: Option<String>,
        error_message: Option<String>,
    },
    #[serde(rename = "request_finished")]
    RequestFinished {
        timestamp: String,
        request_id: String,
        api_kind: String,
        caller_id: Option<String>,
        public_model: String,
        final_provider: Option<String>,
        attempts: usize,
        stream: bool,
        status: String,
        status_code: u16,
        latency_ms: u128,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
    },
}

impl AccessLogEvent {
    pub fn request_started(
        request_id: impl Into<String>,
        method: impl Into<String>,
        path: impl Into<String>,
        api_kind: impl Into<String>,
        public_model: impl Into<String>,
        stream: bool,
        caller_id: Option<String>,
        request_summary: RequestSummary,
    ) -> Self {
        Self::RequestStarted {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            method: method.into(),
            path: path.into(),
            api_kind: api_kind.into(),
            caller_id,
            public_model: public_model.into(),
            stream,
            request_summary,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn request_finished(
        request_id: impl Into<String>,
        api_kind: impl Into<String>,
        public_model: impl Into<String>,
        stream: bool,
        status_code: u16,
        status: impl Into<String>,
        latency_ms: u128,
        attempts: usize,
        final_provider: Option<String>,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        caller_id: Option<String>,
    ) -> Self {
        Self::RequestFinished {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            api_kind: api_kind.into(),
            caller_id,
            public_model: public_model.into(),
            final_provider,
            attempts,
            stream,
            status: status.into(),
            status_code,
            latency_ms,
            input_tokens,
            output_tokens,
        }
    }

    pub fn upstream_attempt_success(
        request_id: impl Into<String>,
        attempt: usize,
        provider: impl Into<String>,
        upstream_model: impl Into<String>,
        public_model: impl Into<String>,
        latency_ms: u128,
    ) -> Self {
        Self::UpstreamAttempt {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            attempt,
            provider: provider.into(),
            upstream_model: upstream_model.into(),
            public_model: public_model.into(),
            result: "success".into(),
            latency_ms,
            error_kind: None,
            error_message: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upstream_attempt_failure(
        request_id: impl Into<String>,
        attempt: usize,
        provider: impl Into<String>,
        upstream_model: impl Into<String>,
        public_model: impl Into<String>,
        latency_ms: u128,
        error_kind: impl Into<String>,
        error_message: impl Into<String>,
    ) -> Self {
        Self::UpstreamAttempt {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.into(),
            attempt,
            provider: provider.into(),
            upstream_model: upstream_model.into(),
            public_model: public_model.into(),
            result: "failure".into(),
            latency_ms,
            error_kind: Some(error_kind.into()),
            error_message: Some(error_message.into()),
        }
    }
}

impl RequestSummary {
    pub fn from_chat_request(payload: &ChatCompletionRequest) -> Self {
        let mut summary = Self {
            message_count: payload.messages.len(),
            has_system_prompt: payload
                .messages
                .iter()
                .any(|message| matches!(message.role, crate::domain::request::UnifiedRole::System)),
            has_temperature: payload.temperature.is_some(),
            has_top_p: payload.top_p.is_some(),
            has_max_tokens: payload.max_tokens.is_some(),
            ..Self::default()
        };

        for message in &payload.messages {
            let len = match &message.content {
                ChatMessageContent::Text(text) => text.chars().count(),
            };
            summary.input_text_chars += len;
            match message.role {
                crate::domain::request::UnifiedRole::System => summary.system_message_count += 1,
                crate::domain::request::UnifiedRole::User => summary.user_message_count += 1,
                crate::domain::request::UnifiedRole::Assistant => {
                    summary.assistant_message_count += 1
                }
            }
        }

        summary
    }

    pub fn from_responses_request(payload: &ResponsesRequest) -> Self {
        Self {
            message_count: 1,
            user_message_count: 1,
            input_text_chars: payload.input.chars().count(),
            has_system_prompt: false,
            has_temperature: payload.temperature.is_some(),
            has_top_p: payload.top_p.is_some(),
            has_max_tokens: payload.max_output_tokens.is_some(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Default)]
pub struct AccessLogger {
    file: Option<Arc<Mutex<File>>>,
}

impl AccessLogger {
    pub async fn new(path: Option<PathBuf>) -> Result<Self, AppError> {
        let file = match path {
            Some(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                    .map_err(|error| {
                        AppError::upstream(format!("failed to open access log: {error}"))
                    })?;
                Some(Arc::new(Mutex::new(file)))
            }
            None => None,
        };

        Ok(Self { file })
    }

    pub async fn append(&self, event: AccessLogEvent) -> Result<(), AppError> {
        let Some(file) = &self.file else {
            return Ok(());
        };

        let mut file = file.lock().await;
        let mut line = serde_json::to_vec(&event).map_err(|error| {
            AppError::upstream(format!("failed to serialize access log: {error}"))
        })?;
        line.push(b'\n');
        file.write_all(&line)
            .await
            .map_err(|error| AppError::upstream(format!("failed to write access log: {error}")))?;
        file.flush()
            .await
            .map_err(|error| AppError::upstream(format!("failed to flush access log: {error}")))?;
        Ok(())
    }

    pub async fn append_warn(&self, event: AccessLogEvent, context: &str) {
        if let Err(error) = self.append(event).await {
            tracing::warn!(context = context, error = %error, "failed to write access log");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::types::{ChatCompletionRequest, ChatMessage, ChatMessageContent, ResponsesRequest},
        domain::request::UnifiedRole,
    };

    #[test]
    fn chat_summary_counts_roles_and_characters() {
        let payload = ChatCompletionRequest {
            model: "gpt-4.1".into(),
            messages: vec![
                ChatMessage {
                    role: UnifiedRole::System,
                    content: ChatMessageContent::Text("rules".into()),
                },
                ChatMessage {
                    role: UnifiedRole::User,
                    content: ChatMessageContent::Text("hello".into()),
                },
                ChatMessage {
                    role: UnifiedRole::Assistant,
                    content: ChatMessageContent::Text("hi".into()),
                },
            ],
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(128),
            stream: Some(false),
        };

        let summary = RequestSummary::from_chat_request(&payload);
        assert_eq!(summary.message_count, 3);
        assert_eq!(summary.system_message_count, 1);
        assert_eq!(summary.user_message_count, 1);
        assert_eq!(summary.assistant_message_count, 1);
        assert_eq!(summary.input_text_chars, 12);
        assert!(summary.has_system_prompt);
        assert!(summary.has_temperature);
        assert!(summary.has_max_tokens);
    }

    #[test]
    fn responses_summary_does_not_store_prompt_text() {
        let payload = ResponsesRequest {
            model: "gpt-4.1".into(),
            input: "secret prompt text".into(),
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            stream: Some(false),
        };

        let summary = RequestSummary::from_responses_request(&payload);
        let json = serde_json::to_string(&summary).unwrap();

        assert!(summary.input_text_chars >= 18);
        assert!(!json.contains("secret prompt text"));
    }

    #[tokio::test]
    async fn appends_access_log_event_as_jsonl() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let logger = AccessLogger::new(Some(path.path().to_path_buf()))
            .await
            .unwrap();

        logger
            .append(AccessLogEvent::request_started(
                "req_123",
                "POST",
                "/v1/chat/completions",
                "chat_completions",
                "gpt-4.1",
                false,
                None,
                RequestSummary::default(),
            ))
            .await
            .unwrap();

        let body = std::fs::read_to_string(path.path()).unwrap();
        assert!(body.contains("\"event\":\"request_started\""));
        assert!(body.contains("\"request_id\":\"req_123\""));
    }

    #[tokio::test]
    async fn disabled_access_logger_is_a_no_op() {
        let logger = AccessLogger::new(None).await.unwrap();
        logger
            .append(AccessLogEvent::request_finished(
                "req_123",
                "chat_completions",
                "gpt-4.1",
                false,
                200,
                "success",
                12,
                1,
                Some("openai".into()),
                None,
                None,
                None,
            ))
            .await
            .unwrap();
    }
}
