use super::{ChatRequest, ChatResponse, LlmBackend};
use crate::error::{AppError, Result};
use async_trait::async_trait;

pub struct ManticProxy {
    #[allow(dead_code)]
    jwt: String,
}

impl ManticProxy {
    pub fn new(jwt: impl Into<String>) -> Self {
        Self { jwt: jwt.into() }
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
