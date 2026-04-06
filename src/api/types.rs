use serde::{Deserialize, Serialize};

use crate::{domain::request::UnifiedRole, error::AppError, models::ModelRegistry};

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageContent {
    Text(String),
}

impl ChatMessageContent {
    pub fn as_text(self) -> Result<String, AppError> {
        match self {
            Self::Text(text) => Ok(text),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatMessage {
    pub role: UnifiedRole,
    pub content: ChatMessageContent,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: String,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelObject>,
}

#[derive(Debug, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub owned_by: String,
    pub metadata: ModelMetadataObject,
}

#[derive(Debug, Serialize)]
pub struct ModelMetadataObject {
    pub streaming: bool,
    pub target_count: usize,
}

impl ModelsResponse {
    pub fn from_registry(registry: &ModelRegistry) -> Self {
        Self {
            object: "list".into(),
            data: registry
                .all()
                .iter()
                .map(|record| ModelObject {
                    id: record.public_name.clone(),
                    object: "model".into(),
                    owned_by: record
                        .targets
                        .first()
                        .map(|target| target.provider.as_str().to_string())
                        .unwrap_or_else(|| "unknown".into()),
                    metadata: ModelMetadataObject {
                        streaming: record
                            .targets
                            .iter()
                            .any(|target| target.capabilities.streaming),
                        target_count: record.targets.len(),
                    },
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<UsageObject>,
}

#[derive(Debug, Serialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatChoiceMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ResponsesResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub output_text: String,
    pub usage: Option<UsageObject>,
}

#[derive(Debug, Serialize)]
pub struct UsageObject {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

impl ChatCompletionResponse {
    pub fn from_domain(
        request_id: impl Into<String>,
        response: crate::domain::response::UnifiedResponse,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: request_id.into(),
            object: "chat.completion".into(),
            model: model.into(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatChoiceMessage {
                    role: "assistant".into(),
                    content: response.text,
                },
                finish_reason: response.finish_reason,
            }],
            usage: response.usage.map(UsageObject::from),
        }
    }
}

impl ResponsesResponse {
    pub fn from_domain(
        request_id: impl Into<String>,
        response: crate::domain::response::UnifiedResponse,
        model: impl Into<String>,
    ) -> Self {
        Self {
            id: request_id.into(),
            object: "response".into(),
            model: model.into(),
            output_text: response.text,
            usage: response.usage.map(UsageObject::from),
        }
    }
}

impl From<crate::domain::response::UnifiedUsage> for UsageObject {
    fn from(value: crate::domain::response::UnifiedUsage) -> Self {
        Self {
            prompt_tokens: value.input_tokens,
            completion_tokens: value.output_tokens,
            total_tokens: value.total_tokens,
        }
    }
}
