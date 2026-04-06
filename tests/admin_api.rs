mod common;

#[tokio::test]
async fn admin_models_returns_pricing_metadata() {
    let app = common::auth_enabled_admin_app().await;
    let response = common::get_with_bearer(app, "/admin/models", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"pricing\""));
}

#[tokio::test]
async fn admin_usage_summary_returns_aggregates() {
    let app = common::auth_enabled_admin_app().await;
    let response = common::get_with_bearer(app, "/admin/usage/summary", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"total_requests\""));
}

#[tokio::test]
async fn recovers_used_quota_from_existing_usage_log() {
    let (app, _) = common::recovered_quota_app().await;

    let response = common::post_json_with_bearer(
        app,
        "/v1/chat/completions",
        "lr_live_alpha",
        common::sample_chat_body("gpt-4.1"),
    )
    .await;

    assert_eq!(response.status, 429);
}
