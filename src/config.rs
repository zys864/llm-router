use std::{env, fs, path::{Path, PathBuf}};

use serde::{Deserialize, Serialize};

use crate::{error::AppError, providers::ProviderKind};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ModelCapabilities {
    pub chat_completions: bool,
    pub responses: bool,
    pub streaming: bool,
}

impl ModelCapabilities {
    pub fn all() -> Self {
        Self {
            chat_completions: true,
            responses: true,
            streaming: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModelTargetConfig {
    pub provider: ProviderKind,
    pub upstream_name: String,
    pub priority: u32,
    #[serde(default = "ModelCapabilities::all")]
    pub capabilities: ModelCapabilities,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ModelConfig {
    pub public_name: String,
    #[serde(default = "ModelCapabilities::all")]
    pub capabilities: ModelCapabilities,
    pub pricing: Option<ModelPricing>,
    pub targets: Vec<ModelTargetConfig>,
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
            capabilities: ModelCapabilities::all(),
            pricing: None,
            targets: vec![ModelTargetConfig {
                provider: ProviderKind::parse(provider)?,
                upstream_name: upstream_name.to_string(),
                priority: 100,
                capabilities: ModelCapabilities::all(),
            }],
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ProxyKeyConfig {
    pub id: String,
    pub api_key: String,
    pub max_requests: u64,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_addr: String,
    pub request_timeout_secs: u64,
    pub enable_provider_default_auth_fallback: bool,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openai_base_url: String,
    pub anthropic_base_url: String,
    pub gemini_base_url: String,
    pub model_config_path: Option<String>,
    pub proxy_api_keys_path: Option<String>,
    pub usage_log_path: Option<String>,
    pub access_log_path: Option<String>,
    pub proxy_keys: Vec<ProxyKeyConfig>,
    pub models: Vec<ModelConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".to_string(),
            request_timeout_secs: 30,
            enable_provider_default_auth_fallback: false,
            openai_api_key: None,
            anthropic_api_key: None,
            gemini_api_key: None,
            openai_base_url: "https://api.openai.com".to_string(),
            anthropic_base_url: "https://api.anthropic.com".to_string(),
            gemini_base_url: "https://generativelanguage.googleapis.com".to_string(),
            model_config_path: None,
            proxy_api_keys_path: None,
            usage_log_path: None,
            access_log_path: None,
            proxy_keys: vec![],
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

    pub fn from_test_paths(
        model_config_path: impl AsRef<Path>,
        proxy_keys_path: Option<impl AsRef<Path>>,
        usage_log_path: Option<impl AsRef<Path>>,
    ) -> Result<Self, AppError> {
        let mut config = Self::default();
        config.model_config_path = Some(model_config_path.as_ref().display().to_string());
        config.proxy_api_keys_path = proxy_keys_path
            .as_ref()
            .map(|path| path.as_ref().display().to_string());
        config.usage_log_path = usage_log_path
            .as_ref()
            .map(|path| path.as_ref().display().to_string());
        config.models = load_json_file(model_config_path.as_ref())?;
        if let Some(path) = proxy_keys_path {
            config.proxy_keys = load_json_file(path.as_ref())?;
        }
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
        config.enable_provider_default_auth_fallback =
            env_flag("ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK");
        config.openai_api_key = env::var("OPENAI_API_KEY").ok();
        config.anthropic_api_key = env::var("ANTHROPIC_API_KEY").ok();
        config.gemini_api_key = env::var("GEMINI_API_KEY").ok();
        config.openai_base_url = env::var("OPENAI_BASE_URL").unwrap_or(config.openai_base_url);
        config.anthropic_base_url =
            env::var("ANTHROPIC_BASE_URL").unwrap_or(config.anthropic_base_url);
        config.gemini_base_url = env::var("GEMINI_BASE_URL").unwrap_or(config.gemini_base_url);
        config.model_config_path = env::var("MODEL_CONFIG_PATH").ok();
        config.proxy_api_keys_path = env::var("PROXY_API_KEYS_PATH").ok();
        config.usage_log_path = env::var("USAGE_LOG_PATH").ok();
        config.access_log_path = env::var("ACCESS_LOG_PATH").ok();

        if config.enable_provider_default_auth_fallback {
            if config.openai_api_key.is_none() {
                config.openai_api_key = load_openai_default_api_key()?;
            }
        }

        if let Some(path) = &config.model_config_path {
            config.models = load_json_file(Path::new(path))?;
        }

        if let Some(path) = &config.proxy_api_keys_path {
            config.proxy_keys = load_json_file(Path::new(path))?;
        }

        Ok(config)
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}

fn load_openai_default_api_key() -> Result<Option<String>, AppError> {
    let Some(home) = env::var_os("HOME") else {
        return Ok(None);
    };
    let path = PathBuf::from(home).join(".codex/auth.json");
    if !path.exists() {
        return Ok(None);
    }

    let auth: CodexAuthFile = load_json_file(&path)?;
    Ok(auth.tokens.access_token)
}

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    tokens: CodexAuthTokens,
}

#[derive(Debug, Deserialize)]
struct CodexAuthTokens {
    access_token: Option<String>,
}

fn load_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, AppError> {
    let body = fs::read_to_string(path).map_err(|error| {
        AppError::validation(format!("failed to read {}: {error}", path.display()))
    })?;
    serde_json::from_str(&body).map_err(|error| {
        AppError::validation(format!("failed to parse {}: {error}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn parses_model_registry_entries() {
        let config =
            AppConfig::from_parts("127.0.0.1:3000", 30, vec!["gpt-4.1=openai:gpt-4.1".into()])
                .unwrap();

        assert_eq!(config.models.len(), 1);
        assert_eq!(config.models[0].public_name, "gpt-4.1");
        assert_eq!(config.models[0].targets[0].provider, ProviderKind::OpenAi);
    }

    #[test]
    fn loads_model_catalog_from_json_file() {
        let config =
            AppConfig::from_test_paths("examples/model-config.json", None::<&str>, None::<&str>)
                .unwrap();

        assert_eq!(config.models.len(), 3);
        assert_eq!(config.models[0].targets.len(), 2);
    }

    #[test]
    fn loads_proxy_keys_from_json_file() {
        let config = AppConfig::from_test_paths(
            "examples/model-config.json",
            Some("examples/proxy-keys.json"),
            None::<&str>,
        )
        .unwrap();

        assert_eq!(config.proxy_keys.len(), 1);
        assert_eq!(config.proxy_keys[0].id, "team-alpha");
    }

    #[test]
    fn loads_model_pricing_from_json_file() {
        let config =
            AppConfig::from_test_paths("examples/model-config.json", None::<&str>, None::<&str>)
                .unwrap();

        let pricing = config.models[1].pricing.as_ref().unwrap();
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn loads_access_log_path_from_env() {
        let _guard = env_lock().lock().unwrap();
        let previous = env::var("ACCESS_LOG_PATH").ok();
        unsafe {
            env::set_var("ACCESS_LOG_PATH", "/tmp/access.jsonl");
        }

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.access_log_path.as_deref(), Some("/tmp/access.jsonl"));

        unsafe {
            match previous {
                Some(value) => env::set_var("ACCESS_LOG_PATH", value),
                None => env::remove_var("ACCESS_LOG_PATH"),
            }
        }
    }

    #[test]
    fn loads_openai_api_key_from_codex_auth_when_fallback_enabled() {
        let _guard = env_lock().lock().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        fs::write(
            home.path().join(".codex/auth.json"),
            r#"{
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": "codex-access-token"
                }
            }"#,
        )
        .unwrap();

        let previous_openai = env::var("OPENAI_API_KEY").ok();
        let previous_home = env::var("HOME").ok();
        let previous_fallback = env::var("ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK").ok();

        unsafe {
            env::remove_var("OPENAI_API_KEY");
            env::set_var("HOME", home.path());
            env::set_var("ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK", "true");
        }

        let config = AppConfig::from_env().unwrap();
        assert_eq!(
            config.openai_api_key.as_deref(),
            Some("codex-access-token")
        );

        restore_env("OPENAI_API_KEY", previous_openai);
        restore_env("HOME", previous_home);
        restore_env(
            "ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK",
            previous_fallback,
        );
    }

    #[test]
    fn prefers_explicit_openai_api_key_over_codex_auth_fallback() {
        let _guard = env_lock().lock().unwrap();
        let home = tempdir().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        fs::write(
            home.path().join(".codex/auth.json"),
            r#"{
                "tokens": {
                    "access_token": "codex-access-token"
                }
            }"#,
        )
        .unwrap();

        let previous_openai = env::var("OPENAI_API_KEY").ok();
        let previous_home = env::var("HOME").ok();
        let previous_fallback = env::var("ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK").ok();

        unsafe {
            env::set_var("OPENAI_API_KEY", "env-openai-key");
            env::set_var("HOME", home.path());
            env::set_var("ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK", "true");
        }

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.openai_api_key.as_deref(), Some("env-openai-key"));

        restore_env("OPENAI_API_KEY", previous_openai);
        restore_env("HOME", previous_home);
        restore_env(
            "ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK",
            previous_fallback,
        );
    }

    fn restore_env(name: &str, previous: Option<String>) {
        unsafe {
            match previous {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}
