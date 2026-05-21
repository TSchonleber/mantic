use super::{ChatRequest, ChatResponse, LlmBackend};
use crate::error::{AppError, Result};
use async_trait::async_trait;

/// Placeholder Mantic-managed LLM proxy. Returns `LlmProxyNotAvailable`
/// until sub-project #8 (brainctl.org backend) lands. The wire format
/// will mirror Anthropic's Messages API.
pub struct ManticProxy {
    _license_jwt: String,
    _base_url: String,
}

impl ManticProxy {
    pub fn new(license_jwt: impl Into<String>) -> Self {
        Self {
            _license_jwt: license_jwt.into(),
            _base_url: "https://brainctl.org/v1/llm-proxy".to_string(),
        }
    }
}

#[async_trait]
impl LlmBackend for ManticProxy {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Err(AppError::LlmProxyNotAvailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::llm::Message;

    #[tokio::test]
    async fn always_returns_proxy_not_available() {
        let backend = ManticProxy::new("fake-jwt");
        let err = backend
            .chat(ChatRequest {
                system: "".into(),
                messages: vec![Message::User {
                    content: "hi".into(),
                }],
                model: "claude-sonnet-4-6".into(),
                max_tokens: 10,
                temperature: 0.0,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::LlmProxyNotAvailable));
    }
}
