mod common;

#[tokio::test]
async fn lists_configured_models() {
    let app = common::models_only_app();
    let response = common::get(app, "/v1/models").await;
    assert_eq!(response.status, 200);
    assert!(response.body.contains("claude-sonnet-4"));
}
