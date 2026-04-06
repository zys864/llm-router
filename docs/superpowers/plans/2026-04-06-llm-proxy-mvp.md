# LLM Proxy MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust service that exposes OpenAI-compatible `models`, `chat completions`, and `responses` APIs backed by OpenAI, Anthropic, and Gemini adapters.

**Architecture:** Use `axum` for the HTTP server, typed API/domain structs for the compatibility layer, a config-backed model registry for deterministic routing, and one provider trait with three concrete adapters. Keep OpenAI-compatible HTTP concerns in `src/api/*` and provider protocol translation in `src/providers/*`.

**Tech Stack:** Rust, `axum`, `tokio`, `reqwest`, `serde`, `serde_json`, `tower`, `tower-http`, `tracing`, `async-stream`, `futures`, `async-trait`, `bytes`, `wiremock`

---

## File Structure

- Create: `src/config.rs`
- Create: `src/app_state.rs`
- Create: `src/error.rs`
- Create: `src/models.rs`
- Create: `src/router.rs`
- Create: `src/sse.rs`
- Create: `src/domain/mod.rs`
- Create: `src/domain/request.rs`
- Create: `src/domain/response.rs`
- Create: `src/api/mod.rs`
- Create: `src/api/types.rs`
- Create: `src/api/models.rs`
- Create: `src/api/chat_completions.rs`
- Create: `src/api/responses.rs`
- Create: `src/providers/mod.rs`
- Create: `src/providers/openai.rs`
- Create: `src/providers/anthropic.rs`
- Create: `src/providers/gemini.rs`
- Create: `tests/common/mod.rs`
- Create: `tests/models_api.rs`
- Create: `tests/chat_api.rs`
- Create: `tests/responses_api.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

### Task 1: Scaffold dependencies and server entry point

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the failing boot test in `src/main.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn app_builds_without_panicking() {
        let _ = build_app(test_config());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app_builds_without_panicking -q`
Expected: FAIL with missing `tokio`, missing `build_app`, and missing `test_config`

- [ ] **Step 3: Add the minimal crate dependencies in `Cargo.toml`**

```toml
[dependencies]
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }
futures = "0.3"
async-stream = "0.3"
async-trait = "0.1"
bytes = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 4: Add a minimal app builder in `src/main.rs`**

```rust
use axum::{routing::get, Router};

fn build_app(_config: AppConfig) -> Router {
    Router::new().route("/healthz", get(|| async { "ok" }))
}

fn test_config() -> AppConfig {
    AppConfig::default()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test app_builds_without_panicking -q`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/main.rs
git commit -m "chore: scaffold proxy server dependencies"
```

### Task 2: Add typed configuration and shared app state

**Files:**
- Create: `src/config.rs`
- Create: `src/app_state.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the failing config parse test in `src/config.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_registry_entries() {
        let config = AppConfig::from_parts(
            "127.0.0.1:3000",
            30,
            vec!["gpt-4.1=openai:gpt-4.1".into()],
        )
        .unwrap();

        assert_eq!(config.models.len(), 1);
        assert_eq!(config.models[0].public_name, "gpt-4.1");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parses_model_registry_entries -q`
Expected: FAIL with missing `AppConfig::from_parts`

- [ ] **Step 3: Implement config parsing in `src/config.rs` and shared state in `src/app_state.rs`**

```rust
#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    pub bind_addr: String,
    pub request_timeout_secs: u64,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub models: Vec<ModelConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelConfig {
    pub public_name: String,
    pub provider: ProviderKind,
    pub upstream_name: String,
}

impl AppConfig {
    pub fn from_parts(
        bind_addr: &str,
        request_timeout_secs: u64,
        raw_models: Vec<String>,
    ) -> Result<Self, ConfigError> {
        let models = raw_models
            .into_iter()
            .map(ModelConfig::parse)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            bind_addr: bind_addr.to_string(),
            request_timeout_secs,
            openai_api_key: None,
            anthropic_api_key: None,
            gemini_api_key: None,
            models,
        })
    }
}
```

```rust
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub registry: ModelRegistry,
    pub provider_factory: ProviderFactory,
}
```

- [ ] **Step 4: Wire the config and state types into `src/main.rs`**

