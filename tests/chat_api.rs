mod common;

#[tokio::test]
async fn proxies_chat_completion_to_openai_adapter() {
    let app = common::openai_app("hello from upstream").await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gpt-4.1",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("hello from upstream"));
}

#[tokio::test]
async fn streams_chat_completion_in_openai_sse_format() {
    let app = common::openai_app("hello from upstream").await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gpt-4.1",
            "messages": [{ "role": "user", "content": "hello" }],
            "stream": true
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("data: [DONE]"));
}

#[tokio::test]
async fn proxies_chat_completion_to_anthropic_adapter() {
    let app = common::anthropic_app("anthropic says hi").await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "claude-sonnet-4",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("anthropic says hi"));
}

#[tokio::test]
async fn proxies_chat_completion_to_gemini_adapter() {
    let app = common::gemini_app("gemini says hi").await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gemini-2.5-pro",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("gemini says hi"));
}

#[tokio::test]
async fn returns_not_found_for_unknown_model() {
    let app = common::models_only_app().await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "missing",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, 400);
    assert!(response.body.contains("model `missing` not found"));
}

#[tokio::test]
async fn returns_timeout_error_for_slow_upstream() {
    let app = common::slow_openai_app().await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        serde_json::json!({
            "model": "gpt-4.1",
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    )
    .await;

    assert_eq!(response.status, 504);
    assert!(response.body.contains("\"request_id\""));
}

#[tokio::test]
async fn falls_back_to_second_target_after_upstream_failure() {
    let app = common::fallback_openai_then_anthropic_app().await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        common::sample_chat_body("smart-fallback"),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("anthropic says hi"));
}
