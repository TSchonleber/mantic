use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub token_symbol: String,
    pub source: String,
    pub context_tags: Vec<String>,
    pub payload: serde_json::Value,
}

impl Signal {
    pub fn new_test(token: impl Into<String>) -> Self {
        Self {
            id: short_id(),
            token_symbol: token.into(),
            source: "test".to_string(),
            context_tags: vec!["debug".to_string()],
            payload: serde_json::json!({"test": true}),
        }
    }
}

pub fn short_id() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[async_trait]
pub trait SignalSource: Send + Sync {
    async fn next(&self) -> Option<Signal>;
}

#[derive(Default, Clone)]
pub struct MockSignalSource {
    queue: Arc<Mutex<VecDeque<Signal>>>,
}

impl MockSignalSource {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub async fn push(&self, s: Signal) {
        self.queue.lock().await.push_back(s);
    }

    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }
}

#[async_trait]
impl SignalSource for MockSignalSource {
    async fn next(&self) -> Option<Signal> {
        self.queue.lock().await.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn push_pop_fifo() {
        let src = MockSignalSource::new();
        src.push(Signal::new_test("BONK")).await;
        src.push(Signal::new_test("WIF")).await;
        assert_eq!(src.len().await, 2);
        assert_eq!(src.next().await.unwrap().token_symbol, "BONK");
        assert_eq!(src.next().await.unwrap().token_symbol, "WIF");
        assert!(src.next().await.is_none());
    }

    #[test]
    fn short_id_is_16_hex_chars() {
        let s = short_id();
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn signal_serializes_round_trip() {
        let s = Signal::new_test("FOO");
        let json = serde_json::to_string(&s).unwrap();
        let back: Signal = serde_json::from_str(&json).unwrap();
        assert_eq!(back.token_symbol, "FOO");
        assert_eq!(back.id, s.id);
    }
}