```rust
mod app_state;
mod config;

use app_state::AppState;
use config::AppConfig;

fn build_app(config: AppConfig) -> Router {
    let state = AppState::from_config(config);
    api::router(state)
}
```

- [ ] **Step 5: Run targeted tests**

Run: `cargo test parses_model_registry_entries app_builds_without_panicking -q`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/app_state.rs src/main.rs
git commit -m "feat: add typed config and shared app state"
```

### Task 3: Add model registry and deterministic routing

**Files:**
- Create: `src/models.rs`
- Create: `src/router.rs`
- Modify: `src/config.rs`
- Test: `src/router.rs`

- [ ] **Step 1: Write the failing routing tests in `src/router.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_model() {
        let registry = ModelRegistry::new(vec![ModelRecord::new(
            "gpt-4.1",
            ProviderKind::OpenAi,
            "gpt-4.1",
        )]);

        let target = resolve_model(&registry, "gpt-4.1").unwrap();
        assert_eq!(target.provider, ProviderKind::OpenAi);
    }

    #[test]
    fn rejects_unknown_model() {
        let registry = ModelRegistry::new(vec![]);
        assert!(resolve_model(&registry, "missing").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test resolves_known_model rejects_unknown_model -q`
Expected: FAIL with missing `ModelRegistry`, `ModelRecord`, and `resolve_model`

- [ ] **Step 3: Implement model records and router lookup**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRecord {
    pub public_name: String,
    pub provider: ProviderKind,
    pub upstream_name: String,
}

#[derive(Clone, Debug)]
pub struct ModelRegistry {
    records: Vec<ModelRecord>,
}

impl ModelRegistry {
    pub fn new(records: Vec<ModelRecord>) -> Self {
        Self { records }
    }

    pub fn get(&self, public_name: &str) -> Option<&ModelRecord> {
        self.records.iter().find(|record| record.public_name == public_name)
    }
}
```

```rust
pub fn resolve_model(
    registry: &ModelRegistry,
    public_name: &str,
) -> Result<ModelRoute, AppError> {
    let record = registry
        .get(public_name)
        .ok_or_else(|| AppError::model_not_found(public_name))?;

    Ok(ModelRoute {
        provider: record.provider,
        public_name: record.public_name.clone(),
        upstream_name: record.upstream_name.clone(),
    })
}
```

- [ ] **Step 4: Run routing tests**

Run: `cargo test resolves_known_model rejects_unknown_model -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/models.rs src/router.rs src/config.rs
git commit -m "feat: add model registry and routing"
```

### Task 4: Add unified errors and OpenAI-style error envelopes

**Files:**
- Create: `src/error.rs`
- Modify: `src/main.rs`
- Test: `src/error.rs`

- [ ] **Step 1: Write the failing error serialization test in `src/error.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_model_not_found_error() {
        let body = AppError::model_not_found("missing").into_response_body("req_123");
        assert_eq!(body.error.message, "model `missing` not found");
        assert_eq!(body.error.r#type, "invalid_request_error");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test serializes_model_not_found_error -q`
Expected: FAIL with missing `AppError`

- [ ] **Step 3: Implement the error type and envelope**

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("model `{0}` not found")]
    ModelNotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("timeout")]
    Timeout,
}

