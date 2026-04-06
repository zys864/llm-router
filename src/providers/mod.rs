use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    config::AppConfig,
    domain::{
        request::{ApiKind, UnifiedMessage, UnifiedRequest, UnifiedRole},
        response::{StreamEvent, UnifiedResponse, UnifiedUsage},
    },
    error::AppError,
    router::ModelRoute,
};

pub mod anthropic;
pub mod gemini;
pub mod openai;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
}

impl ProviderKind {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            _ => Err(AppError::validation(format!(
                "unsupported provider `{value}`"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn complete(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError>;
    async fn stream(&self, request: UnifiedRequest) -> Result<Vec<StreamEvent>, AppError>;
}

#[derive(Clone)]
pub struct ProviderFactory {
    providers: HashMap<ProviderKind, Arc<dyn ProviderAdapter>>,
}

impl ProviderFactory {
    pub fn from_config(config: &AppConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .expect("failed to build reqwest client");

        let mut providers: HashMap<ProviderKind, Arc<dyn ProviderAdapter>> = HashMap::new();

        if let Some(api_key) = &config.openai_api_key {
            providers.insert(
                ProviderKind::OpenAi,
                Arc::new(openai::OpenAiProvider::new(
                    client.clone(),
                    api_key.clone(),
                    config.openai_base_url.clone(),
                )),
            );
        }

        if let Some(api_key) = &config.anthropic_api_key {
            providers.insert(
                ProviderKind::Anthropic,
                Arc::new(anthropic::AnthropicProvider::new(
                    client.clone(),
                    api_key.clone(),
                    config.anthropic_base_url.clone(),
                )),
            );
        }

        if let Some(api_key) = &config.gemini_api_key {
            providers.insert(
                ProviderKind::Gemini,
                Arc::new(gemini::GeminiProvider::new(
                    client,
                    api_key.clone(),
                    config.gemini_base_url.clone(),
                )),
            );
        }

        Self { providers }
    }

    pub fn for_route(&self, route: &ModelRoute) -> Result<Arc<dyn ProviderAdapter>, AppError> {
        self.providers
            .get(&route.provider)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotConfigured(route.provider.as_str().into()))
    }
}

pub(crate) fn json_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)?
        } else {
            current.get(*segment)?
        };
    }
    current.as_str().map(ToString::to_string)
}

pub(crate) fn json_u32(value: &serde_json::Value, path: &[&str]) -> Option<u32> {
    let mut current = value;
    for segment in path {
        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)?
        } else {
            current.get(*segment)?
        };
    }
    current.as_u64().map(|value| value as u32)
}

pub(crate) fn system_prompt(messages: &[UnifiedMessage]) -> Option<String> {
    messages.iter().find_map(|message| {
        if message.role == UnifiedRole::System {
            Some(message.content.clone())
        } else {
            None
        }
    })
}

pub(crate) fn non_system_messages(messages: &[UnifiedMessage]) -> Vec<&UnifiedMessage> {
    messages
        .iter()
        .filter(|message| message.role != UnifiedRole::System)
        .collect()
}

pub(crate) fn simple_stream(response: &UnifiedResponse) -> Vec<StreamEvent> {
    vec![
        StreamEvent::Started,
        StreamEvent::DeltaText(response.text.clone()),
        StreamEvent::Completed,
    ]
}

pub(crate) fn usage(input_tokens: Option<u32>, output_tokens: Option<u32>) -> Option<UnifiedUsage> {
    let total_tokens = match (input_tokens, output_tokens) {
        (Some(input), Some(output)) => Some(input + output),
        _ => None,
    };

    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        None
    } else {
        Some(UnifiedUsage {
            input_tokens,
            output_tokens,
            total_tokens,
        })
    }
}

pub(crate) fn map_reqwest_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::Timeout
    } else {
        AppError::upstream(error.to_string())
    }
}

pub(crate) fn endpoint_path(kind: ApiKind) -> &'static str {
    match kind {
        ApiKind::ChatCompletions => "/v1/chat/completions",
        ApiKind::Responses => "/v1/responses",
    }
}
