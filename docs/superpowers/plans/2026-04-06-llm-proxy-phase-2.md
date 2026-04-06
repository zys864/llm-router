# LLM Proxy Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real provider streaming, model metadata with fallback routing, proxy API key auth with quotas, and append-only usage logging to the LLM proxy.

**Architecture:** Extend the existing OpenAI-compatible handlers while keeping provider-specific logic in adapters and streaming parsers. Replace single-target routing with route plans, enforce auth and quota before handler execution, and log one terminal usage record per request.

**Tech Stack:** Rust, `axum`, `tokio`, `reqwest`, `serde`, `serde_json`, `tower`, `futures`, `tokio-stream`, `tokio-util`, `wiremock`

---

## File Structure

- Create: `src/auth.rs`
- Create: `src/quota.rs`
- Create: `src/usage.rs`
- Create: `src/providers/streaming.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`
- Modify: `src/config.rs`
- Modify: `src/app_state.rs`
- Modify: `src/error.rs`
- Modify: `src/models.rs`
- Modify: `src/router.rs`
- Modify: `src/domain/request.rs`
- Modify: `src/domain/response.rs`
- Modify: `src/sse.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/models.rs`
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `src/api/types.rs`
- Modify: `src/providers/mod.rs`
- Modify: `src/providers/openai.rs`
- Modify: `src/providers/anthropic.rs`
- Modify: `src/providers/gemini.rs`
- Modify: `README.md`
- Modify: `.env.example`
- Create: `examples/model-config.json`
- Create: `examples/proxy-keys.json`
- Modify: `tests/common/mod.rs`
- Modify: `tests/chat_api.rs`
- Modify: `tests/models_api.rs`
- Modify: `tests/responses_api.rs`
- Create: `tests/auth_api.rs`
- Create: `tests/logging_api.rs`

### Task 1: Add config support for model catalogs, proxy keys, and usage logging

**Files:**
- Modify: `src/config.rs`
- Create: `examples/model-config.json`
- Create: `examples/proxy-keys.json`
- Test: `src/config.rs`

- [ ] **Step 1: Write the failing config tests**

```rust
#[test]
fn loads_model_catalog_from_json_file() {
    let config = AppConfig::from_test_paths(
        "examples/model-config.json",
        None,
        None,
    )
    .unwrap();

    assert_eq!(config.models.len(), 3);
    assert_eq!(config.models[0].targets.len(), 2);
}

#[test]
fn loads_proxy_keys_from_json_file() {
    let config = AppConfig::from_test_paths(
        "examples/model-config.json",
        Some("examples/proxy-keys.json"),
        None,
    )
    .unwrap();

    assert_eq!(config.proxy_keys.len(), 1);
    assert_eq!(config.proxy_keys[0].id, "team-alpha");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test loads_model_catalog_from_json_file loads_proxy_keys_from_json_file -q`
Expected: FAIL with missing richer config types and loaders

