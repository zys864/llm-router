mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn successful_request_writes_start_attempt_and_finish_events() {
    let (app, log_path) = common::access_logging_openai_app().await;

    let response = common::post_json(
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
    let (app, log_path) = common::access_logging_fallback_app().await;

    let response = common::post_json(
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
    let (app, log_path) = common::access_logging_failure_app().await;

    let response = common::post_json(
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

#[tokio::test]
async fn access_log_records_request_summary_fields() {
    let (app, log_path) = common::access_logging_openai_app().await;

    common::post_json(
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