#[derive(Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
    pub request_id: String,
}
```

- [ ] **Step 4: Run the targeted test**

Run: `cargo test serializes_model_not_found_error -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/main.rs
git commit -m "feat: add unified api error handling"
```

### Task 5: Define OpenAI-compatible API types and internal domain models

**Files:**
- Create: `src/api/types.rs`
- Create: `src/domain/mod.rs`
- Create: `src/domain/request.rs`
- Create: `src/domain/response.rs`
- Test: `src/domain/request.rs`

- [ ] **Step 1: Write the failing conversion test in `src/domain/request.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{ChatCompletionRequest, ChatMessage, MessageContent};

    #[test]
    fn converts_chat_request_into_domain_request() {
        let request = ChatCompletionRequest {
            model: "gpt-4.1".into(),
            messages: vec![ChatMessage::user("hello")],
            temperature: Some(0.2),
            max_tokens: Some(64),
            stream: Some(false),
        };

        let domain = UnifiedRequest::from_chat(request, sample_route(), "req_123").unwrap();
        assert_eq!(domain.route.public_name, "gpt-4.1");
        assert_eq!(domain.messages.len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test converts_chat_request_into_domain_request -q`
Expected: FAIL with missing API and domain types

- [ ] **Step 3: Implement the typed API payloads and domain models**

```rust
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: String,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stream: Option<bool>,
}
```

```rust
#[derive(Clone, Debug)]
pub struct UnifiedRequest {
    pub request_id: String,
    pub route: ModelRoute,
    pub messages: Vec<UnifiedMessage>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stream: bool,
}
```

- [ ] **Step 4: Run the targeted test**

Run: `cargo test converts_chat_request_into_domain_request -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/types.rs src/domain/mod.rs src/domain/request.rs src/domain/response.rs
git commit -m "feat: add api payloads and unified domain models"
```

### Task 6: Add provider trait and OpenAI adapter

**Files:**
- Create: `src/providers/mod.rs`
- Create: `src/providers/openai.rs`
- Modify: `src/error.rs`
- Test: `src/providers/openai.rs`

- [ ] **Step 1: Write the failing OpenAI adapter test in `src/providers/openai.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maps_unified_request_to_openai_payload() {
        let adapter = OpenAiProvider::new(reqwest::Client::new(), "secret".into(), test_base_url());
        let payload = adapter.build_request_body(sample_unified_request());

        assert_eq!(payload["model"], "gpt-4.1");
        assert_eq!(payload["messages"][0]["content"], "hello");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test maps_unified_request_to_openai_payload -q`
Expected: FAIL with missing provider trait and OpenAI adapter

- [ ] **Step 3: Implement the provider trait and OpenAI adapter**

```rust
#[async_trait::async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn complete(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError>;
    async fn stream(
        &self,
        request: UnifiedRequest,
    ) -> Result<ProviderEventStream, AppError>;
}
```

```rust
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn build_request_body(&self, request: UnifiedRequest) -> serde_json::Value {
        serde_json::json!({
            "model": request.route.upstream_name,
            "messages": request.messages.iter().map(to_openai_message).collect::<Vec<_>>(),
            "temperature": request.temperature,
            "max_tokens": request.max_output_tokens,
            "stream": request.stream,
        })
    }
}
```

- [ ] **Step 4: Run the targeted test**

Run: `cargo test maps_unified_request_to_openai_payload -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/providers/mod.rs src/providers/openai.rs src/error.rs Cargo.toml
git commit -m "feat: add provider trait and openai adapter"
```

### Task 7: Add Anthropic and Gemini adapters

**Files:**
- Create: `src/providers/anthropic.rs`
- Create: `src/providers/gemini.rs`
- Modify: `src/providers/mod.rs`
- Test: `src/providers/anthropic.rs`
- Test: `src/providers/gemini.rs`

- [ ] **Step 1: Write the failing adapter mapping tests**

```rust
#[tokio::test]
async fn maps_unified_request_to_anthropic_payload() {
    let adapter = AnthropicProvider::new(reqwest::Client::new(), "secret".into(), test_base_url());
    let payload = adapter.build_request_body(sample_unified_request());
    assert_eq!(payload["model"], "claude-sonnet-4-20250514");
}

