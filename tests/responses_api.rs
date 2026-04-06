mod common;

#[tokio::test]
async fn proxies_responses_request() {
    let app = common::openai_app("hello from responses").await;
    let response = common::post_json(
        app,
        "/v1/responses",
        serde_json::json!({
            "model": "gpt-4.1",
            "input": "hello"
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("hello from responses"));
}

#[tokio::test]
async fn streams_responses_request() {
    let app = common::openai_app("hello from responses").await;
    let response = common::post_json(
        app,
        "/v1/responses",
        serde_json::json!({
            "model": "gpt-4.1",
            "input": "hello",
            "stream": true
        }),
    )
    .await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("data: [DONE]"));
}
