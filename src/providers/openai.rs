use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{
        request::{UnifiedMessage, UnifiedRequest},
        response::UnifiedResponse,
    },
    error::AppError,
    outbound_audit::OutboundAuditLogger,
    providers::{
        ProviderAdapter, audit_http_call, endpoint_path, json_string, json_u32, map_reqwest_error,
        usage,
    },
};

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    outbound_audit_logger: OutboundAuditLogger,
    proxy_enabled: bool,
}

impl OpenAiProvider {
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
            "messages": request.messages.iter().map(to_openai_message).collect::<Vec<_>>(),
            "input": request.messages.first().map(|message| message.content.clone()),
            "temperature": request.temperature,
            "max_tokens": request.max_output_tokens,
            "max_output_tokens": request.max_output_tokens,
            "stream": request.stream
        })
    }

    async fn execute(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        let url = format!("{}{}", self.base_url, endpoint_path(request.api_kind));
        let request_body = self.build_request_body(&request);
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
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
                    "openai",
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
            "openai",
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

        let text = json_string(&body, &["choices", "0", "message", "content"])
            .or_else(|| json_string(&body, &["output_text"]))
            .or_else(|| json_string(&body, &["output", "0", "content", "0", "text"]))
            .unwrap_or_default();

        Ok(UnifiedResponse {
            text,
            finish_reason: json_string(&body, &["choices", "0", "finish_reason"])
                .or_else(|| json_string(&body, &["status"])),
            usage: usage(
                json_u32(&body, &["usage", "prompt_tokens"])
                    .or_else(|| json_u32(&body, &["usage", "input_tokens"])),
                json_u32(&body, &["usage", "completion_tokens"])
                    .or_else(|| json_u32(&body, &["usage", "output_tokens"])),
            ),
            provider: "openai".into(),
            model: request.route.public_name,
        })
    }
}

#[async_trait]
impl ProviderAdapter for OpenAiProvider {
    async fn complete(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        self.execute(request).await
    }

    async fn stream(
        &self,
        mut request: UnifiedRequest,
    ) -> Result<crate::domain::response::EventStream, AppError> {
        request.stream = true;
        let url = format!("{}{}", self.base_url, endpoint_path(request.api_kind));
        let request_body = self.build_request_body(&request);
        let request_size = request_body.to_string().len() as u64;
        let started_at = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
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
                    "openai",
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
            "openai",
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
                crate::providers::json_string(&json, &["choices", "0", "delta", "content"])
                    .map(crate::domain::response::StreamEvent::TextDelta)
                    .or_else(|| {
                        crate::providers::json_string(&json, &["delta", "content"])
                            .map(crate::domain::response::StreamEvent::TextDelta)
                    })
            },
        ))
    }
}

fn to_openai_message(message: &UnifiedMessage) -> Value {
    json!({
        "role": format!("{:?}", message.role).to_lowercase(),
        "content": message.content
    })
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
    async fn maps_unified_request_to_openai_payload() {
        let adapter = OpenAiProvider::new(
            reqwest::Client::new(),
            "secret".into(),
            "http://localhost".into(),
            OutboundAuditLogger::default(),
            false,
        );
        let payload = adapter.build_request_body(&sample_unified_request());

        assert_eq!(payload["model"], "gpt-4.1");
        assert_eq!(payload["messages"][0]["content"], "hello");
    }

    fn sample_unified_request() -> UnifiedRequest {
        UnifiedRequest {
            request_id: "req_123".into(),
            api_kind: ApiKind::ChatCompletions,
            route: ModelRoute {
                provider: ProviderKind::OpenAi,
                public_name: "gpt-4.1".into(),
                upstream_name: "gpt-4.1".into(),
                capabilities: crate::config::ModelCapabilities::all(),
            },
            model: "gpt-4.1".into(),
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
