mod common;

#[tokio::test]
async fn writes_one_usage_record_for_successful_request() {
    let (app, log_path) = common::usage_logging_app().await;
    let response = common::post_json(
        app,
        "/v1/chat/completions",
        common::sample_chat_body("gpt-4.1"),
    )
    .await;

    assert_eq!(response.status, 200);
    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"status\":\"success\""));
}

#[tokio::test]
async fn writes_usage_record_with_cost_fields() {
    let (app, log_path) = common::usage_logging_app().await;
    let _ = common::post_json(
        app,
        "/v1/chat/completions",
        common::sample_chat_body("gpt-4.1"),
    )
    .await;

    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"timestamp\""));
    assert!(body.contains("\"estimated_cost\""));
}
