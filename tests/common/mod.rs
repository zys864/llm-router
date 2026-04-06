#![allow(dead_code)]

use std::path::PathBuf;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::util::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, method, path},
};

use llm_router::{
    build_app,
    config::{AppConfig, ModelCapabilities, ModelConfig, ModelTargetConfig, ProxyKeyConfig},
    providers::ProviderKind,
};

pub struct TestResponse {
    pub status: StatusCode,
    pub body: String,
}

pub async fn get(app: axum::Router, uri: &str) -> TestResponse {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    response_to_text(response).await
}

pub async fn get_with_bearer(app: axum::Router, uri: &str, token: &str) -> TestResponse {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response_to_text(response).await
}

pub async fn post_json(app: axum::Router, uri: &str, value: serde_json::Value) -> TestResponse {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    response_to_text(response).await
}

pub async fn post_json_with_bearer(
    app: axum::Router,
    uri: &str,
    token: &str,
    value: serde_json::Value,
) -> TestResponse {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    response_to_text(response).await
}

pub async fn response_to_text(response: axum::response::Response) -> TestResponse {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    TestResponse {
        status,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}

pub fn model(public_name: &str, provider: ProviderKind, upstream_name: &str) -> ModelConfig {
    ModelConfig {
        public_name: public_name.into(),
        capabilities: ModelCapabilities::all(),
        targets: vec![ModelTargetConfig {
            provider,
            upstream_name: upstream_name.into(),
            priority: 100,
            capabilities: ModelCapabilities::all(),
        }],
    }
}

pub fn fallback_model(public_name: &str) -> ModelConfig {
    ModelConfig {
        public_name: public_name.into(),
        capabilities: ModelCapabilities::all(),
        targets: vec![
            ModelTargetConfig {
                provider: ProviderKind::OpenAi,
                upstream_name: "gpt-4.1".into(),
                priority: 100,
                capabilities: ModelCapabilities::all(),
            },
            ModelTargetConfig {
                provider: ProviderKind::Anthropic,
                upstream_name: "claude-sonnet-4-20250514".into(),
                priority: 50,
                capabilities: ModelCapabilities::all(),
            },
        ],
    }
}

pub fn auth_keys(limit: u64) -> Vec<ProxyKeyConfig> {
    vec![ProxyKeyConfig {
        id: "team-alpha".into(),
        api_key: "lr_live_alpha".into(),
        max_requests: limit,
    }]
}

pub async fn openai_app(body_text: &str) -> axum::Router {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_string_contains("\"stream\":true"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(format!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\ndata: [DONE]\n\n",
                    body_text
                )),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{ "message": { "content": body_text }, "finish_reason": "stop" }],
            "usage": { "prompt_tokens": 4, "completion_tokens": 3 }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("\"stream\":true"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(format!(
                    "data: {{\"delta\":{{\"content\":\"{}\"}}}}\n\ndata: [DONE]\n\n",
                    body_text
                )),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "output_text": body_text,
            "usage": { "input_tokens": 4, "output_tokens": 3 }
        })))
        .mount(&server)
        .await;

    build_app(AppConfig {
        openai_api_key: Some("test-openai".into()),
        openai_base_url: server.uri(),
        models: vec![model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1")],
        ..AppConfig::default()
    })
    .await
}

pub async fn slow_openai_app() -> axum::Router {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(200))
                .set_body_json(serde_json::json!({
                    "choices": [{ "message": { "content": "too slow" }, "finish_reason": "stop" }]
                })),
        )
        .mount(&server)
        .await;

    build_app(AppConfig {
        request_timeout_secs: 0,
        openai_api_key: Some("test-openai".into()),
        openai_base_url: server.uri(),
        models: vec![model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1")],
        ..AppConfig::default()
    })
    .await
}

pub async fn anthropic_app(body_text: &str) -> axum::Router {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "text": body_text }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 4, "output_tokens": 3 }
        })))
        .mount(&server)
        .await;

    build_app(AppConfig {
        anthropic_api_key: Some("test-anthropic".into()),
        anthropic_base_url: server.uri(),
        models: vec![model(
            "claude-sonnet-4",
            ProviderKind::Anthropic,
            "claude-sonnet-4-20250514",
        )],
        ..AppConfig::default()
    })
    .await
}

pub async fn gemini_app(body_text: &str) -> axum::Router {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": body_text }] },
                "finishReason": "STOP"
            }],
            "usageMetadata": { "promptTokenCount": 4, "candidatesTokenCount": 3 }
        })))
        .mount(&server)
        .await;

    build_app(AppConfig {
        gemini_api_key: Some("test-gemini".into()),
        gemini_base_url: server.uri(),
        models: vec![model(
            "gemini-2.5-pro",
            ProviderKind::Gemini,
            "gemini-2.5-pro",
        )],
        ..AppConfig::default()
    })
    .await
}

pub async fn auth_enabled_app() -> axum::Router {
    build_app(AppConfig {
        proxy_keys: auth_keys(10),
        models: vec![model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1")],
        ..AppConfig::default()
    })
    .await
}

pub async fn quota_limited_app() -> axum::Router {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }]
        })))
        .mount(&server)
        .await;

    build_app(AppConfig {
        proxy_keys: auth_keys(1),
        openai_api_key: Some("test-openai".into()),
        openai_base_url: server.uri(),
        models: vec![model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1")],
        ..AppConfig::default()
    })
    .await
}

pub async fn fallback_openai_then_anthropic_app() -> axum::Router {
    let openai = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&openai)
        .await;

    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "text": "anthropic says hi" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 4, "output_tokens": 3 }
        })))
        .mount(&anthropic)
        .await;

    build_app(AppConfig {
        openai_api_key: Some("test-openai".into()),
        openai_base_url: openai.uri(),
        anthropic_api_key: Some("test-anthropic".into()),
        anthropic_base_url: anthropic.uri(),
        models: vec![fallback_model("smart-fallback")],
        ..AppConfig::default()
    })
    .await
}

pub async fn usage_logging_app() -> (axum::Router, PathBuf) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{ "message": { "content": "hello from upstream" }, "finish_reason": "stop" }]
        })))
        .mount(&server)
        .await;

    let path = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    let log_path = path.to_path_buf();
    path.keep().unwrap();

    let app = build_app(AppConfig {
        openai_api_key: Some("test-openai".into()),
        openai_base_url: server.uri(),
        usage_log_path: Some(log_path.display().to_string()),
        models: vec![model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1")],
        ..AppConfig::default()
    })
    .await;

    (app, log_path)
}

pub async fn models_only_app() -> axum::Router {
    build_app(AppConfig {
        models: vec![
            model("gpt-4.1", ProviderKind::OpenAi, "gpt-4.1"),
            model(
                "claude-sonnet-4",
                ProviderKind::Anthropic,
                "claude-sonnet-4-20250514",
            ),
        ],
        ..AppConfig::default()
    })
    .await
}

pub fn sample_chat_body(model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": "hello" }]
    })
}
