use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{request::UnifiedRequest, response::UnifiedResponse},
    error::AppError,
    providers::{
        ProviderAdapter, json_string, json_u32, map_reqwest_error, non_system_messages,
        system_prompt, usage,
    },
};

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(client: Client, api_key: String, base_url: String) -> Self {
        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn build_request_body(&self, request: &UnifiedRequest) -> Value {
        json!({
            "model": request.route.upstream_name,
            "system": system_prompt(&request.messages),
            "messages": non_system_messages(&request.messages)
                .into_iter()
                .map(|message| json!({
                    "role": format!("{:?}", message.role).to_lowercase(),
                    "content": [{ "type": "text", "text": message.content }]
                }))
                .collect::<Vec<_>>(),
            "temperature": request.temperature,
            "max_tokens": request.max_output_tokens.unwrap_or(1024)
        })
    }

    async fn execute(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&self.build_request_body(&request))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let body: Value = response.json().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(AppError::upstream(body.to_string()));
        }

        Ok(UnifiedResponse {
            text: json_string(&body, &["content", "0", "text"]).unwrap_or_default(),
            finish_reason: json_string(&body, &["stop_reason"]),
            usage: usage(
                json_u32(&body, &["usage", "input_tokens"]),
                json_u32(&body, &["usage", "output_tokens"]),
            ),
            provider: "anthropic".into(),
            model: request.route.public_name,
        })
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicProvider {
    async fn complete(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        self.execute(request).await
    }

    async fn stream(
        &self,
        request: UnifiedRequest,
    ) -> Result<crate::domain::response::EventStream, AppError> {
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "stream": true,
                "model": request.route.upstream_name,
                "system": system_prompt(&request.messages),
                "messages": non_system_messages(&request.messages)
                    .into_iter()
                    .map(|message| serde_json::json!({
                        "role": format!("{:?}", message.role).to_lowercase(),
                        "content": [{ "type": "text", "text": message.content }]
                    }))
                    .collect::<Vec<_>>(),
                "temperature": request.temperature,
                "max_tokens": request.max_output_tokens.unwrap_or(1024)
            }))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::upstream(
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "stream failed".into()),
            ));
        }

        Ok(crate::providers::streaming::sse_json_stream(
            response.bytes_stream(),
            |json| {
                crate::providers::json_string(&json, &["delta", "text"])
                    .map(crate::domain::response::StreamEvent::TextDelta)
                    .or_else(|| {
                        crate::providers::json_string(&json, &["content_block", "text"])
                            .map(crate::domain::response::StreamEvent::TextDelta)
                    })
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::request::{ApiKind, UnifiedMessage, UnifiedRole},
        providers::ProviderKind,
        router::ModelRoute,
    };

    #[tokio::test]
    async fn maps_unified_request_to_anthropic_payload() {
        let adapter = AnthropicProvider::new(
            reqwest::Client::new(),
            "secret".into(),
            "http://localhost".into(),
        );
        let payload = adapter.build_request_body(&sample_unified_request());
        assert_eq!(payload["model"], "claude-sonnet-4-20250514");
    }

    fn sample_unified_request() -> UnifiedRequest {
        UnifiedRequest {
            request_id: "req_123".into(),
            api_kind: ApiKind::ChatCompletions,
            route: ModelRoute {
                provider: ProviderKind::Anthropic,
                public_name: "claude-sonnet-4".into(),
                upstream_name: "claude-sonnet-4-20250514".into(),
                capabilities: crate::config::ModelCapabilities::all(),
            },
            model: "claude-sonnet-4".into(),
            messages: vec![UnifiedMessage {
                role: UnifiedRole::User,
                content: "hello".into(),
            }],
            temperature: Some(0.2),
            max_output_tokens: Some(128),
            stream: false,
            caller_id: None,
        }
    }
}
