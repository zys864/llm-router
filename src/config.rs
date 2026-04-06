use std::env;

use crate::{error::AppError, providers::ProviderKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelConfig {
    pub public_name: String,
    pub provider: ProviderKind,
    pub upstream_name: String,
}

impl ModelConfig {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, AppError> {
        let value = value.as_ref();
        let (public_name, provider_and_model) = value.split_once('=').ok_or_else(|| {
            AppError::validation(format!(
                "invalid model mapping `{value}`; expected public=provider:upstream"
            ))
        })?;
        let (provider, upstream_name) = provider_and_model.split_once(':').ok_or_else(|| {
            AppError::validation(format!(
                "invalid provider mapping `{value}`; expected provider:upstream"
            ))
        })?;

        Ok(Self {
            public_name: public_name.to_string(),
            provider: ProviderKind::parse(provider)?,
            upstream_name: upstream_name.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub request_timeout_secs: u64,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openai_base_url: String,
    pub anthropic_base_url: String,
    pub gemini_base_url: String,
    pub models: Vec<ModelConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".to_string(),
            request_timeout_secs: 30,
            openai_api_key: None,
            anthropic_api_key: None,
            gemini_api_key: None,
            openai_base_url: "https://api.openai.com".to_string(),
            anthropic_base_url: "https://api.anthropic.com".to_string(),
            gemini_base_url: "https://generativelanguage.googleapis.com".to_string(),
            models: vec![],
        }
    }
}

impl AppConfig {
    pub fn from_parts(
        bind_addr: &str,
        request_timeout_secs: u64,
        raw_models: Vec<String>,
    ) -> Result<Self, AppError> {
        let mut config = Self {
            bind_addr: bind_addr.to_string(),
            request_timeout_secs,
            ..Self::default()
        };
        config.models = raw_models
            .into_iter()
            .map(ModelConfig::parse)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(config)
    }

    pub fn from_env() -> Result<Self, AppError> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
        let request_timeout_secs = env::var("REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(30);
        let raw_models = env::var("MODEL_MAPPINGS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .filter(|item| !item.trim().is_empty())
                    .map(|item| item.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut config = Self::from_parts(&bind_addr, request_timeout_secs, raw_models)?;
        config.openai_api_key = env::var("OPENAI_API_KEY").ok();
        config.anthropic_api_key = env::var("ANTHROPIC_API_KEY").ok();
        config.gemini_api_key = env::var("GEMINI_API_KEY").ok();
        config.openai_base_url = env::var("OPENAI_BASE_URL").unwrap_or(config.openai_base_url);
        config.anthropic_base_url =
            env::var("ANTHROPIC_BASE_URL").unwrap_or(config.anthropic_base_url);
        config.gemini_base_url = env::var("GEMINI_BASE_URL").unwrap_or(config.gemini_base_url);
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_registry_entries() {
        let config =
            AppConfig::from_parts("127.0.0.1:3000", 30, vec!["gpt-4.1=openai:gpt-4.1".into()])
                .unwrap();

        assert_eq!(config.models.len(), 1);
        assert_eq!(config.models[0].public_name, "gpt-4.1");
        assert_eq!(config.models[0].provider, ProviderKind::OpenAi);
    }
}