#[tokio::test]
async fn maps_unified_request_to_gemini_payload() {
    let adapter = GeminiProvider::new(reqwest::Client::new(), "secret".into(), test_base_url());
    let payload = adapter.build_request_body(sample_unified_request());
    assert_eq!(payload["model"], "gemini-2.5-pro");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test maps_unified_request_to_anthropic_payload maps_unified_request_to_gemini_payload -q`
Expected: FAIL with missing adapter implementations

- [ ] **Step 3: Implement the Anthropic and Gemini adapters**

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}
```

```rust
fn anthropic_messages(messages: &[UnifiedMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|message| message.role != UnifiedRole::System)
        .map(to_anthropic_message)
        .collect()
}

fn gemini_contents(messages: &[UnifiedMessage]) -> Vec<serde_json::Value> {
    messages.iter().map(to_gemini_content).collect()
}
```

- [ ] **Step 4: Run the targeted tests**

Run: `cargo test maps_unified_request_to_anthropic_payload maps_unified_request_to_gemini_payload -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/providers/anthropic.rs src/providers/gemini.rs src/providers/mod.rs
git commit -m "feat: add anthropic and gemini adapters"
```

### Task 8: Add SSE encoder for OpenAI-style streaming

**Files:**
- Create: `src/sse.rs`
- Modify: `src/domain/response.rs`
- Test: `src/sse.rs`

- [ ] **Step 1: Write the failing SSE encoding test in `src/sse.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_delta_and_done_events() {
        let chunks = encode_events(
            "gpt-4.1",
            vec![
                StreamEvent::DeltaText("hel".into()),
                StreamEvent::DeltaText("lo".into()),
                StreamEvent::Completed,
            ],
        );

        assert!(chunks[0].contains("\"delta\""));
        assert_eq!(chunks.last().unwrap(), "data: [DONE]\n\n");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test encodes_delta_and_done_events -q`
Expected: FAIL with missing `encode_events`

- [ ] **Step 3: Implement stream events and SSE encoding**

```rust
#[derive(Clone, Debug)]
pub enum StreamEvent {
    Started,
    DeltaText(String),
    Usage(UnifiedUsage),
    Completed,
    Error(String),
}
```

```rust
pub fn encode_events(model: &str, events: Vec<StreamEvent>) -> Vec<String> {
    let mut chunks = Vec::new();

    for event in events {
        match event {
            StreamEvent::DeltaText(text) => {
                chunks.push(format!(
                    "data: {{\"object\":\"chat.completion.chunk\",\"model\":\"{model}\",\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n"
                ));
            }
            StreamEvent::Completed => chunks.push("data: [DONE]\n\n".into()),
            _ => {}
        }
    }

    chunks
}
```

- [ ] **Step 4: Run the targeted test**

Run: `cargo test encodes_delta_and_done_events -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/sse.rs src/domain/response.rs
git commit -m "feat: add sse chunk encoding"
```

### Task 9: Add `/v1/models` handler and integration test

**Files:**
- Create: `src/api/mod.rs`
- Create: `src/api/models.rs`
- Modify: `src/main.rs`
- Create: `tests/common/mod.rs`
- Create: `tests/models_api.rs`

- [ ] **Step 1: Write the failing integration test in `tests/models_api.rs`**

```rust
#[tokio::test]
async fn lists_configured_models() {
    let app = test_app_with_models(vec![
        ("gpt-4.1", "openai", "gpt-4.1"),
        ("claude-sonnet-4", "anthropic", "claude-sonnet-4-20250514"),
    ]);

    let response = app.get("/v1/models").await;
    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("claude-sonnet-4"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test lists_configured_models -q`
Expected: FAIL with missing API router and test helper

- [ ] **Step 3: Implement the route wiring and handler**

```rust
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/models", get(models::list_models))
        .with_state(state)
}
```

```rust
pub async fn list_models(State(state): State<AppState>) -> Result<Json<ModelsResponse>, AppError> {
    Ok(Json(ModelsResponse::from_registry(&state.registry)))
}
```

- [ ] **Step 4: Run the targeted integration test**

Run: `cargo test lists_configured_models -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/mod.rs src/api/models.rs src/main.rs tests/common/mod.rs tests/models_api.rs
git commit -m "feat: add models endpoint"
```

### Task 10: Add non-streaming `/v1/chat/completions`

**Files:**
- Create: `src/api/chat_completions.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/providers/mod.rs`
- Create: `tests/chat_api.rs`

- [ ] **Step 1: Write the failing non-streaming chat integration test**

```rust
#[tokio::test]
async fn proxies_chat_completion_to_openai_adapter() {
    let app = test_app_with_openai_mock("hello from upstream");

    let response = app
        .post_json(
            "/v1/chat/completions",
            serde_json::json!({
                "model": "gpt-4.1",
                "messages": [{ "role": "user", "content": "hello" }]
            }),
        )
        .await;

    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("hello from upstream"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test proxies_chat_completion_to_openai_adapter -q`
Expected: FAIL with missing chat handler

- [ ] **Step 3: Implement the non-streaming chat handler**

```rust
pub async fn create_chat_completion(
    State(state): State<AppState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    let route = resolve_model(&state.registry, &payload.model)?;
    let request = UnifiedRequest::from_chat(payload, route, new_request_id())?;
    let provider = state.provider_factory.for_route(&request.route)?;

    if request.stream {
        return stream_chat_completion(provider, request).await;
    }

    let response = provider.complete(request).await?;
    Ok(Json(ChatCompletionResponse::from_domain(response)).into_response())
}
```

- [ ] **Step 4: Run the targeted integration test**

Run: `cargo test proxies_chat_completion_to_openai_adapter -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/chat_completions.rs src/api/mod.rs src/providers/mod.rs tests/chat_api.rs
git commit -m "feat: add chat completions endpoint"
```

### Task 11: Add streaming `/v1/chat/completions`

**Files:**
- Modify: `src/api/chat_completions.rs`
- Modify: `src/providers/openai.rs`
- Modify: `src/providers/anthropic.rs`
- Modify: `src/providers/gemini.rs`
- Modify: `tests/chat_api.rs`

- [ ] **Step 1: Write the failing streaming integration test**

```rust
#[tokio::test]
async fn streams_chat_completion_in_openai_sse_format() {
    let app = test_app_with_streaming_openai_mock(vec!["hel", "lo"]);

    let response = app
        .post_json(
            "/v1/chat/completions",
            serde_json::json!({
                "model": "gpt-4.1",
                "messages": [{ "role": "user", "content": "hello" }],
                "stream": true
            }),
        )
        .await;

    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("data: [DONE]"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test streams_chat_completion_in_openai_sse_format -q`
Expected: FAIL with missing SSE response path

- [ ] **Step 3: Implement the streaming handler and provider stream mapping**

```rust
async fn stream_chat_completion(
    provider: Arc<dyn ProviderAdapter>,
    request: UnifiedRequest,
) -> Result<Response, AppError> {
    let stream = provider.stream(request).await?;
    let body = Body::from_stream(stream.map(|event| Ok(Bytes::from(event.into_chunk()))));

    Ok(Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap())
}
```

- [ ] **Step 4: Run the targeted integration test**

Run: `cargo test streams_chat_completion_in_openai_sse_format -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/chat_completions.rs src/providers/openai.rs src/providers/anthropic.rs src/providers/gemini.rs tests/chat_api.rs
git commit -m "feat: add streaming chat completions"
```

### Task 12: Add non-streaming and streaming `/v1/responses`

**Files:**
- Create: `src/api/responses.rs`
- Modify: `src/api/mod.rs`
- Create: `tests/responses_api.rs`

- [ ] **Step 1: Write the failing responses integration tests**

```rust
#[tokio::test]
async fn proxies_responses_request() {
    let app = test_app_with_openai_mock("hello from responses");

    let response = app
        .post_json(
            "/v1/responses",
            serde_json::json!({
                "model": "gpt-4.1",
                "input": "hello"
            }),
        )
        .await;

    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("hello from responses"));
}

#[tokio::test]
async fn streams_responses_request() {
    let app = test_app_with_streaming_openai_mock(vec!["hel", "lo"]);

    let response = app
        .post_json(
            "/v1/responses",
            serde_json::json!({
                "model": "gpt-4.1",
                "input": "hello",
                "stream": true
            }),
        )
        .await;

    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("data: [DONE]"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test proxies_responses_request streams_responses_request -q`
Expected: FAIL with missing responses handler

- [ ] **Step 3: Implement the responses handler by converting into `UnifiedRequest`**

```rust
pub async fn create_response(
    State(state): State<AppState>,
    Json(payload): Json<ResponsesRequest>,
) -> Result<Response, AppError> {
    let route = resolve_model(&state.registry, &payload.model)?;
    let request = UnifiedRequest::from_responses(payload, route, new_request_id())?;
    let provider = state.provider_factory.for_route(&request.route)?;

    if request.stream {
        return stream_responses(provider, request).await;
    }

    let response = provider.complete(request).await?;
    Ok(Json(ResponsesResponse::from_domain(response)).into_response())
}
```

- [ ] **Step 4: Run the targeted integration tests**

Run: `cargo test proxies_responses_request streams_responses_request -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/responses.rs src/api/mod.rs tests/responses_api.rs
git commit -m "feat: add responses endpoint"
```

### Task 13: Add provider mock integration coverage for Anthropic and Gemini

**Files:**
- Modify: `tests/common/mod.rs`
- Modify: `tests/chat_api.rs`
- Modify: `tests/responses_api.rs`

- [ ] **Step 1: Write the failing adapter coverage tests**

```rust
#[tokio::test]
async fn proxies_chat_completion_to_anthropic_adapter() {
    let app = test_app_with_anthropic_mock("anthropic says hi");
    let response = app.post_json("/v1/chat/completions", anthropic_request()).await;
    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("anthropic says hi"));
}

#[tokio::test]
async fn proxies_chat_completion_to_gemini_adapter() {
    let app = test_app_with_gemini_mock("gemini says hi");
    let response = app.post_json("/v1/chat/completions", gemini_request()).await;
    assert_eq!(response.status(), 200);
    assert!(response.text().await.contains("gemini says hi"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test proxies_chat_completion_to_anthropic_adapter proxies_chat_completion_to_gemini_adapter -q`
Expected: FAIL until test helpers and adapters are fully wired

- [ ] **Step 3: Extend the test harness and provider factory wiring**

```rust
pub fn test_app_with_anthropic_mock(body_text: &str) -> TestApp {
    build_test_app(TestHarness::anthropic(body_text))
}

pub fn test_app_with_gemini_mock(body_text: &str) -> TestApp {
    build_test_app(TestHarness::gemini(body_text))
}
```

- [ ] **Step 4: Run the targeted tests**

Run: `cargo test proxies_chat_completion_to_anthropic_adapter proxies_chat_completion_to_gemini_adapter -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tests/common/mod.rs tests/chat_api.rs tests/responses_api.rs src/providers/mod.rs
git commit -m "test: add anthropic and gemini integration coverage"
```

### Task 14: Add error-path integration tests and timeout handling

**Files:**
- Modify: `src/error.rs`
- Modify: `src/providers/mod.rs`
- Modify: `tests/chat_api.rs`
- Modify: `tests/responses_api.rs`

- [ ] **Step 1: Write the failing error-path tests**

```rust
#[tokio::test]
async fn returns_not_found_for_unknown_model() {
    let app = test_app_with_models(vec![("gpt-4.1", "openai", "gpt-4.1")]);
    let response = app
        .post_json(
            "/v1/chat/completions",
            serde_json::json!({
                "model": "missing",
                "messages": [{ "role": "user", "content": "hello" }]
            }),
        )
        .await;

    assert_eq!(response.status(), 400);
    assert!(response.text().await.contains("model `missing` not found"));
}

#[tokio::test]
async fn returns_timeout_error_for_slow_upstream() {
    let app = test_app_with_slow_openai_mock();
    let response = app
        .post_json(
            "/v1/chat/completions",
            serde_json::json!({
                "model": "gpt-4.1",
                "messages": [{ "role": "user", "content": "hello" }]
            }),
        )
        .await;

    assert_eq!(response.status(), 504);
    assert!(response.text().await.contains("\"request_id\""));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test returns_not_found_for_unknown_model returns_timeout_error_for_slow_upstream -q`
Expected: FAIL until timeout mapping and error bodies are returned end-to-end

- [ ] **Step 3: Implement timeout mapping and stable API error responses**

```rust
impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::ModelNotFound(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
        }
    }
}
```

- [ ] **Step 4: Run the targeted tests**

Run: `cargo test returns_not_found_for_unknown_model returns_timeout_error_for_slow_upstream -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/providers/mod.rs tests/chat_api.rs tests/responses_api.rs
git commit -m "feat: add timeout and error-path coverage"
```

### Task 15: Final verification and README-level run notes

**Files:**
- Modify: `src/main.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`
Expected: command completes with no errors

- [ ] **Step 2: Run the full test suite**

Run: `cargo test`
Expected: PASS for unit and integration tests

- [ ] **Step 3: Run a manual smoke check**

Run: `cargo run`
Expected: server starts and binds to configured address without panic

- [ ] **Step 4: Verify the public endpoints manually**

Run: `curl http://127.0.0.1:3000/v1/models`
Expected: JSON object with `data` model list

Run: `curl -X POST http://127.0.0.1:3000/v1/chat/completions -H 'content-type: application/json' -d '{"model":"gpt-4.1","messages":[{"role":"user","content":"hello"}]}'`
Expected: OpenAI-style JSON response or a clear upstream credential error if no provider key is configured

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src tests
git commit -m "feat: complete llm proxy mvp"
```
