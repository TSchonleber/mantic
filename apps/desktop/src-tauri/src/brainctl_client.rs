use crate::error::{AppError, Result};
use crate::mcp_codec::{IdGen, Request, Response};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

pub struct BrainctlClient {
    inner: Arc<Mutex<Option<ClientState>>>,
    binary_path: PathBuf,
    db_path: PathBuf,
    ids: IdGen,
    request_timeout: Duration,
}

struct ClientState {
    child: Child,
    stdin: ChildStdin,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, (i64, String)>>>>>,
    _reader_task: JoinHandle<()>,
}

impl BrainctlClient {
    pub fn new(binary_path: PathBuf, db_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            binary_path,
            db_path,
            ids: IdGen::new(),
            request_timeout: Duration::from_secs(5),
        }
    }

    /// Send a JSON-RPC request and await its response.
    /// Spawns the subprocess on first call. On crash, respawns up to 3 times.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let mut attempts = 0;
        let mut last_err: Option<AppError> = None;
        while attempts < 3 {
            attempts += 1;
            match self.try_call(method, params.clone()).await {
                Ok(v) => return Ok(v),
                Err(AppError::BrainctlUnavailable { reason }) => {
                    last_err = Some(AppError::BrainctlUnavailable { reason });
                    // tear down any dead client + backoff
                    let mut guard = self.inner.lock().await;
                    *guard = None;
                    drop(guard);
                    tokio::time::sleep(Duration::from_millis(250 * (1 << (attempts - 1)))).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or(AppError::BrainctlUnavailable {
            reason: "exhausted retries".into(),
        }))
    }

    async fn try_call(&self, method: &str, params: Value) -> Result<Value> {
        self.ensure_spawned().await?;
        let id = self.ids.next();
        let req = Request::new(id, method, params);
        let (tx, rx) = oneshot::channel();

        let mut guard = self.inner.lock().await;
        let state = guard
            .as_mut()
            .ok_or_else(|| AppError::BrainctlUnavailable {
                reason: "subprocess not running".into(),
            })?;
        state.pending.lock().await.insert(id, tx);
        let encoded = req.encode();
        state.stdin.write_all(encoded.as_bytes()).await.map_err(|e| {
            AppError::BrainctlUnavailable {
                reason: format!("stdin write failed: {e}"),
            }
        })?;
        state.stdin.flush().await.ok();
        drop(guard);

        match tokio::time::timeout(self.request_timeout, rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err((code, message)))) => Err(AppError::Brainctl { code, message }),
            Ok(Err(_)) => Err(AppError::BrainctlUnavailable {
                reason: "response channel closed (subprocess crash?)".into(),
            }),
            Err(_) => Err(AppError::Brainctl {
                code: -32001,
                message: "request timeout".into(),
            }),
        }
    }

    async fn ensure_spawned(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let mut cmd = Command::new(&self.binary_path);
        cmd.env("BRAINCTL_DB", &self.db_path);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("failed to spawn brainctl-mcp ({}): {e}", self.binary_path.display()),
        })?;

        let stdin = child.stdin.take().ok_or_else(|| AppError::BrainctlUnavailable {
            reason: "failed to capture child stdin".into(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| AppError::BrainctlUnavailable {
            reason: "failed to capture child stdout".into(),
        })?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, (i64, String)>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_for_reader = pending.clone();

        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let resp = match Response::parse(&line) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let Some(id) = resp.id else {
                    continue; // server notification, ignore
                };
                let mut p = pending_for_reader.lock().await;
                if let Some(tx) = p.remove(&id) {
                    let payload = if let Some(err) = resp.error {
                        Err((err.code, err.message))
                    } else {
                        Ok(resp.result.unwrap_or(Value::Null))
                    };
                    let _ = tx.send(payload);
                }
            }
        });

        // Send MCP initialize handshake — best-effort, ignore response.
        let init_req = Request::new(
            0,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "mantic-desktop", "version": "0.0.0" }
            }),
        );

        let mut stdin = stdin;
        stdin.write_all(init_req.encode().as_bytes()).await.ok();
        stdin.flush().await.ok();

        *guard = Some(ClientState {
            child,
            stdin,
            pending,
            _reader_task: reader_task,
        });
        Ok(())
    }

    /// Send SIGTERM, wait briefly, then SIGKILL.
    pub async fn shutdown(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(mut state) = guard.take() {
            let _ = state.child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), state.child.wait()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn call_against_missing_binary_returns_unavailable() {
        let client = BrainctlClient::new(
            PathBuf::from("/definitely/not/here/brainctl-mcp"),
            PathBuf::from("/tmp/test-brain.db"),
        );
        let r = client.call("memory_add", json!({"content": "x", "category": "lesson"})).await;
        assert!(matches!(r, Err(AppError::BrainctlUnavailable { .. })));
    }

    #[tokio::test]
    async fn call_against_echo_binary_round_trips() {
        // Use a tiny shell echo as the "subprocess" — it reads stdin, writes a canned response.
        // We script it to read one line and emit a valid JSON-RPC response for it.
        let script = r#"#!/usr/bin/env bash
read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"ok":true}}'
sleep 1
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), script).unwrap();
        let mut perms = std::fs::metadata(tmp.path()).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        std::fs::set_permissions(tmp.path(), perms).unwrap();

        let client = BrainctlClient::new(
            tmp.path().to_path_buf(),
            PathBuf::from("/tmp/test-brain.db"),
        );
        // The echo emits id=1 regardless; ensure our IdGen starts at 1 too (it does).
        let r = client.call("memory_add", json!({"content": "x"})).await;
        assert!(r.is_ok(), "expected ok, got {r:?}");
        assert_eq!(r.unwrap()["ok"], true);
        client.shutdown().await;
    }
}
