use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{request::UnifiedRequest, response::UnifiedResponse},
    error::AppError,
    providers::{ProviderAdapter, json_string, json_u32, map_reqwest_error, usage},
};

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GeminiProvider {
    pub fn new(client: Client, api_key: String, base_url: String) -> Self {
        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn build_request_body(&self, request: &UnifiedRequest) -> Value {
        json!({
            "contents": request.messages.iter().map(|message| {
                json!({
                    "role": if format!("{:?}", message.role).to_lowercase() == "assistant" { "model" } else { "user" },
                    "parts": [{ "text": message.content }]
                })
            }).collect::<Vec<_>>(),
            "generationConfig": {
                "temperature": request.temperature,
                "maxOutputTokens": request.max_output_tokens
            }
        })
    }

    async fn execute(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        let response = self
            .client
            .post(format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                self.base_url, request.route.upstream_name, self.api_key
            ))
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
            text: json_string(&body, &["candidates", "0", "content", "parts", "0", "text"])
                .unwrap_or_default(),
            finish_reason: json_string(&body, &["candidates", "0", "finishReason"]),
            usage: usage(
                json_u32(&body, &["usageMetadata", "promptTokenCount"]),
                json_u32(&body, &["usageMetadata", "candidatesTokenCount"]),
            ),
            provider: "gemini".into(),
            model: request.route.public_name,
        })
    }
}

#[async_trait]
impl ProviderAdapter for GeminiProvider {
    async fn complete(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        self.execute(request).await
    }

    async fn stream(
        &self,
        request: UnifiedRequest,
    ) -> Result<crate::domain::response::EventStream, AppError> {
        let response = self
            .client
            .post(format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                self.base_url, request.route.upstream_name, self.api_key
            ))
            .json(&self.build_request_body(&request))
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
                crate::providers::json_string(
                    &json,
                    &["candidates", "0", "content", "parts", "0", "text"],
                )
                .map(crate::domain::response::StreamEvent::TextDelta)
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
    async fn maps_unified_request_to_gemini_payload() {
        let adapter = GeminiProvider::new(
            reqwest::Client::new(),
            "secret".into(),
            "http://localhost".into(),
        );
        let payload = adapter.build_request_body(&sample_unified_request());
        assert!(payload["contents"][0]["parts"][0]["text"] == "hello");
    }

    fn sample_unified_request() -> UnifiedRequest {
        UnifiedRequest {
            request_id: "req_123".into(),
            api_kind: ApiKind::ChatCompletions,
            route: ModelRoute {
                provider: ProviderKind::Gemini,
                public_name: "gemini-2.5-pro".into(),
                upstream_name: "gemini-2.5-pro".into(),
                capabilities: crate::config::ModelCapabilities::all(),
            },
            model: "gemini-2.5-pro".into(),
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
