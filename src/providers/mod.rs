use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::{Client, Proxy};
use serde::{Deserialize, Serialize};

use crate::{
    config::AppConfig,
    domain::{
        request::{ApiKind, UnifiedMessage, UnifiedRequest, UnifiedRole},
        response::{EventStream, UnifiedResponse, UnifiedUsage},
    },
    error::AppError,
    router::ModelRoute,
};

pub mod anthropic;
pub mod gemini;
pub mod openai;
pub mod streaming;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
    async fn stream(&self, request: UnifiedRequest) -> Result<EventStream, AppError>;
}

#[derive(Clone)]
pub struct ProviderFactory {
    providers: HashMap<ProviderKind, Arc<dyn ProviderAdapter>>,
}

impl ProviderFactory {
    pub fn from_config(config: &AppConfig) -> Result<Self, AppError> {
        let mut client = Client::builder().timeout(Duration::from_secs(config.request_timeout_secs));
        if let Some(proxy_url) = &config.upstream_proxy_url {
            let proxy = Proxy::all(proxy_url).map_err(|error| {
                AppError::validation(format!("invalid UPSTREAM_PROXY_URL `{proxy_url}`: {error}"))
            })?;
            client = client.proxy(proxy);
        }
        let client = client
            .build()
            .map_err(|error| AppError::upstream(format!("failed to build reqwest client: {error}")))?;

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
        } else if config.enable_provider_default_auth_fallback {
            providers.insert(
                ProviderKind::Anthropic,
                Arc::new(UnimplementedFallbackProvider::new(ProviderKind::Anthropic)),
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
        } else if config.enable_provider_default_auth_fallback {
            providers.insert(
                ProviderKind::Gemini,
                Arc::new(UnimplementedFallbackProvider::new(ProviderKind::Gemini)),
            );
        }

        Ok(Self { providers })
    }

    pub fn for_route(&self, route: &ModelRoute) -> Result<Arc<dyn ProviderAdapter>, AppError> {
        self.providers
            .get(&route.provider)
            .cloned()
            .ok_or_else(|| AppError::ProviderNotConfigured(route.provider.as_str().into()))
    }
}

struct UnimplementedFallbackProvider {
    provider: ProviderKind,
}

impl UnimplementedFallbackProvider {
    fn new(provider: ProviderKind) -> Self {
        Self { provider }
    }

    fn error(&self) -> AppError {
        AppError::not_implemented(format!(
            "default auth fallback for provider `{}`",
            self.provider.as_str()
        ))
    }
}

#[async_trait]
impl ProviderAdapter for UnimplementedFallbackProvider {
    async fn complete(&self, _request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        Err(self.error())
    }

    async fn stream(&self, _request: UnifiedRequest) -> Result<EventStream, AppError> {
        Err(self.error())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::ModelCapabilities, router::ModelRoute};

    #[tokio::test]
    async fn returns_not_implemented_for_anthropic_default_auth_fallback() {
        let config = AppConfig {
            enable_provider_default_auth_fallback: true,
            ..AppConfig::default()
        };
        let factory = ProviderFactory::from_config(&config).unwrap();
        let provider = factory
            .for_route(&ModelRoute {
                provider: ProviderKind::Anthropic,
                public_name: "claude-sonnet-4".into(),
                upstream_name: "claude-sonnet-4".into(),
                capabilities: ModelCapabilities::all(),
            })
            .unwrap();

        let error = provider.complete(sample_request(ProviderKind::Anthropic)).await.unwrap_err();
        assert!(matches!(error, AppError::NotImplemented(_)));
    }

    #[test]
    fn returns_validation_error_for_invalid_upstream_proxy_url() {
        let config = AppConfig {
            upstream_proxy_url: Some("://bad-proxy".into()),
            ..AppConfig::default()
        };

        let error = match ProviderFactory::from_config(&config) {
            Ok(_) => panic!("expected invalid proxy config to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, AppError::Validation(_)));
        assert!(error.to_string().contains("UPSTREAM_PROXY_URL"));
    }

    fn sample_request(provider: ProviderKind) -> UnifiedRequest {
        UnifiedRequest {
            request_id: "req_123".into(),
            api_kind: ApiKind::ChatCompletions,
            route: ModelRoute {
                provider,
                public_name: "sample".into(),
                upstream_name: "sample".into(),
                capabilities: ModelCapabilities::all(),
            },
            model: "sample".into(),
            messages: vec![UnifiedMessage {
                role: UnifiedRole::User,
                content: "hello".into(),
            }],
            temperature: None,
            max_output_tokens: None,
            stream: false,
            caller_id: None,
        }
    }
}
