use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Request {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }

    pub fn encode(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap();
        s.push('\n');
        s
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Response {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

impl Response {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

pub struct IdGen {
    next: AtomicU64,
}

impl IdGen {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    pub fn next(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_encode_includes_newline() {
        let req = Request::new(1, "memory_add", json!({"content": "hi"}));
        let encoded = req.encode();
        assert!(encoded.ends_with('\n'));
        let parsed: Value = serde_json::from_str(encoded.trim()).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "memory_add");
        assert_eq!(parsed["params"]["content"], "hi");
    }

    #[test]
    fn response_parse_success() {
        let line = r#"{"jsonrpc":"2.0","id":7,"result":{"memory_id":42}}"#;
        let resp = Response::parse(line).unwrap();
        assert_eq!(resp.id, Some(7));
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["memory_id"], 42);
    }

    #[test]
    fn response_parse_error() {
        let line = r#"{"jsonrpc":"2.0","id":8,"error":{"code":-32602,"message":"bad params"}}"#;
        let resp = Response::parse(line).unwrap();
        assert_eq!(resp.id, Some(8));
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
        assert_eq!(err.message, "bad params");
    }

    #[test]
    fn response_parse_notification_has_no_id() {
        let line = r#"{"jsonrpc":"2.0","method":"some/notification","params":{}}"#;
        let resp = Response::parse(line).unwrap();
        assert!(resp.id.is_none());
    }

    #[test]
    fn id_gen_is_monotonic() {
        let g = IdGen::new();
        assert_eq!(g.next(), 1);
        assert_eq!(g.next(), 2);
        assert_eq!(g.next(), 3);
    }
}
