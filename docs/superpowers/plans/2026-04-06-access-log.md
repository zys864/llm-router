# Access Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured JSONL access logging for request start, upstream attempts, and request finish without storing raw prompts.

**Architecture:** Introduce a new `access_log` module parallel to `usage`, wire it through `AppConfig` and `AppState`, and emit structured events from the existing chat and responses handlers. Keep access-log failures non-fatal by swallowing logger errors behind `tracing::warn!`, and use integration tests with temporary log files to verify success, fallback, failure, and redaction behavior.

**Tech Stack:** Rust, `axum`, `tokio`, `serde`, `serde_json`, `tracing`, `wiremock`, `tempfile`

---

## File Structure

- Create: `src/access_log.rs`
- Modify: `src/lib.rs`
- Modify: `src/config.rs`
- Modify: `src/app_state.rs`
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `src/api/types.rs`
- Modify: `src/error.rs`
- Modify: `README.md`
- Modify: `.env.example`
- Modify: `tests/common/mod.rs`
- Create: `tests/access_logging_api.rs`

### Task 1: Add access-log config and logger core

**Files:**
- Create: `src/access_log.rs`
- Modify: `src/config.rs`
- Modify: `src/app_state.rs`
- Modify: `src/lib.rs`
- Test: `src/access_log.rs`
- Test: `src/config.rs`

- [ ] **Step 1: Write the failing config and logger tests**

```rust
#[test]
fn loads_access_log_path_from_env() {
    temp_env::with_var("ACCESS_LOG_PATH", Some("/tmp/access.jsonl"), || {
        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.access_log_path.as_deref(), Some("/tmp/access.jsonl"));
    });
}

#[tokio::test]
async fn appends_access_log_event_as_jsonl() {
    let path = tempfile::NamedTempFile::new().unwrap();
    let logger = AccessLogger::new(Some(path.path().to_path_buf())).await.unwrap();

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test loads_access_log_path_from_env appends_access_log_event_as_jsonl disabled_access_logger_is_a_no_op -q`
Expected: FAIL with missing `access_log_path`, `AccessLogger`, and `AccessLogEvent`

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
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

