mod common;

#[tokio::test]
async fn rejects_missing_proxy_key_when_auth_is_enabled() {
    let app = common::auth_enabled_app().await;
    let response = common::get(app, "/v1/models").await;

    assert_eq!(response.status, 401);
}

#[tokio::test]
async fn accepts_valid_proxy_key_when_auth_is_enabled() {
    let app = common::auth_enabled_app().await;
    let response = common::get_with_bearer(app, "/v1/models", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn rejects_request_after_quota_is_exhausted() {
    let app = common::quota_limited_app().await;

    let first = common::post_json_with_bearer(
        app.clone(),
        "/v1/chat/completions",
        "lr_live_alpha",
        common::sample_chat_body("gpt-4.1"),
    )
    .await;
    assert_eq!(first.status, 200);

    let second = common::post_json_with_bearer(
        app,
        "/v1/chat/completions",
        "lr_live_alpha",
        common::sample_chat_body("gpt-4.1"),
    )
    .await;
    assert_eq!(second.status, 429);
}