- [ ] **Step 3: Implement config structs and file loading**

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct ModelCatalogEntry {
    pub public_name: String,
    pub capabilities: ModelCapabilities,
    pub targets: Vec<ModelTargetConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProxyKeyConfig {
    pub id: String,
    pub api_key: String,
    pub max_requests: u64,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub model_config_path: Option<String>,
    pub proxy_api_keys_path: Option<String>,
    pub usage_log_path: Option<String>,
    pub proxy_keys: Vec<ProxyKeyConfig>,
    pub models: Vec<ModelCatalogEntry>,
    // keep existing provider base URLs and keys
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test loads_model_catalog_from_json_file loads_proxy_keys_from_json_file -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs examples/model-config.json examples/proxy-keys.json .env.example
git commit -m "feat: add phase 2 config loading"
```

### Task 2: Add auth and quota core modules

**Files:**
- Create: `src/auth.rs`
- Create: `src/quota.rs`
- Modify: `src/app_state.rs`
- Test: `src/auth.rs`
- Test: `src/quota.rs`

- [ ] **Step 1: Write the failing auth and quota tests**

```rust
#[test]
fn resolves_valid_proxy_key() {
    let auth = AuthService::new(vec![ProxyKeyConfig {
        id: "team-alpha".into(),
        api_key: "lr_live_alpha".into(),
        max_requests: 10,
    }]);

    let caller = auth.authenticate("lr_live_alpha").unwrap().unwrap();
    assert_eq!(caller.id, "team-alpha");
}

#[test]
fn rejects_requests_after_quota_is_exhausted() {
    let quota = QuotaStore::new(vec![CallerQuota::new("team-alpha", 1)]);
    assert!(quota.try_acquire("team-alpha").is_ok());
    assert!(quota.try_acquire("team-alpha").is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test resolves_valid_proxy_key rejects_requests_after_quota_is_exhausted -q`
Expected: FAIL with missing auth and quota modules

- [ ] **Step 3: Implement auth and quota modules**

```rust
#[derive(Clone, Debug)]
pub struct AuthenticatedCaller {
    pub id: String,
}

pub struct AuthService {
    callers_by_key: HashMap<String, ProxyKeyConfig>,
}

pub struct QuotaStore {
    counters: Arc<Mutex<HashMap<String, u64>>>,
    limits: HashMap<String, u64>,
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test resolves_valid_proxy_key rejects_requests_after_quota_is_exhausted -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/auth.rs src/quota.rs src/app_state.rs
git commit -m "feat: add auth and quota core modules"
```

### Task 3: Add richer model registry and fallback route plans

**Files:**
- Modify: `src/models.rs`
- Modify: `src/router.rs`
- Modify: `src/domain/request.rs`
- Test: `src/router.rs`

- [ ] **Step 1: Write the failing routing tests**

```rust
#[test]
fn builds_route_plan_in_priority_order() {
    let registry = sample_registry_with_fallback();
    let plan = resolve_route_plan(&registry, "claude-sonnet-4", Capability::ChatCompletions).unwrap();

    assert_eq!(plan.targets.len(), 2);
    assert_eq!(plan.targets[0].provider, ProviderKind::Anthropic);
}

#[test]
fn filters_out_targets_without_streaming_capability() {
    let registry = sample_registry_with_fallback();
    let plan = resolve_route_plan(&registry, "claude-sonnet-4", Capability::Streaming).unwrap();

    assert!(plan.targets.iter().all(|target| target.capabilities.streaming));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test builds_route_plan_in_priority_order filters_out_targets_without_streaming_capability -q`
Expected: FAIL with missing route-plan types

- [ ] **Step 3: Implement model metadata and route plans**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelCapabilities {
    pub chat_completions: bool,
    pub responses: bool,
    pub streaming: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelTarget {
    pub provider: ProviderKind,
    pub upstream_name: String,
    pub priority: u32,
    pub capabilities: ModelCapabilities,
}

pub struct RoutePlan {
    pub public_name: String,
    pub targets: Vec<ModelTarget>,
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test builds_route_plan_in_priority_order filters_out_targets_without_streaming_capability -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/models.rs src/router.rs src/domain/request.rs
git commit -m "feat: add route plans and fallback metadata"
```

### Task 4: Add usage record model and append-only logger

**Files:**
- Create: `src/usage.rs`
- Modify: `src/app_state.rs`
- Test: `src/usage.rs`

- [ ] **Step 1: Write the failing usage logger test**

```rust
#[tokio::test]
async fn appends_usage_record_as_jsonl() {
    let path = tempfile::NamedTempFile::new().unwrap();
    let logger = UsageLogger::new(Some(path.path().to_path_buf())).unwrap();

    logger.append(UsageRecord::success("req_123", "gpt-4.1")).await.unwrap();

    let body = std::fs::read_to_string(path.path()).unwrap();
    assert!(body.contains("\"request_id\":\"req_123\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test appends_usage_record_as_jsonl -q`
Expected: FAIL with missing usage logger

- [ ] **Step 3: Implement the usage logger**

```rust
#[derive(Clone, Debug, Serialize)]
pub struct UsageRecord {
    pub request_id: String,
    pub caller_id: Option<String>,
    pub model: String,
    pub provider: Option<String>,
    pub attempts: usize,
    pub status: String,
}

#[derive(Clone)]
pub struct UsageLogger {
    file: Option<Arc<tokio::sync::Mutex<tokio::fs::File>>>,
}
```

- [ ] **Step 4: Run the targeted test**

Run: `cargo test appends_usage_record_as_jsonl -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/usage.rs src/app_state.rs Cargo.toml Cargo.lock
git commit -m "feat: add usage logging"
```

### Task 5: Add auth middleware and request context propagation

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/error.rs`
- Test: `tests/auth_api.rs`

- [ ] **Step 1: Write the failing auth integration tests**

```rust
#[tokio::test]
async fn rejects_missing_proxy_key_when_auth_is_enabled() {
    let app = auth_enabled_app().await;
    let response = get(app, "/v1/models").await;

    assert_eq!(response.status, 401);
}

#[tokio::test]
async fn accepts_valid_proxy_key_when_auth_is_enabled() {
    let app = auth_enabled_app().await;
    let response = get_with_bearer(app, "/v1/models", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test rejects_missing_proxy_key_when_auth_is_enabled accepts_valid_proxy_key_when_auth_is_enabled -q`
Expected: FAIL with missing auth enforcement

- [ ] **Step 3: Implement auth middleware**

```rust
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    if request.uri().path() == "/healthz" {
        return Ok(next.run(request).await);
    }

    let caller = state.auth_service.authenticate_header(request.headers())?;
    request.extensions_mut().insert(caller);
    Ok(next.run(request).await)
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test rejects_missing_proxy_key_when_auth_is_enabled accepts_valid_proxy_key_when_auth_is_enabled -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/api/mod.rs src/error.rs tests/auth_api.rs tests/common/mod.rs
git commit -m "feat: add proxy auth middleware"
```

### Task 6: Add quota enforcement at request entry

**Files:**
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `tests/auth_api.rs`

- [ ] **Step 1: Write the failing quota integration test**

```rust
#[tokio::test]
async fn rejects_request_after_quota_is_exhausted() {
    let app = quota_limited_app().await;

    let first = post_json_with_bearer(app.clone(), "/v1/chat/completions", "lr_live_alpha", sample_chat_body()).await;
    assert_eq!(first.status, 200);

    let second = post_json_with_bearer(app, "/v1/chat/completions", "lr_live_alpha", sample_chat_body()).await;
    assert_eq!(second.status, 429);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test rejects_request_after_quota_is_exhausted -q`
Expected: FAIL with missing quota enforcement

- [ ] **Step 3: Implement quota checks before provider execution**

```rust
let caller = request.extensions().get::<AuthenticatedCaller>().cloned();
state.quota_store.try_acquire_optional(caller.as_ref())?;
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test rejects_request_after_quota_is_exhausted -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/chat_completions.rs src/api/responses.rs tests/auth_api.rs
git commit -m "feat: enforce proxy request quotas"
```

### Task 7: Replace synthetic streaming with live provider streaming primitives

**Files:**
- Create: `src/providers/streaming.rs`
- Modify: `src/domain/response.rs`
- Modify: `src/sse.rs`
- Modify: `src/providers/mod.rs`
- Test: `src/sse.rs`

- [ ] **Step 1: Write the failing live-stream encoding test**

```rust
#[tokio::test]
async fn encodes_live_stream_events_into_sse_chunks() {
    let stream = tokio_stream::iter(vec![
        Ok(StreamEvent::TextDelta("hel".into())),
        Ok(StreamEvent::TextDelta("lo".into())),
        Ok(StreamEvent::Completed),
    ]);

    let chunks = collect_sse_chunks("gpt-4.1", stream).await;
    assert!(chunks[0].contains("\"delta\""));
    assert_eq!(chunks.last().unwrap(), "data: [DONE]\n\n");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test encodes_live_stream_events_into_sse_chunks -q`
Expected: FAIL with missing async stream encoder

- [ ] **Step 3: Implement shared async stream types**

```rust
pub type EventResult = Result<StreamEvent, AppError>;
pub type EventStream = Pin<Box<dyn Stream<Item = EventResult> + Send>>;
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test encodes_live_stream_events_into_sse_chunks -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/providers/streaming.rs src/domain/response.rs src/sse.rs src/providers/mod.rs
git commit -m "feat: add live streaming primitives"
```

### Task 8: Add real OpenAI, Anthropic, and Gemini streaming adapters

**Files:**
- Modify: `src/providers/openai.rs`
- Modify: `src/providers/anthropic.rs`
- Modify: `src/providers/gemini.rs`
- Modify: `tests/chat_api.rs`
- Modify: `tests/responses_api.rs`

- [ ] **Step 1: Write the failing provider streaming integration tests**

```rust
#[tokio::test]
async fn proxies_real_openai_stream() {
    let app = openai_streaming_app().await;
    let response = post_json(app, "/v1/chat/completions", streaming_chat_body()).await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("hel"));
}

#[tokio::test]
async fn proxies_real_anthropic_stream() {
    let app = anthropic_streaming_app().await;
    let response = post_json(app, "/v1/chat/completions", anthropic_streaming_body()).await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("hello"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test proxies_real_openai_stream proxies_real_anthropic_stream -q`
Expected: FAIL with synthetic streaming behavior

- [ ] **Step 3: Implement live stream parsing in adapters**

```rust
async fn stream(&self, request: UnifiedRequest) -> Result<EventStream, AppError> {
    let response = self.client.post(...).send().await?;
    Ok(parse_openai_sse(response.bytes_stream()))
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test proxies_real_openai_stream proxies_real_anthropic_stream -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/providers/openai.rs src/providers/anthropic.rs src/providers/gemini.rs tests/chat_api.rs tests/responses_api.rs tests/common/mod.rs
git commit -m "feat: add real provider streaming"
```

### Task 9: Add fallback execution for non-stream requests and pre-stream failures

**Files:**
- Modify: `src/router.rs`
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `src/providers/mod.rs`
- Test: `tests/chat_api.rs`

- [ ] **Step 1: Write the failing fallback integration tests**

```rust
#[tokio::test]
async fn falls_back_to_second_target_after_upstream_failure() {
    let app = fallback_openai_then_anthropic_app().await;
    let response = post_json(app, "/v1/chat/completions", fallback_chat_body()).await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("anthropic says hi"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test falls_back_to_second_target_after_upstream_failure -q`
Expected: FAIL with single-target routing

- [ ] **Step 3: Implement route-attempt loop**

```rust
for target in &request.route_plan.targets {
    match provider.complete(request.with_target(target.clone())).await {
        Ok(response) => return Ok(response),
        Err(error) if error.is_fallback_eligible() => continue,
        Err(error) => return Err(error),
    }
}
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test falls_back_to_second_target_after_upstream_failure -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/router.rs src/api/chat_completions.rs src/api/responses.rs src/providers/mod.rs tests/chat_api.rs
git commit -m "feat: add fallback routing"
```

### Task 10: Add usage logging to request success and failure paths

**Files:**
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `src/usage.rs`
- Create: `tests/logging_api.rs`

- [ ] **Step 1: Write the failing usage logging integration test**

```rust
#[tokio::test]
async fn writes_one_usage_record_for_successful_request() {
    let (app, log_path) = usage_logging_app().await;
    let response = post_json(app, "/v1/chat/completions", sample_chat_body()).await;

    assert_eq!(response.status, 200);
    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"status\":\"success\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test writes_one_usage_record_for_successful_request -q`
Expected: FAIL with missing usage logging integration

- [ ] **Step 3: Implement usage logging hooks**

```rust
state
    .usage_logger
    .append(UsageRecord::from_success(&request, &response, attempts, caller))
    .await?;
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test writes_one_usage_record_for_successful_request -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/chat_completions.rs src/api/responses.rs src/usage.rs tests/logging_api.rs tests/common/mod.rs
git commit -m "feat: add usage logging integration"
```

### Task 11: Expose richer model metadata and update docs

**Files:**
- Modify: `src/api/types.rs`
- Modify: `src/api/models.rs`
- Modify: `tests/models_api.rs`
- Modify: `README.md`
- Modify: `.env.example`

- [ ] **Step 1: Write the failing models metadata test**

```rust
#[tokio::test]
async fn lists_streaming_capability_for_models() {
    let app = models_only_app();
    let response = get(app, "/v1/models").await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"streaming\":true"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test lists_streaming_capability_for_models -q`
Expected: FAIL with current minimal model output

- [ ] **Step 3: Implement richer models response and docs**

```rust
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub owned_by: String,
    pub metadata: ModelMetadataObject,
}
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test lists_streaming_capability_for_models -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/api/types.rs src/api/models.rs tests/models_api.rs README.md .env.example
git commit -m "feat: expose model metadata"
```

### Task 12: Final verification

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`
Expected: command completes with no errors

- [ ] **Step 2: Run the full test suite**

Run: `cargo test`
Expected: PASS for unit and integration tests

- [ ] **Step 3: Run a smoke start**

Run: `cargo run`
Expected: service starts without panic and binds to configured address

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src tests README.md .env.example docs/superpowers/plans/2026-04-06-llm-proxy-phase-2.md
git commit -m "feat: complete phase 2 proxy capabilities"
```
