use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{request::UnifiedRequest, response::UnifiedResponse},
    error::AppError,
    outbound_audit::OutboundAuditLogger,
    providers::{
        ProviderAdapter, audit_http_call, json_string, json_u32, map_reqwest_error,
        non_system_messages, system_prompt, usage,
    },
};

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    outbound_audit_logger: OutboundAuditLogger,
    proxy_enabled: bool,
}

impl AnthropicProvider {
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
        let url = format!("{}/v1/messages", self.base_url);
        let request_body = self.build_request_body(&request);
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let error = map_reqwest_error(error);
                audit_http_call(
                    &self.outbound_audit_logger,
                    Some(request.request_id.clone()),
                    "anthropic",
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
            "anthropic",
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
        let url = format!("{}/v1/messages", self.base_url);
        let request_body = serde_json::json!({
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
        });
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let error = map_reqwest_error(error);
                audit_http_call(
                    &self.outbound_audit_logger,
                    Some(request.request_id.clone()),
                    "anthropic",
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
            "anthropic",
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
            OutboundAuditLogger::default(),
            false,
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
