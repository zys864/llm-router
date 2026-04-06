use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{request::UnifiedRequest, response::UnifiedResponse},
    error::AppError,
    outbound_audit::OutboundAuditLogger,
    providers::{
        ProviderAdapter, audit_http_call, json_string, json_u32, map_reqwest_error, usage,
    },
};

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    outbound_audit_logger: OutboundAuditLogger,
    proxy_enabled: bool,
}

impl GeminiProvider {
    pub fn new(
        client: Client,
        api_key: String,
        base_url: String,
        outbound_audit_logger: OutboundAuditLogger,
        proxy_enabled: bool,
    ) -> Self {
        Self {
            client,
            api_key,
            base_url,
            outbound_audit_logger,
            proxy_enabled,
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
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url, request.route.upstream_name, self.api_key
        );
        let request_body = self.build_request_body(&request);
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self.client.post(&url).json(&request_body).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let error = map_reqwest_error(error);
                audit_http_call(
                    &self.outbound_audit_logger,
                    Some(request.request_id.clone()),
                    "gemini",
                    "POST",
                    &url,
                    "failure",
                    started_at.elapsed().as_millis(),
                    None,
                    Some(request_size),
                    Some((error.error_type(), error.to_string())),
                    self.proxy_enabled,
                )
                .await;
                return Err(error);
            }
        };

        let status = response.status();
        audit_http_call(
            &self.outbound_audit_logger,
            Some(request.request_id.clone()),
            "gemini",
            "POST",
            &url,
            if status.is_success() {
                "success"
            } else {
                "failure"
            },
            started_at.elapsed().as_millis(),
            Some(status.as_u16()),
            Some(request_size),
            None,
            self.proxy_enabled,
        )
        .await;
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
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, request.route.upstream_name, self.api_key
        );
        let request_body = self.build_request_body(&request);
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self.client.post(&url).json(&request_body).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let error = map_reqwest_error(error);
                audit_http_call(
                    &self.outbound_audit_logger,
                    Some(request.request_id.clone()),
                    "gemini",
                    "POST",
                    &url,
                    "failure",
                    started_at.elapsed().as_millis(),
                    None,
                    Some(request_size),
                    Some((error.error_type(), error.to_string())),
                    self.proxy_enabled,
                )
                .await;
                return Err(error);
            }
        };

        let status = response.status();
        audit_http_call(
            &self.outbound_audit_logger,
            Some(request.request_id.clone()),
            "gemini",
            "POST",
            &url,
            if status.is_success() {
                "success"
            } else {
                "failure"
            },
            started_at.elapsed().as_millis(),
            Some(status.as_u16()),
            Some(request_size),
            None,
            self.proxy_enabled,
        )
        .await;
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
            OutboundAuditLogger::default(),
            false,
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
