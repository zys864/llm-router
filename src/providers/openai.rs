use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::{
    domain::{
        request::{UnifiedMessage, UnifiedRequest},
        response::UnifiedResponse,
    },
    error::AppError,
    providers::{
        ProviderAdapter, endpoint_path, json_string, json_u32, map_reqwest_error, simple_stream,
        usage,
    },
};

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
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
            "messages": request.messages.iter().map(to_openai_message).collect::<Vec<_>>(),
            "input": request.messages.first().map(|message| message.content.clone()),
            "temperature": request.temperature,
            "max_tokens": request.max_output_tokens,
            "max_output_tokens": request.max_output_tokens,
            "stream": false
        })
    }

    async fn execute(&self, request: UnifiedRequest) -> Result<UnifiedResponse, AppError> {
        let response = self
            .client
            .post(format!(
                "{}{}",
                self.base_url,
                endpoint_path(request.api_kind)
            ))
            .bearer_auth(&self.api_key)
            .json(&self.build_request_body(&request))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
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
    ) -> Result<Vec<crate::domain::response::StreamEvent>, AppError> {
        request.stream = false;
        let response = self.execute(request).await?;
        Ok(simple_stream(&response))
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
            },
            messages: vec![UnifiedMessage {
                role: UnifiedRole::User,
                content: "hello".into(),
            }],
            temperature: Some(0.2),
            max_output_tokens: Some(128),
            stream: false,
        }
    }
}
