#![allow(dead_code)]

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::util::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use llm_router::{
    build_app,
    config::{AppConfig, ModelConfig},
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

pub async fn response_to_text(response: axum::response::Response) -> TestResponse {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    TestResponse {
        status,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}

pub async fn openai_app(body_text: &str) -> axum::Router {
    let server = MockServer::start().await;
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
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "output_text": body_text,
            "usage": { "input_tokens": 4, "output_tokens": 3 }
        })))
        .mount(&server)
        .await;

    build_app(AppConfig {
        openai_api_key: Some("test-openai".into()),
        openai_base_url: server.uri(),
        models: vec![ModelConfig {
            public_name: "gpt-4.1".into(),
            provider: ProviderKind::OpenAi,
            upstream_name: "gpt-4.1".into(),
        }],
        ..AppConfig::default()
    })
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
        models: vec![ModelConfig {
            public_name: "gpt-4.1".into(),
            provider: ProviderKind::OpenAi,
            upstream_name: "gpt-4.1".into(),
        }],
        ..AppConfig::default()
    })
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
        models: vec![ModelConfig {
            public_name: "claude-sonnet-4".into(),
            provider: ProviderKind::Anthropic,
            upstream_name: "claude-sonnet-4-20250514".into(),
        }],
        ..AppConfig::default()
    })
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
        models: vec![ModelConfig {
            public_name: "gemini-2.5-pro".into(),
            provider: ProviderKind::Gemini,
            upstream_name: "gemini-2.5-pro".into(),
        }],
        ..AppConfig::default()
    })
}

pub fn models_only_app() -> axum::Router {
    build_app(AppConfig {
        models: vec![
            ModelConfig {
                public_name: "gpt-4.1".into(),
                provider: ProviderKind::OpenAi,
                upstream_name: "gpt-4.1".into(),
            },
            ModelConfig {
                public_name: "claude-sonnet-4".into(),
                provider: ProviderKind::Anthropic,
                upstream_name: "claude-sonnet-4-20250514".into(),
            },
        ],
        ..AppConfig::default()
    })
}
