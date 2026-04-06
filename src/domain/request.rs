use serde::{Deserialize, Serialize};

use crate::{
    api::types::{ChatCompletionRequest, ChatMessage, ResponsesRequest},
    error::AppError,
    router::ModelRoute,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnifiedRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedMessage {
    pub role: UnifiedRole,
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiKind {
    ChatCompletions,
    Responses,
}

#[derive(Clone, Debug)]
pub struct UnifiedRequest {
    pub request_id: String,
    pub api_kind: ApiKind,
    pub model: String,
    pub route: ModelRoute,
    pub messages: Vec<UnifiedMessage>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stream: bool,
    pub caller_id: Option<String>,
}

impl UnifiedRequest {
    pub fn from_chat(
        request: ChatCompletionRequest,
        route: ModelRoute,
        request_id: impl Into<String>,
    ) -> Result<Self, AppError> {
        if request.messages.is_empty() {
            return Err(AppError::validation("messages must not be empty"));
        }

        Ok(Self {
            request_id: request_id.into(),
            api_kind: ApiKind::ChatCompletions,
            model: route.public_name.clone(),
            route,
            messages: request
                .messages
                .into_iter()
                .map(Self::from_chat_message)
                .collect::<Result<Vec<_>, _>>()?,
            temperature: request.temperature,
            max_output_tokens: request.max_tokens,
            stream: request.stream.unwrap_or(false),
            caller_id: None,
        })
    }

    pub fn from_responses(
        request: ResponsesRequest,
        route: ModelRoute,
        request_id: impl Into<String>,
    ) -> Result<Self, AppError> {
        if request.input.trim().is_empty() {
            return Err(AppError::validation("input must not be empty"));
        }

        Ok(Self {
            request_id: request_id.into(),
            api_kind: ApiKind::Responses,
            model: route.public_name.clone(),
            route,
            messages: vec![UnifiedMessage {
                role: UnifiedRole::User,
                content: request.input,
            }],
            temperature: request.temperature,
            max_output_tokens: request.max_output_tokens,
            stream: request.stream.unwrap_or(false),
            caller_id: None,
        })
    }

    pub fn with_caller(mut self, caller_id: Option<String>) -> Self {
        self.caller_id = caller_id;
        self
    }

    fn from_chat_message(message: ChatMessage) -> Result<UnifiedMessage, AppError> {
        Ok(UnifiedMessage {
            role: message.role,
            content: message.content.as_text()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ChatMessageContent;
    use crate::providers::ProviderKind;

    #[test]
    fn converts_chat_request_into_domain_request() {
        let request = ChatCompletionRequest {
            model: "gpt-4.1".into(),
            messages: vec![ChatMessage {
                role: UnifiedRole::User,
                content: ChatMessageContent::Text("hello".into()),
            }],
            temperature: Some(0.2),
            max_tokens: Some(64),
            stream: Some(false),
        };

        let domain = UnifiedRequest::from_chat(request, sample_route(), "req_123").unwrap();
        assert_eq!(domain.model, "gpt-4.1");
        assert_eq!(domain.messages.len(), 1);
    }

    fn sample_route() -> ModelRoute {
        ModelRoute {
            provider: ProviderKind::OpenAi,
            public_name: "gpt-4.1".into(),
            upstream_name: "gpt-4.1".into(),
            capabilities: crate::config::ModelCapabilities::all(),
        }
    }
}
