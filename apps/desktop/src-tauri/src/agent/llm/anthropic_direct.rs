use super::{ChatRequest, ChatResponse, LlmBackend, Message, TokenUsage};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicDirect {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicDirect {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        Self {
            client,
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: Option<UsageWire>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageWire {
    input_tokens: u32,
    output_tokens: u32,
}

#[async_trait]
impl LlmBackend for AnthropicDirect {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "system": request.system,
            "temperature": request.temperature,
            "messages": request.messages.iter().map(|m| match m {
                Message::User { content } => json!({"role": "user", "content": content}),
                Message::Assistant { content } => json!({"role": "assistant", "content": content}),
            }).collect::<Vec<_>>(),
        });

        let url = format!("{}/v1/messages", self.base_url);
        let res = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmBackend(format!("anthropic request: {e}")))?;

        let status = res.status();
        if !status.is_success() {
            let body_text = res.text().await.unwrap_or_default();
            return Err(AppError::LlmBackend(format!(
                "anthropic returned {}: {}",
                status, body_text
            )));
        }

        let parsed: AnthropicResponse = res
            .json()
            .await
            .map_err(|e| AppError::LlmBackend(format!("anthropic parse: {e}")))?;

        let text = parsed
            .content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(ChatResponse {
            content: text,
            stop_reason: parsed.stop_reason.unwrap_or_else(|| "unknown".to_string()),
            usage: parsed
                .usage
                .map(|u| TokenUsage { input_tokens: u.input_tokens, output_tokens: u.output_tokens })
                .unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn happy_path_parses_text_content() {
        let mut server = Server::new_async().await;
        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-6",
                "content": [{"type": "text", "text": "hello"}],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 10, "output_tokens": 5}
            }"#)
            .create_async()
            .await;

        let backend = AnthropicDirect::with_base_url("sk-test", server.url());
        let res = backend.chat(ChatRequest {
            system: "be helpful".into(),
            messages: vec![Message::User { content: "hi".into() }],
            model: "claude-sonnet-4-6".into(),
            max_tokens: 100,
            temperature: 0.0,
        }).await.unwrap();

        assert_eq!(res.content, "hello");
        assert_eq!(res.stop_reason, "end_turn");
        assert_eq!(res.usage.input_tokens, 10);
        assert_eq!(res.usage.output_tokens, 5);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn http_4xx_surfaces_llm_backend_error() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/v1/messages")
            .with_status(401)
            .with_body(r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#)
            .create_async()
            .await;

        let backend = AnthropicDirect::with_base_url("bad-key", server.url());
        let err = backend.chat(ChatRequest {
            system: "".into(),
            messages: vec![Message::User { content: "hi".into() }],
            model: "claude-sonnet-4-6".into(),
            max_tokens: 10,
            temperature: 0.0,
        }).await.unwrap_err();

        match err {
            AppError::LlmBackend(msg) => assert!(msg.contains("401")),
            other => panic!("expected LlmBackend, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn concatenates_multiple_text_blocks_and_ignores_non_text() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_body(r#"{
                "id": "msg_2",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-6",
                "content": [
                    {"type": "text", "text": "part one "},
                    {"type": "tool_use", "id": "x"},
                    {"type": "text", "text": "part two"}
                ],
                "stop_reason": "end_turn"
            }"#)
            .create_async()
            .await;

        let backend = AnthropicDirect::with_base_url("k", server.url());
        let res = backend.chat(ChatRequest {
            system: "".into(),
            messages: vec![Message::User { content: "hi".into() }],
            model: "claude-sonnet-4-6".into(),
            max_tokens: 10,
            temperature: 0.0,
        }).await.unwrap();

        assert_eq!(res.content, "part one part two");
    }
}