#[derive(Clone, Default)]
pub struct AccessLogger {
    file: Option<Arc<Mutex<File>>>,
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test loads_access_log_path_from_env appends_access_log_event_as_jsonl disabled_access_logger_is_a_no_op -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/access_log.rs src/config.rs src/app_state.rs src/lib.rs Cargo.toml Cargo.lock
git commit -m "feat: add access log core"
```

### Task 2: Add request-summary builders without prompt persistence

**Files:**
- Create: `src/access_log.rs`
- Modify: `src/api/types.rs`
- Test: `src/access_log.rs`

- [ ] **Step 1: Write the failing summary tests**

```rust
#[test]
fn chat_summary_counts_roles_and_characters() {
    let payload = ChatCompletionRequest {
        model: "gpt-4.1".into(),
        messages: vec![
            ChatMessage { role: "system".into(), content: "rules".into() },
            ChatMessage { role: "user".into(), content: "hello".into() },
            ChatMessage { role: "assistant".into(), content: "hi".into() },
        ],
        temperature: Some(0.7),
        max_tokens: Some(128),
        stream: Some(false),
        ..Default::default()
    };

    let summary = RequestSummary::from_chat_request(&payload);
    assert_eq!(summary.message_count, 3);
    assert_eq!(summary.system_message_count, 1);
    assert_eq!(summary.user_message_count, 1);
    assert_eq!(summary.assistant_message_count, 1);
    assert_eq!(summary.input_text_chars, 14);
    assert!(summary.has_system_prompt);
    assert!(summary.has_temperature);
    assert!(summary.has_max_tokens);
}

#[test]
fn responses_summary_does_not_store_prompt_text() {
    let payload = ResponsesRequest {
        model: "gpt-4.1".into(),
        input: "secret prompt text".into(),
        stream: Some(false),
        ..Default::default()
    };

    let summary = RequestSummary::from_responses_request(&payload);
    let json = serde_json::to_string(&summary).unwrap();

    assert!(summary.input_text_chars >= 18);
    assert!(!json.contains("secret prompt text"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test chat_summary_counts_roles_and_characters responses_summary_does_not_store_prompt_text -q`
Expected: FAIL with missing request-summary builders or request type defaults

- [ ] **Step 3: Write minimal implementation**

```rust
impl RequestSummary {
    pub fn from_chat_request(payload: &ChatCompletionRequest) -> Self {
        let mut summary = Self {
            message_count: payload.messages.len(),
            has_system_prompt: payload.messages.iter().any(|m| m.role == "system"),
            has_temperature: payload.temperature.is_some(),
            has_top_p: payload.top_p.is_some(),
            has_max_tokens: payload.max_tokens.is_some(),
            ..Self::default()
        };

        for message in &payload.messages {
            let len = message.content.chars().count();
            summary.input_text_chars += len;
            match message.role.as_str() {
                "system" => summary.system_message_count += 1,
                "user" => summary.user_message_count += 1,
                "assistant" => summary.assistant_message_count += 1,
                _ => summary.other_message_count += 1,
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
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test chat_summary_counts_roles_and_characters responses_summary_does_not_store_prompt_text -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/access_log.rs src/api/types.rs Cargo.toml Cargo.lock
git commit -m "feat: add request summary logging helpers"
```

### Task 3: Add handler access-log instrumentation and non-fatal logging

**Files:**
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Modify: `src/error.rs`
- Modify: `src/access_log.rs`
- Test: `tests/access_logging_api.rs`
- Test: `tests/common/mod.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
#[tokio::test]
async fn successful_request_writes_start_attempt_and_finish_events() {
    let (app, log_path) = access_logging_openai_app().await;

    let response = post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gpt-4.1",
            "messages": [{ "role": "user", "content": "hello from secret prompt" }]
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"event\":\"request_started\""));
    assert!(body.contains("\"event\":\"upstream_attempt\""));
    assert!(body.contains("\"event\":\"request_finished\""));
    assert!(!body.contains("hello from secret prompt"));
}

#[tokio::test]
async fn fallback_request_writes_multiple_attempt_events() {
    let (app, log_path) = access_logging_fallback_app().await;

    let response = post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "fallback-model",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.matches("\"event\":\"upstream_attempt\"").count() >= 2);
}

#[tokio::test]
async fn terminal_failure_still_writes_finish_event() {
    let (app, log_path) = access_logging_failure_app().await;

    let response = post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "broken-model",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"event\":\"request_finished\""));
    assert!(body.contains("\"status\":\"error\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test successful_request_writes_start_attempt_and_finish_events fallback_request_writes_multiple_attempt_events terminal_failure_still_writes_finish_event -q`
Expected: FAIL with missing access-log integration helpers and missing handler instrumentation

- [ ] **Step 3: Write minimal implementation**

```rust
let request_started_at = std::time::Instant::now();
state
    .access_logger
    .append_warn(
        AccessLogEvent::request_started(
            request_id.clone(),
            "POST",
            "/v1/chat/completions",
            "chat_completions",
            payload.model.clone(),
            payload.stream.unwrap_or(false),
            caller.as_ref().map(|caller| caller.id.clone()),
            RequestSummary::from_chat_request(&payload),
        ),
        "request_started",
    )
    .await;

let attempt_started_at = std::time::Instant::now();
match provider.complete(request.clone()).await {
    Ok(response) => {
        state
            .access_logger
            .append_warn(
                AccessLogEvent::upstream_attempt_success(
                    request_id.clone(),
                    attempts,
                    route.provider.as_str(),
                    route.upstream_name.clone(),
                    route.public_name.clone(),
                    attempt_started_at.elapsed().as_millis(),
                ),
                "upstream_attempt",
            )
            .await;
        state
            .access_logger
            .append_warn(
                AccessLogEvent::request_finished(
                    request_id.clone(),
                    "chat_completions",
                    route.public_name.clone(),
                    false,
                    200,
                    "success",
                    request_started_at.elapsed().as_millis(),
                    attempts,
                    Some(response.provider.clone()),
                    response.usage.as_ref().map(|u| u.input_tokens),
                    response.usage.as_ref().map(|u| u.output_tokens),
                    caller.as_ref().map(|caller| caller.id.clone()),
                ),
                "request_finished",
            )
            .await;
    }
    Err(error) => {
        state
            .access_logger
            .append_warn(
                AccessLogEvent::upstream_attempt_failure(
                    request_id.clone(),
                    attempts,
                    route.provider.as_str(),
                    route.upstream_name.clone(),
                    route.public_name.clone(),
                    attempt_started_at.elapsed().as_millis(),
                    error.kind(),
                    error.to_string(),
                ),
                "upstream_attempt",
            )
            .await;
        last_error = Some(error);
    }
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test successful_request_writes_start_attempt_and_finish_events fallback_request_writes_multiple_attempt_events terminal_failure_still_writes_finish_event -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/access_log.rs src/api/chat_completions.rs src/api/responses.rs src/error.rs tests/common/mod.rs tests/access_logging_api.rs
git commit -m "feat: add access log request instrumentation"
```

### Task 4: Document access-log configuration and verify full suite

**Files:**
- Modify: `README.md`
- Modify: `.env.example`
- Test: `tests/access_logging_api.rs`

- [ ] **Step 1: Write the failing documentation-aware test**

```rust
#[tokio::test]
async fn access_log_records_request_summary_fields() {
    let (app, log_path) = access_logging_openai_app().await;

    post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gpt-4.1",
            "temperature": 0.2,
            "max_tokens": 64,
            "messages": [
                { "role": "system", "content": "rules" },
                { "role": "user", "content": "hello" }
            ]
        }),
    )
    .await;

    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"has_temperature\":true"));
    assert!(body.contains("\"has_max_tokens\":true"));
    assert!(body.contains("\"system_message_count\":1"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test access_log_records_request_summary_fields -q`
Expected: FAIL if summary fields are missing from logged JSON

- [ ] **Step 3: Update docs and examples**

```dotenv
ACCESS_LOG_PATH=./logs/access.jsonl
```

```md
- `ACCESS_LOG_PATH` appends structured JSONL access events for request start,
  upstream attempts, and request finish.
- Access logs include request-shape summaries only; raw prompts are never written.
```

- [ ] **Step 4: Run verification commands**

Run: `cargo fmt --all`
Expected: PASS

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add README.md .env.example tests/access_logging_api.rs
git commit -m "docs: document access log configuration"
```

## Self-Review

- Spec coverage:
  - Dedicated subsystem: Task 1
  - Request summaries without prompt persistence: Task 2
  - Start/attempt/finish events: Task 3
  - Non-fatal logging failures: Task 3
  - Config and docs: Tasks 1 and 4
  - Success/fallback/failure/redaction tests: Tasks 3 and 4
- Placeholder scan:
  - No `TODO`, `TBD`, or “similar to above” placeholders remain.
- Type consistency:
  - Plan uses `access_log_path`, `AccessLogger`, `AccessLogEvent`, and `RequestSummary` consistently across all tasks.
