# Agent Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the engine that runs trading strategies — agent lifecycle, LLM-driven decision loop, paper-trading executor, decision logging to brain.db. After this lands, the user can create one or more agents in the desktop UI, "arm" them, fire test signals via a debug button, and watch the full loop run end-to-end: signal → brain orient → Claude decides → paper-execute → decision logged → live tape updates.

**Architecture:** Rust side adds an `agent/` module (config, state machine, signal source, paper executor, decision orchestrator, LlmBackend trait + AnthropicDirect + ManticProxy stub, runtime fleet manager). The runtime owns an `Arc<Mutex<HashMap<String, Arc<Agent>>>>`; each `Agent` is a tokio task that loops on an `mpsc::Receiver<Signal>`. Agent identity and config live in brain.db via the existing `BrainctlClient` — no new Mantic SQL tables. The frontend gets a new Fleet route (replacing Account as the post-wallet landing) plus a LiveTape component that polls `recent_events` every 5s.

**Tech Stack:** Rust (tokio, async-trait, reqwest, mockito for HTTP tests, serde, uuid via short-id helper, keyring for BYO LLM key), Tauri 2 (resource bundling already configured), React + Vite + Tailwind 4 (Fleet/AgentConfig/LiveTape), Vitest + @testing-library/react (frontend), Playwright headless Chromium against the Vite dev server (real-browser smoke test — new for this sub-project).

---

## Pre-flight

Working dir: `/Users/r4vager/Documents/Mantic`. Current `main` HEAD: `e3d5749` (the agent-runtime design spec commit). Create the feature branch in Task 1.

Verify prereqs:
```bash
node --version  # >= 20
pnpm --version  # >= 10
rustc --version # >= 1.78
```

### Safe verification commands (use these — no Finder pops, no DMG bundler)

```bash
# Rust unit + integration tests
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
cargo test

# Frontend tests
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test
pnpm --filter mantic-desktop lint
```

### DO NOT run during this plan

- `pnpm tauri build` — pops macOS DMG bundler / Finder
- `pnpm build:desktop` — same
- `pnpm dev:desktop` (i.e. `pnpm --filter mantic-desktop tauri dev`) — pops a Tauri window
- `cargo run` from `src-tauri/` — same

The Playwright smoke test in Task 21 uses `pnpm --filter mantic-desktop dev` (Vite dev server only, no Tauri window). That is safe because it runs in the browser, not the Tauri webview.

---

## File Structure

### New Rust modules: `apps/desktop/src-tauri/src/agent/`

- `mod.rs` — re-exports
- `runtime.rs` — `AgentRuntime` owns the fleet, exposes spawn/list/get/arm/pause/kill/dispatch
- `agent.rs` — `Agent` struct + the per-agent tokio task body
- `state.rs` — `AgentState` enum + transition helpers
- `config.rs` — `AgentConfig`, `AgentConfigInput`, `AgentSummary`, `AgentDetails`, serde
- `signal.rs` — `Signal`, `SignalSource` trait, `MockSignalSource`
- `executor.rs` — `Executor` trait, `PaperExecutor`, `Position`, `TradeResult`
- `decision.rs` — `build_prompt`, `parse_response`, `DecisionOutcome`
- `llm/mod.rs` — `LlmBackend` trait, `ChatRequest`, `ChatResponse`, `Message`, `TokenUsage`
- `llm/anthropic_direct.rs` — BYO-key Anthropic Messages client
- `llm/mantic_proxy.rs` — stub returning `ProxyNotAvailable`
- `llm/prompt.rs` — shared prompt-construction helper used by `decision::build_prompt`

### Modified Rust files

- `apps/desktop/src-tauri/src/error.rs` — add 5 new variants: `AgentNotFound`, `AgentInvalidState`, `LlmBackend`, `LlmProxyNotAvailable`, `PaperExecutor`
- `apps/desktop/src-tauri/src/state.rs` — add `agent_runtime: Arc<AgentRuntime>` field
- `apps/desktop/src-tauri/src/commands.rs` — add 8 commands
- `apps/desktop/src-tauri/src/lib.rs` — register `mod agent;`, manage `AgentRuntime`, register the 8 new commands, kick off first-run defaults
- `apps/desktop/src-tauri/src/license.rs` — add `LlmKeyStore` (or extend `KeyringStore` with a second slot helper) for `agent-llm-anthropic-key-<agent-id>`

### Modified / new frontend files

- `apps/desktop/src/lib/tauri-bridge.ts` — add Anthropic-shaped typed wrappers for the 8 new commands plus shared `Signal`, `AgentSummary`, `AgentDetails`, `AgentConfigInput` types
- `apps/desktop/src/lib/tauri-bridge.test.ts` (extend existing) — unit tests for the new wrappers
- `apps/desktop/src/routes/Fleet.tsx` (new) + `Fleet.test.tsx` (new)
- `apps/desktop/src/routes/AgentConfig.tsx` (new) + `AgentConfig.test.tsx` (new)
- `apps/desktop/src/components/LiveTape.tsx` (new) + `LiveTape.test.tsx` (new)
- `apps/desktop/src/App.tsx` — route paired+wallet → Fleet (Account becomes accessible via a header link inside Fleet)
- `apps/desktop/src/App.test.tsx` — extend the existing four-state suite to expect Fleet on `paired-with-wallet`

### New Playwright workspace

- `apps/desktop-smoke/package.json`
- `apps/desktop-smoke/playwright.config.ts`
- `apps/desktop-smoke/tests/fleet.spec.ts`
- `pnpm-workspace.yaml` (no change — `apps/*` already globs the new workspace)
- `package.json` (root) — add `test:smoke` script

### New Rust integration test

- `apps/desktop/src-tauri/tests/agent_loop_integration.rs` — gated on `MANTIC_BRAINCTL_BIN` env var or default sidecar location; otherwise skips

---

## Task 1: Branch + new error variants

**Files:**
- Modify: `apps/desktop/src-tauri/src/error.rs`

- [ ] **Step 1: Create feature branch**

```bash
cd /Users/r4vager/Documents/Mantic
git checkout main
git pull origin main
git checkout -b build/agent-runtime
```

Expected: `Switched to a new branch 'build/agent-runtime'`.

- [ ] **Step 2: Add 5 new variants to `AppError`**

Edit `apps/desktop/src-tauri/src/error.rs`. Append the new variants before the closing `}` of the `AppError` enum (after the existing `Bs58` variant):

```rust
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("agent in invalid state for this operation: {0}")]
    AgentInvalidState(String),

    #[error("llm backend error: {0}")]
    LlmBackend(String),

    #[error("mantic proxy is not available yet — switch to BYO mode or wait for sub-project #8")]
    LlmProxyNotAvailable,

    #[error("paper executor error: {0}")]
    PaperExecutor(String),
```

Do NOT touch the existing `impl Serialize for AppError` block — it already serializes by display string and will pick these up automatically.

- [ ] **Step 3: Verify it compiles**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
```

Expected: `Finished` with no errors (warnings about unused variants are fine — they're consumed in later tasks).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/error.rs
git commit -m "feat(agent): add 5 new AppError variants for agent runtime"
```

---

## Task 2: `LlmBackend` trait + `AnthropicDirect` impl

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/mod.rs`
- Create: `apps/desktop/src-tauri/src/agent/llm/mod.rs`
- Create: `apps/desktop/src-tauri/src/agent/llm/anthropic_direct.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (declare `mod agent;`)

- [ ] **Step 1: Create the `agent` module skeleton**

Create `apps/desktop/src-tauri/src/agent/mod.rs`:

```rust
pub mod config;
pub mod decision;
pub mod executor;
pub mod llm;
pub mod runtime;
pub mod signal;
pub mod state;
pub mod agent;

pub use agent::Agent;
pub use config::{AgentConfig, AgentConfigInput, AgentDetails, AgentSummary};
pub use runtime::AgentRuntime;
pub use signal::Signal;
pub use state::AgentState;
```

We'll create the actual module files in later tasks. For now, to keep `cargo check` green after Task 2, create minimal stubs for each referenced module. **Easiest path:** create the bare files in this task, leave them empty-but-syntactically-valid (just `// stub` comments), and flesh them out in subsequent tasks.

```bash
mkdir -p /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/agent/llm
touch /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/agent/{config,decision,executor,runtime,signal,state,agent}.rs
touch /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/agent/llm/{mantic_proxy,prompt}.rs
```

In each newly-touched file write a single line:

```rust
// stub — filled in by subsequent tasks of build/agent-runtime
```

In `mod.rs` re-exports we reference `Agent`, `AgentConfig`, `AgentSummary`, etc. Since those types don't exist yet, comment out the `pub use` lines and add them back at the end of each owning task. Leave only the `pub mod ...;` declarations active.

- [ ] **Step 2: Create `apps/desktop/src-tauri/src/agent/llm/mod.rs`**

```rust
pub mod anthropic_direct;
pub mod mantic_proxy;
pub mod prompt;

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    User { content: String },
    Assistant { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub stop_reason: String,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}
```

- [ ] **Step 3: Create `apps/desktop/src-tauri/src/agent/llm/anthropic_direct.rs`**

```rust
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
```

- [ ] **Step 4: Declare `mod agent;` in `lib.rs`**

In `apps/desktop/src-tauri/src/lib.rs`, after `pub mod wallet;` add:

```rust
mod agent;
```

- [ ] **Step 5: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
cargo test agent::llm::anthropic_direct
```

Expected: all 3 mockito tests pass; compile succeeds with dead-code warnings for the still-stubbed sibling modules.

- [ ] **Step 6: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(agent): LlmBackend trait + AnthropicDirect impl with mockito tests"
```

---

## Task 3: `ManticProxy` stub

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/llm/mantic_proxy.rs`

- [ ] **Step 1: Implement the stub**

Replace the stub in `apps/desktop/src-tauri/src/agent/llm/mantic_proxy.rs`:

```rust
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
        let err = backend.chat(ChatRequest {
            system: "".into(),
            messages: vec![Message::User { content: "hi".into() }],
            model: "claude-sonnet-4-6".into(),
            max_tokens: 10,
            temperature: 0.0,
        }).await.unwrap_err();
        assert!(matches!(err, AppError::LlmProxyNotAvailable));
    }
}
```

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::llm::mantic_proxy
```

Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/llm/mantic_proxy.rs
git commit -m "feat(agent): ManticProxy stub returning LlmProxyNotAvailable"
```

---

## Task 4: LLM key storage in keychain

**Files:**
- Modify: `apps/desktop/src-tauri/src/license.rs`

We add an `LlmKeyStore` that mirrors `KeyringStore`'s shape but takes a dynamic agent_id and produces a per-agent slot. Keeping it in `license.rs` for now since that file is where keychain plumbing lives; it can be promoted to its own file later.

- [ ] **Step 1: Add the new store**

Append to `apps/desktop/src-tauri/src/license.rs` (after the existing `clone_keyring_error` fn):

```rust
const LLM_SERVICE: &str = "org.brainctl.mantic";
const LLM_KEY_PREFIX: &str = "agent-llm-anthropic-key-";

/// Per-agent Anthropic API key storage. Slot: `agent-llm-anthropic-key-<agent_id>`.
pub struct LlmKeyStore;

impl LlmKeyStore {
    pub fn new() -> Self { Self }

    fn ensure_test_builder() {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
    }

    fn entry(&self, agent_id: &str) -> Result<keyring::Entry> {
        Self::ensure_test_builder();
        let slot = format!("{LLM_KEY_PREFIX}{agent_id}");
        keyring::Entry::new(LLM_SERVICE, &slot).map_err(AppError::Keyring)
    }

    pub fn save(&self, agent_id: &str, api_key: &str) -> Result<()> {
        self.entry(agent_id)?.set_password(api_key)?;
        Ok(())
    }

    pub fn load(&self, agent_id: &str) -> Result<Option<String>> {
        match self.entry(agent_id)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }

    pub fn clear(&self, agent_id: &str) -> Result<()> {
        match self.entry(agent_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }
}

impl Default for LlmKeyStore {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod llm_key_tests {
    use super::*;

    #[test]
    fn save_load_clear_roundtrip() {
        let store = LlmKeyStore::new();
        store.save("agent-7", "sk-test-abc").unwrap();
        assert_eq!(store.load("agent-7").unwrap().as_deref(), Some("sk-test-abc"));
        store.clear("agent-7").unwrap();
        assert!(store.load("agent-7").unwrap().is_none());
    }

    #[test]
    fn load_unknown_returns_none() {
        let store = LlmKeyStore::new();
        assert!(store.load("nonexistent-agent-id-xyz").unwrap().is_none());
    }

    #[test]
    fn clear_unknown_is_idempotent() {
        let store = LlmKeyStore::new();
        store.clear("nonexistent-agent-id-xyz").unwrap();
    }
}
```

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test license::llm_key_tests
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/license.rs
git commit -m "feat(agent): LlmKeyStore for per-agent Anthropic API keys"
```

---

## Task 5: `Signal` type + `MockSignalSource`

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/signal.rs`

- [ ] **Step 1: Implement `Signal` and the source trait**

Replace the stub in `apps/desktop/src-tauri/src/agent/signal.rs`:

```rust
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
    /// Returns the next signal, or `None` if no signals are queued.
    async fn next(&self) -> Option<Signal>;
}

#[derive(Default, Clone)]
pub struct MockSignalSource {
    queue: Arc<Mutex<VecDeque<Signal>>>,
}

impl MockSignalSource {
    pub fn new() -> Self {
        Self { queue: Arc::new(Mutex::new(VecDeque::new())) }
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
```

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::signal
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/signal.rs
git commit -m "feat(agent): Signal type + MockSignalSource"
```

---

## Task 6: `AgentConfig` + serde

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/config.rs`

- [ ] **Step 1: Implement the config types**

Replace the stub in `apps/desktop/src-tauri/src/agent/config.rs`:

```rust
use super::state::AgentState;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmBackendKind {
    #[serde(rename = "anthropic-direct")]
    AnthropicDirect,
    #[serde(rename = "mantic-proxy")]
    ManticProxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// brain.db entity_id, as a string. Stable agent identity.
    pub id: String,
    pub name: String,
    pub max_position_sol: f64,
    pub daily_loss_cap_sol: f64,
    pub nl_overlay: String,
    pub strategy_template_id: String, // "mock" for v1
    pub llm_backend: LlmBackendKind,
    pub llm_model: String, // e.g. "claude-sonnet-4-6"
    pub max_tokens: u32,
    pub temperature: f32,
}

impl AgentConfig {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(AppError::AgentInvalidState("name cannot be empty".into()));
        }
        if self.max_position_sol <= 0.0 || self.max_position_sol > 1000.0 {
            return Err(AppError::AgentInvalidState(
                "max_position_sol must be 0 < x <= 1000".into(),
            ));
        }
        if self.daily_loss_cap_sol < 0.0 {
            return Err(AppError::AgentInvalidState(
                "daily_loss_cap_sol must be non-negative".into(),
            ));
        }
        if self.max_tokens < 64 || self.max_tokens > 8192 {
            return Err(AppError::AgentInvalidState(
                "max_tokens must be 64..=8192".into(),
            ));
        }
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(AppError::AgentInvalidState(
                "temperature must be 0.0..=2.0".into(),
            ));
        }
        Ok(())
    }
}

/// Input shape from the frontend (no id — runtime assigns).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigInput {
    pub name: String,
    pub max_position_sol: f64,
    pub daily_loss_cap_sol: f64,
    pub nl_overlay: String,
    pub strategy_template_id: String,
    pub llm_backend: LlmBackendKind,
    pub llm_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl AgentConfigInput {
    pub fn into_config_with_id(self, id: String) -> AgentConfig {
        AgentConfig {
            id,
            name: self.name,
            max_position_sol: self.max_position_sol,
            daily_loss_cap_sol: self.daily_loss_cap_sol,
            nl_overlay: self.nl_overlay,
            strategy_template_id: self.strategy_template_id,
            llm_backend: self.llm_backend,
            llm_model: self.llm_model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

/// Summary returned by list/get for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub state: AgentState,
    pub has_llm_key: bool,
}

/// Detailed view including recent decisions/events ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDetails {
    pub config: AgentConfig,
    pub state: AgentState,
    pub has_llm_key: bool,
    /// Position ids currently open in PaperExecutor for this agent.
    pub open_position_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_config() -> AgentConfig {
        AgentConfig {
            id: "1".into(),
            name: "Test".into(),
            max_position_sol: 0.5,
            daily_loss_cap_sol: 2.0,
            nl_overlay: "".into(),
            strategy_template_id: "mock".into(),
            llm_backend: LlmBackendKind::AnthropicDirect,
            llm_model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            temperature: 0.0,
        }
    }

    #[test]
    fn valid_config_passes() { ok_config().validate().unwrap(); }

    #[test]
    fn empty_name_rejected() {
        let mut c = ok_config(); c.name = "   ".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn zero_position_size_rejected() {
        let mut c = ok_config(); c.max_position_sol = 0.0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn negative_daily_cap_rejected() {
        let mut c = ok_config(); c.daily_loss_cap_sol = -1.0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn tokens_out_of_range_rejected() {
        let mut c = ok_config(); c.max_tokens = 50;
        assert!(c.validate().is_err());
        c.max_tokens = 10_000;
        assert!(c.validate().is_err());
    }

    #[test]
    fn temperature_out_of_range_rejected() {
        let mut c = ok_config(); c.temperature = 3.0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn json_roundtrip_preserves_fields() {
        let c = ok_config();
        let s = serde_json::to_string(&c).unwrap();
        let back: AgentConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.name, c.name);
        assert_eq!(back.llm_model, c.llm_model);
        assert!(matches!(back.llm_backend, LlmBackendKind::AnthropicDirect));
    }

    #[test]
    fn input_into_config_assigns_id() {
        let inp = AgentConfigInput {
            name: "x".into(),
            max_position_sol: 0.5,
            daily_loss_cap_sol: 2.0,
            nl_overlay: "".into(),
            strategy_template_id: "mock".into(),
            llm_backend: LlmBackendKind::AnthropicDirect,
            llm_model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let c = inp.into_config_with_id("42".into());
        assert_eq!(c.id, "42");
    }
}
```

- [ ] **Step 2: Re-export from `mod.rs`**

In `apps/desktop/src-tauri/src/agent/mod.rs`, un-comment the `pub use config::*;` line (or add it now):

```rust
pub use config::{AgentConfig, AgentConfigInput, AgentDetails, AgentSummary};
```

This will fail to compile until Task 7 defines `AgentState` (which `AgentSummary` references), so keep `AgentState` un-exported here or stub a default `AgentState::Idle` in Task 7 before re-exporting. Easiest: leave the re-export commented and re-enable in Task 7.

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::config
```

Expected: 8 tests pass (after Task 7 defines `AgentState`). For this task, compilation requires Task 7 — so commit pair (Tasks 6 + 7) together is acceptable. **Recommended:** complete Task 7 before running `cargo test`, then commit Tasks 6 and 7 separately.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/config.rs
git commit -m "feat(agent): AgentConfig with validation + summary/detail types"
```

---

## Task 7: `AgentState` machine + transitions

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/state.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`

- [ ] **Step 1: Implement the state enum and transitions**

Replace the stub in `apps/desktop/src-tauri/src/agent/state.rs`:

```rust
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentState {
    Idle,
    Armed,
    Running { step: RunStep },
    Paused,
    Error { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStep {
    Orienting,
    CallingLlm,
    Executing,
    Logging,
}

impl AgentState {
    pub fn can_dispatch(&self) -> bool {
        matches!(self, AgentState::Armed)
    }

    pub fn can_arm(&self) -> bool {
        matches!(self, AgentState::Idle | AgentState::Paused | AgentState::Error { .. })
    }

    pub fn can_pause(&self) -> bool {
        matches!(self, AgentState::Armed | AgentState::Running { .. })
    }

    pub fn require_armed(&self) -> Result<()> {
        if self.can_dispatch() {
            Ok(())
        } else {
            Err(AppError::AgentInvalidState(format!(
                "agent must be Armed to dispatch a signal (current: {:?})", self
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_can_arm_not_dispatch() {
        let s = AgentState::Idle;
        assert!(s.can_arm());
        assert!(!s.can_dispatch());
        assert!(!s.can_pause());
    }

    #[test]
    fn armed_can_dispatch_pause_not_arm() {
        let s = AgentState::Armed;
        assert!(!s.can_arm());
        assert!(s.can_dispatch());
        assert!(s.can_pause());
    }

    #[test]
    fn running_can_pause_not_dispatch() {
        let s = AgentState::Running { step: RunStep::CallingLlm };
        assert!(!s.can_arm());
        assert!(!s.can_dispatch());
        assert!(s.can_pause());
    }

    #[test]
    fn error_can_arm_not_dispatch() {
        let s = AgentState::Error { reason: "x".into() };
        assert!(s.can_arm());
        assert!(!s.can_dispatch());
    }

    #[test]
    fn require_armed_on_idle_errors() {
        let s = AgentState::Idle;
        assert!(s.require_armed().is_err());
    }

    #[test]
    fn require_armed_on_armed_ok() {
        let s = AgentState::Armed;
        s.require_armed().unwrap();
    }

    #[test]
    fn serializes_with_tagged_kind() {
        let s = AgentState::Running { step: RunStep::Orienting };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"running\""));
        assert!(json.contains("\"step\":\"orienting\""));
        let back: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
```

- [ ] **Step 2: Re-enable re-exports in `mod.rs`**

In `apps/desktop/src-tauri/src/agent/mod.rs`:

```rust
pub mod agent;
pub mod config;
pub mod decision;
pub mod executor;
pub mod llm;
pub mod runtime;
pub mod signal;
pub mod state;

pub use config::{AgentConfig, AgentConfigInput, AgentDetails, AgentSummary};
pub use signal::Signal;
pub use state::AgentState;
// Agent and AgentRuntime re-exports come in Tasks 10 & 11.
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::state agent::config
```

Expected: 7 state tests + 8 config tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/state.rs apps/desktop/src-tauri/src/agent/mod.rs
git commit -m "feat(agent): AgentState machine with transition predicates"
```

---

## Task 8: `PaperExecutor` + position math

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/executor.rs`

- [ ] **Step 1: Implement the executor**

Replace the stub in `apps/desktop/src-tauri/src/agent/executor.rs`:

```rust
use super::signal::short_id;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub agent_id: String,
    pub token_symbol: String,
    pub entry_price_sol: f64,
    pub size_sol: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub opened_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub position: Position,
    pub exit_price_sol: f64,
    pub pnl_sol: f64,
    pub pnl_pct: f64,
    pub closed_at: i64,
}

pub struct PaperExecutor {
    positions: Mutex<HashMap<String, Position>>,
}

impl PaperExecutor {
    pub fn new() -> Self {
        Self { positions: Mutex::new(HashMap::new()) }
    }

    pub fn open(
        &self,
        agent_id: &str,
        token: &str,
        size_sol: f64,
        entry_price_sol: f64,
        tp_pct: f64,
        sl_pct: f64,
    ) -> Result<Position> {
        if size_sol <= 0.0 {
            return Err(AppError::PaperExecutor("size must be positive".into()));
        }
        if entry_price_sol <= 0.0 {
            return Err(AppError::PaperExecutor("entry price must be positive".into()));
        }
        let pos = Position {
            id: short_id(),
            agent_id: agent_id.to_string(),
            token_symbol: token.to_string(),
            entry_price_sol,
            size_sol,
            take_profit_pct: tp_pct,
            stop_loss_pct: sl_pct,
            opened_at: chrono::Utc::now().timestamp(),
        };
        self.positions.lock().unwrap().insert(pos.id.clone(), pos.clone());
        Ok(pos)
    }

    pub fn close(&self, position_id: &str, exit_price_sol: f64) -> Result<TradeResult> {
        if exit_price_sol <= 0.0 {
            return Err(AppError::PaperExecutor("exit price must be positive".into()));
        }
        let pos = self.positions.lock().unwrap().remove(position_id)
            .ok_or_else(|| AppError::PaperExecutor(format!("position {position_id} not found")))?;
        // Token-units = size_sol / entry_price_sol. P&L_sol = units * (exit - entry).
        let units = pos.size_sol / pos.entry_price_sol;
        let pnl_sol = units * (exit_price_sol - pos.entry_price_sol);
        let pnl_pct = (exit_price_sol - pos.entry_price_sol) / pos.entry_price_sol * 100.0;
        Ok(TradeResult {
            position: pos,
            exit_price_sol,
            pnl_sol,
            pnl_pct,
            closed_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn list_for(&self, agent_id: &str) -> Vec<Position> {
        self.positions
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.agent_id == agent_id)
            .cloned()
            .collect()
    }

    pub fn list_all(&self) -> Vec<Position> {
        self.positions.lock().unwrap().values().cloned().collect()
    }
}

impl Default for PaperExecutor {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_position() {
        let ex = PaperExecutor::new();
        let pos = ex.open("agent-1", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        assert_eq!(pos.agent_id, "agent-1");
        assert_eq!(pos.size_sol, 0.5);
        assert_eq!(ex.list_for("agent-1").len(), 1);
    }

    #[test]
    fn open_rejects_zero_size() {
        let ex = PaperExecutor::new();
        assert!(ex.open("a", "BONK", 0.0, 0.0001, 50.0, 20.0).is_err());
    }

    #[test]
    fn close_at_double_price_doubles_position_value() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        let res = ex.close(&pos.id, 0.0002).unwrap();
        // Bought 5000 units at 0.0001, sold at 0.0002 → +0.5 SOL
        assert!((res.pnl_sol - 0.5).abs() < 1e-9);
        assert!((res.pnl_pct - 100.0).abs() < 1e-9);
    }

    #[test]
    fn close_at_half_price_loses_half() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "WIF", 1.0, 1.0, 100.0, 50.0).unwrap();
        let res = ex.close(&pos.id, 0.5).unwrap();
        assert!((res.pnl_sol - (-0.5)).abs() < 1e-9);
        assert!((res.pnl_pct - (-50.0)).abs() < 1e-9);
    }

    #[test]
    fn close_removes_from_open_list() {
        let ex = PaperExecutor::new();
        let pos = ex.open("a", "BONK", 0.5, 0.0001, 50.0, 20.0).unwrap();
        ex.close(&pos.id, 0.0001).unwrap();
        assert!(ex.list_for("a").is_empty());
    }

    #[test]
    fn close_unknown_position_errors() {
        let ex = PaperExecutor::new();
        assert!(ex.close("nope", 1.0).is_err());
    }

    #[test]
    fn list_for_isolates_by_agent() {
        let ex = PaperExecutor::new();
        ex.open("a", "X", 0.5, 1.0, 10.0, 10.0).unwrap();
        ex.open("b", "Y", 0.5, 1.0, 10.0, 10.0).unwrap();
        assert_eq!(ex.list_for("a").len(), 1);
        assert_eq!(ex.list_for("b").len(), 1);
        assert_eq!(ex.list_all().len(), 2);
    }
}
```

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::executor
```

Expected: 7 tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/executor.rs
git commit -m "feat(agent): PaperExecutor with open/close + P&L math"
```

---

## Task 9: `decision::build_prompt` + `decision::parse_response`

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/llm/prompt.rs`
- Modify: `apps/desktop/src-tauri/src/agent/decision.rs`

- [ ] **Step 1: Implement the prompt helper**

Replace the stub in `apps/desktop/src-tauri/src/agent/llm/prompt.rs`:

```rust
use crate::agent::config::AgentConfig;
use crate::agent::executor::Position;
use crate::agent::signal::Signal;

pub fn system_prompt() -> String {
    r#"You are Mantic, an autonomous trading agent. You evaluate signals and make paper-trade decisions for the user.

You ALWAYS respond with a single JSON object matching this schema:
{
  "action": "buy" | "sell" | "skip",
  "size_sol": number (only for buy),
  "take_profit_pct": number (only for buy),
  "stop_loss_pct": number (only for buy),
  "position_id": string (only for sell),
  "reasoning_summary": string (<=200 chars),
  "reasoning_chain": string (your full thought process)
}

No prose outside the JSON. No markdown fences."#.to_string()
}

pub fn user_prompt(
    config: &AgentConfig,
    positions: &[Position],
    brain_context: &str,
    signal: &Signal,
) -> String {
    let positions_json = serde_json::to_string_pretty(positions).unwrap_or_else(|_| "[]".into());
    let payload_json = serde_json::to_string_pretty(&signal.payload).unwrap_or_else(|_| "{}".into());
    let tags = signal.context_tags.join(", ");
    format!(
        "Agent config:\n  Name: {name}\n  Max position size: {max_pos} SOL\n  Daily loss cap: {cap} SOL\n  Behavior notes: {overlay}\n\nCurrent paper positions:\n{positions_json}\n\nRecent brain memories relevant to this signal:\n{brain_context}\n\nNEW SIGNAL:\n  Token: {token}\n  Source: {source}\n  Context: {tags}\n  Payload: {payload_json}\n\nDecide your action.",
        name = config.name,
        max_pos = config.max_position_sol,
        cap = config.daily_loss_cap_sol,
        overlay = config.nl_overlay,
        token = signal.token_symbol,
        source = signal.source,
    )
}
```

- [ ] **Step 2: Implement decision parsing**

Replace the stub in `apps/desktop/src-tauri/src/agent/decision.rs`:

```rust
use crate::agent::config::AgentConfig;
use crate::agent::executor::Position;
use crate::agent::llm::{prompt, ChatRequest, Message};
use crate::agent::signal::Signal;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum DecisionOutcome {
    Buy {
        size_sol: f64,
        take_profit_pct: f64,
        stop_loss_pct: f64,
        reasoning_summary: String,
        reasoning_chain: String,
    },
    Sell {
        position_id: String,
        reasoning_summary: String,
        reasoning_chain: String,
    },
    Skip {
        reasoning_summary: String,
        reasoning_chain: String,
    },
}

pub fn build_prompt(
    config: &AgentConfig,
    positions: &[Position],
    brain_context: &str,
    signal: &Signal,
) -> ChatRequest {
    ChatRequest {
        system: prompt::system_prompt(),
        messages: vec![Message::User {
            content: prompt::user_prompt(config, positions, brain_context, signal),
        }],
        model: config.llm_model.clone(),
        max_tokens: config.max_tokens,
        temperature: config.temperature,
    }
}

/// Parse the LLM's textual response into a `DecisionOutcome`.
/// We strip leading/trailing whitespace and a single pair of markdown
/// fences if the model returns them despite the system prompt.
pub fn parse_response(raw: &str) -> Result<DecisionOutcome> {
    let trimmed = strip_fences(raw.trim());
    serde_json::from_str::<DecisionOutcome>(trimmed).map_err(|e| {
        AppError::LlmBackend(format!("llm json parse failed: {e}; raw={}", trimmed))
    })
}

fn strip_fences(s: &str) -> &str {
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::LlmBackendKind;

    fn cfg() -> AgentConfig {
        AgentConfig {
            id: "1".into(),
            name: "Test".into(),
            max_position_sol: 0.5,
            daily_loss_cap_sol: 2.0,
            nl_overlay: "be cautious".into(),
            strategy_template_id: "mock".into(),
            llm_backend: LlmBackendKind::AnthropicDirect,
            llm_model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            temperature: 0.0,
        }
    }

    #[test]
    fn build_prompt_includes_signal_token_and_overlay() {
        let req = build_prompt(&cfg(), &[], "no memories yet", &Signal::new_test("BONK"));
        let user = match &req.messages[0] {
            Message::User { content } => content,
            _ => panic!(),
        };
        assert!(user.contains("BONK"));
        assert!(user.contains("be cautious"));
        assert!(user.contains("no memories yet"));
        assert_eq!(req.model, "claude-sonnet-4-6");
    }

    #[test]
    fn parses_buy() {
        let raw = r#"{"action":"buy","size_sol":0.25,"take_profit_pct":50,"stop_loss_pct":20,"reasoning_summary":"strong signal","reasoning_chain":"longer thought"}"#;
        let d = parse_response(raw).unwrap();
        match d {
            DecisionOutcome::Buy { size_sol, take_profit_pct, .. } => {
                assert!((size_sol - 0.25).abs() < 1e-9);
                assert!((take_profit_pct - 50.0).abs() < 1e-9);
            }
            _ => panic!("expected buy"),
        }
    }

    #[test]
    fn parses_sell() {
        let raw = r#"{"action":"sell","position_id":"abc","reasoning_summary":"tp hit","reasoning_chain":"."}"#;
        match parse_response(raw).unwrap() {
            DecisionOutcome::Sell { position_id, .. } => assert_eq!(position_id, "abc"),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_skip() {
        let raw = r#"{"action":"skip","reasoning_summary":"low conviction","reasoning_chain":"."}"#;
        match parse_response(raw).unwrap() {
            DecisionOutcome::Skip { reasoning_summary, .. } => {
                assert_eq!(reasoning_summary, "low conviction");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn strips_markdown_fences() {
        let raw = "```json\n{\"action\":\"skip\",\"reasoning_summary\":\"x\",\"reasoning_chain\":\"y\"}\n```";
        assert!(parse_response(raw).is_ok());
    }

    #[test]
    fn malformed_json_returns_llm_backend_error() {
        let err = parse_response("not json").unwrap_err();
        assert!(matches!(err, AppError::LlmBackend(_)));
    }

    #[test]
    fn unknown_action_returns_error() {
        let err = parse_response(r#"{"action":"yolo","reasoning_summary":"","reasoning_chain":""}"#)
            .unwrap_err();
        assert!(matches!(err, AppError::LlmBackend(_)));
    }
}
```

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::decision agent::llm::prompt
```

Expected: 7 tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/decision.rs apps/desktop/src-tauri/src/agent/llm/prompt.rs
git commit -m "feat(agent): prompt builder + DecisionOutcome parsing"
```

---

## Task 10: `Agent` task — full lifecycle

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/agent.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`

- [ ] **Step 1: Implement the `Agent` struct**

Replace the stub in `apps/desktop/src-tauri/src/agent/agent.rs`:

```rust
use crate::agent::config::AgentConfig;
use crate::agent::decision::{self, DecisionOutcome};
use crate::agent::executor::PaperExecutor;
use crate::agent::llm::LlmBackend;
use crate::agent::signal::Signal;
use crate::agent::state::{AgentState, RunStep};
use crate::brainctl_client::BrainctlClient;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

pub struct Agent {
    pub id: String,
    pub config: Arc<RwLock<AgentConfig>>,
    pub state: Arc<RwLock<AgentState>>,
    signal_rx: Mutex<Option<mpsc::Receiver<Signal>>>,
    signal_tx: mpsc::Sender<Signal>,
    llm: Arc<dyn LlmBackend>,
    executor: Arc<PaperExecutor>,
    brainctl: Arc<BrainctlClient>,
}

impl Agent {
    pub fn new(
        config: AgentConfig,
        llm: Arc<dyn LlmBackend>,
        executor: Arc<PaperExecutor>,
        brainctl: Arc<BrainctlClient>,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(32);
        Arc::new(Self {
            id: config.id.clone(),
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(AgentState::Idle)),
            signal_rx: Mutex::new(Some(rx)),
            signal_tx: tx,
            llm,
            executor,
            brainctl,
        })
    }

    pub fn signal_sender(&self) -> mpsc::Sender<Signal> {
        self.signal_tx.clone()
    }

    pub async fn current_state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    pub async fn set_state(&self, new: AgentState) {
        *self.state.write().await = new;
    }

    /// Long-running task body. Pulls signals off the channel and drives
    /// the full per-signal lifecycle. Returns when the sender is dropped.
    pub async fn run(self: Arc<Self>) {
        let mut rx = match self.signal_rx.lock().await.take() {
            Some(r) => r,
            None => return, // run() was called twice
        };
        while let Some(signal) = rx.recv().await {
            if let Err(e) = self.handle_signal(signal).await {
                self.set_state(AgentState::Error { reason: e.to_string() }).await;
            } else if matches!(*self.state.read().await, AgentState::Running { .. }) {
                self.set_state(AgentState::Armed).await;
            }
        }
    }

    async fn handle_signal(&self, signal: Signal) -> Result<()> {
        // require armed before processing
        self.state.read().await.require_armed()?;

        // 1. orienting
        self.set_state(AgentState::Running { step: RunStep::Orienting }).await;
        let cfg = self.config.read().await.clone();
        let brain_ctx = self.brainctl
            .agent_orient(Some("mantic"), Some(&signal.token_symbol))
            .await
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "[]".into());

        let positions = self.executor.list_for(&self.id);

        // 2. calling llm
        self.set_state(AgentState::Running { step: RunStep::CallingLlm }).await;
        let req = decision::build_prompt(&cfg, &positions, &brain_ctx, &signal);
        let resp = self.llm.chat(req).await?;
        let outcome = decision::parse_response(&resp.content)?;

        // 3. executing
        self.set_state(AgentState::Running { step: RunStep::Executing }).await;
        match &outcome {
            DecisionOutcome::Buy {
                size_sol, take_profit_pct, stop_loss_pct,
                reasoning_summary, ..
            } => {
                // synthetic entry price = 1.0 SOL/token for v1 (no oracle yet)
                let pos = self.executor.open(
                    &self.id, &signal.token_symbol, *size_sol, 1.0,
                    *take_profit_pct, *stop_loss_pct,
                )?;
                let record = serde_json::json!({
                    "action": "buy", "position": pos, "signal_id": signal.id,
                });
                let _ = self.brainctl
                    .event_add("result", &record.to_string(), Some(0.5))
                    .await;
                let _ = self.brainctl
                    .decision_add(
                        &format!("Decision {}", signal.id),
                        reasoning_summary,
                        Some("mantic"),
                    )
                    .await;
            }
            DecisionOutcome::Sell { position_id, reasoning_summary, .. } => {
                // synthetic exit price = 1.5 SOL/token for v1 (no oracle yet) so sells visibly resolve
                let res = self.executor.close(position_id, 1.5)?;
                let importance = (res.pnl_pct.abs() / 100.0).clamp(0.3, 1.0);
                let record = serde_json::json!({
                    "action": "sell", "trade": res, "signal_id": signal.id,
                });
                let _ = self.brainctl
                    .event_add("result", &record.to_string(), Some(importance))
                    .await;
                let _ = self.brainctl
                    .decision_add(
                        &format!("Decision {}", signal.id),
                        reasoning_summary,
                        Some("mantic"),
                    )
                    .await;
            }
            DecisionOutcome::Skip { reasoning_summary, .. } => {
                let _ = self.brainctl
                    .event_add(
                        "observation",
                        &format!("skipped: {reasoning_summary}"),
                        Some(0.3),
                    )
                    .await;
            }
        }

        // 4. logging full reasoning chain
        self.set_state(AgentState::Running { step: RunStep::Logging }).await;
        let chain = match &outcome {
            DecisionOutcome::Buy { reasoning_chain, .. }
            | DecisionOutcome::Sell { reasoning_chain, .. }
            | DecisionOutcome::Skip { reasoning_chain, .. } => reasoning_chain.clone(),
        };
        let _ = self.brainctl
            .event_add("decision", &chain, Some(0.7))
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::LlmBackendKind;
    use crate::agent::llm::{ChatRequest, ChatResponse, TokenUsage};
    use async_trait::async_trait;

    struct FakeLlm { reply: String }

    #[async_trait]
    impl LlmBackend for FakeLlm {
        async fn chat(&self, _r: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                content: self.reply.clone(),
                stop_reason: "end_turn".into(),
                usage: TokenUsage::default(),
            })
        }
    }

    fn agent_with_reply(reply: &str) -> Arc<Agent> {
        let cfg = AgentConfig {
            id: "agent-test".into(),
            name: "T".into(),
            max_position_sol: 0.5,
            daily_loss_cap_sol: 2.0,
            nl_overlay: "".into(),
            strategy_template_id: "mock".into(),
            llm_backend: LlmBackendKind::AnthropicDirect,
            llm_model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let llm = Arc::new(FakeLlm { reply: reply.into() });
        let exec = Arc::new(PaperExecutor::new());
        // Note: BrainctlClient is constructed but the test never spawns the
        // subprocess — agent_orient/event_add/decision_add will fail; that's
        // OK because we ignore-via-`let _ =` in the agent body.
        let brainctl = Arc::new(BrainctlClient::new(
            std::path::PathBuf::from("/nonexistent/brainctl-mcp"),
            std::path::PathBuf::from("/nonexistent/brain.db"),
            "agent-test",
        ));
        Agent::new(cfg, llm, exec, brainctl)
    }

    #[tokio::test]
    async fn dispatch_when_idle_errors() {
        let agent = agent_with_reply(r#"{"action":"skip","reasoning_summary":"x","reasoning_chain":"y"}"#);
        // state is Idle by default
        let err = agent.handle_signal(Signal::new_test("BONK")).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::AgentInvalidState(_)));
    }

    #[tokio::test]
    async fn buy_decision_opens_position() {
        let agent = agent_with_reply(
            r#"{"action":"buy","size_sol":0.25,"take_profit_pct":50,"stop_loss_pct":20,"reasoning_summary":"x","reasoning_chain":"y"}"#
        );
        agent.set_state(AgentState::Armed).await;
        agent.handle_signal(Signal::new_test("BONK")).await.unwrap();
        assert_eq!(agent.executor.list_for("agent-test").len(), 1);
    }

    #[tokio::test]
    async fn skip_decision_opens_no_position() {
        let agent = agent_with_reply(
            r#"{"action":"skip","reasoning_summary":"x","reasoning_chain":"y"}"#
        );
        agent.set_state(AgentState::Armed).await;
        agent.handle_signal(Signal::new_test("BONK")).await.unwrap();
        assert!(agent.executor.list_for("agent-test").is_empty());
    }

    #[tokio::test]
    async fn malformed_llm_returns_llm_backend_error() {
        let agent = agent_with_reply("not json");
        agent.set_state(AgentState::Armed).await;
        let err = agent.handle_signal(Signal::new_test("BONK")).await.unwrap_err();
        assert!(matches!(err, crate::error::AppError::LlmBackend(_)));
    }
}
```

- [ ] **Step 2: Re-export from mod.rs**

In `apps/desktop/src-tauri/src/agent/mod.rs`, add:

```rust
pub use agent::Agent;
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::agent
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/agent.rs apps/desktop/src-tauri/src/agent/mod.rs
git commit -m "feat(agent): Agent task body with full signal lifecycle"
```

---

## Task 11: `AgentRuntime` — fleet management

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`

- [ ] **Step 1: Implement the runtime**

Replace the stub in `apps/desktop/src-tauri/src/agent/runtime.rs`:

```rust
use crate::agent::agent::Agent;
use crate::agent::config::{AgentConfig, AgentDetails, AgentSummary, LlmBackendKind};
use crate::agent::executor::PaperExecutor;
use crate::agent::llm::anthropic_direct::AnthropicDirect;
use crate::agent::llm::mantic_proxy::ManticProxy;
use crate::agent::llm::LlmBackend;
use crate::agent::signal::Signal;
use crate::brainctl_client::BrainctlClient;
use crate::error::{AppError, Result};
use crate::license::LlmKeyStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AgentRuntime {
    agents: Arc<Mutex<HashMap<String, Arc<Agent>>>>,
    executor: Arc<PaperExecutor>,
    brainctl: Arc<BrainctlClient>,
    llm_keys: Arc<LlmKeyStore>,
}

impl AgentRuntime {
    pub fn new(brainctl: Arc<BrainctlClient>) -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            executor: Arc::new(PaperExecutor::new()),
            brainctl,
            llm_keys: Arc::new(LlmKeyStore::new()),
        }
    }

    pub fn brainctl(&self) -> Arc<BrainctlClient> { self.brainctl.clone() }
    pub fn llm_keys(&self) -> Arc<LlmKeyStore> { self.llm_keys.clone() }

    pub async fn spawn(&self, config: AgentConfig) -> Result<AgentSummary> {
        config.validate()?;
        let llm = self.build_backend(&config)?;
        let agent = Agent::new(config.clone(), llm, self.executor.clone(), self.brainctl.clone());
        let agent_for_task = agent.clone();
        tokio::spawn(async move { agent_for_task.run().await });
        self.agents.lock().await.insert(config.id.clone(), agent.clone());
        Ok(AgentSummary {
            id: config.id.clone(),
            name: config.name.clone(),
            state: agent.current_state().await,
            has_llm_key: self.has_llm_key(&config.id).await,
        })
    }

    fn build_backend(&self, config: &AgentConfig) -> Result<Arc<dyn LlmBackend>> {
        match config.llm_backend {
            LlmBackendKind::AnthropicDirect => {
                let key = self.llm_keys.load(&config.id)?.unwrap_or_default();
                // Even an empty key produces a backend that will 401; we'd
                // rather surface that than refuse to construct the agent.
                Ok(Arc::new(AnthropicDirect::new(key)))
            }
            LlmBackendKind::ManticProxy => Ok(Arc::new(ManticProxy::new(""))),
        }
    }

    pub async fn list(&self) -> Vec<AgentSummary> {
        let mut out = Vec::new();
        for (id, a) in self.agents.lock().await.iter() {
            let cfg = a.config.read().await;
            out.push(AgentSummary {
                id: id.clone(),
                name: cfg.name.clone(),
                state: a.current_state().await,
                has_llm_key: self.has_llm_key(id).await,
            });
        }
        out
    }

    pub async fn get(&self, id: &str) -> Result<AgentDetails> {
        let agents = self.agents.lock().await;
        let a = agents.get(id).ok_or_else(|| AppError::AgentNotFound(id.into()))?;
        let cfg = a.config.read().await.clone();
        Ok(AgentDetails {
            state: a.current_state().await,
            has_llm_key: self.has_llm_key(id).await,
            open_position_ids: self.executor.list_for(id).into_iter().map(|p| p.id).collect(),
            config: cfg,
        })
    }

    pub async fn arm(&self, id: &str) -> Result<AgentSummary> {
        let agents = self.agents.lock().await;
        let a = agents.get(id).ok_or_else(|| AppError::AgentNotFound(id.into()))?;
        let cur = a.current_state().await;
        if !cur.can_arm() {
            return Err(AppError::AgentInvalidState(format!("cannot arm from {cur:?}")));
        }
        a.set_state(crate::agent::state::AgentState::Armed).await;
        Ok(AgentSummary {
            id: id.into(),
            name: a.config.read().await.name.clone(),
            state: a.current_state().await,
            has_llm_key: self.has_llm_key(id).await,
        })
    }

    pub async fn pause(&self, id: &str) -> Result<AgentSummary> {
        let agents = self.agents.lock().await;
        let a = agents.get(id).ok_or_else(|| AppError::AgentNotFound(id.into()))?;
        let cur = a.current_state().await;
        if !cur.can_pause() {
            return Err(AppError::AgentInvalidState(format!("cannot pause from {cur:?}")));
        }
        a.set_state(crate::agent::state::AgentState::Paused).await;
        Ok(AgentSummary {
            id: id.into(),
            name: a.config.read().await.name.clone(),
            state: a.current_state().await,
            has_llm_key: self.has_llm_key(id).await,
        })
    }

    pub async fn kill(&self, id: &str) -> Result<()> {
        let agent = self.agents.lock().await.remove(id);
        if agent.is_none() {
            return Err(AppError::AgentNotFound(id.into()));
        }
        // Dropping the Agent's signal_tx clones causes the receiver loop to exit
        // eventually; we don't await it. Best-effort key wipe.
        let _ = self.llm_keys.clear(id);
        Ok(())
    }

    pub async fn dispatch(&self, id: &str, signal: Signal) -> Result<()> {
        let agents = self.agents.lock().await;
        let a = agents.get(id).ok_or_else(|| AppError::AgentNotFound(id.into()))?;
        a.current_state().await.require_armed()?;
        a.signal_sender()
            .send(signal)
            .await
            .map_err(|e| AppError::AgentInvalidState(format!("send failed: {e}")))
    }

    pub async fn set_llm_key(&self, id: &str, api_key: &str) -> Result<()> {
        // Validate agent exists
        let agents = self.agents.lock().await;
        if !agents.contains_key(id) {
            return Err(AppError::AgentNotFound(id.into()));
        }
        drop(agents);
        self.llm_keys.save(id, api_key)
    }

    async fn has_llm_key(&self, id: &str) -> bool {
        self.llm_keys.load(id).map(|o| o.is_some()).unwrap_or(false)
    }

    /// Test-only access to the executor for unit tests + integration tests.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn executor(&self) -> Arc<PaperExecutor> { self.executor.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::LlmBackendKind;

    fn cfg(id: &str, name: &str) -> AgentConfig {
        AgentConfig {
            id: id.into(),
            name: name.into(),
            max_position_sol: 0.5,
            daily_loss_cap_sol: 2.0,
            nl_overlay: "".into(),
            strategy_template_id: "mock".into(),
            llm_backend: LlmBackendKind::AnthropicDirect,
            llm_model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            temperature: 0.0,
        }
    }

    fn rt() -> AgentRuntime {
        let brainctl = Arc::new(BrainctlClient::new(
            std::path::PathBuf::from("/nonexistent/brainctl-mcp"),
            std::path::PathBuf::from("/nonexistent/brain.db"),
            "test",
        ));
        AgentRuntime::new(brainctl)
    }

    #[tokio::test]
    async fn spawn_then_list_returns_one() {
        let r = rt();
        r.spawn(cfg("a1", "Alpha")).await.unwrap();
        let list = r.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Alpha");
    }

    #[tokio::test]
    async fn arm_transitions_state() {
        let r = rt();
        r.spawn(cfg("a1", "Alpha")).await.unwrap();
        let sum = r.arm("a1").await.unwrap();
        assert_eq!(sum.state, crate::agent::state::AgentState::Armed);
    }

    #[tokio::test]
    async fn dispatch_when_idle_errors() {
        let r = rt();
        r.spawn(cfg("a1", "Alpha")).await.unwrap();
        // not armed
        let err = r.dispatch("a1", Signal::new_test("BONK")).await.unwrap_err();
        assert!(matches!(err, AppError::AgentInvalidState(_)));
    }

    #[tokio::test]
    async fn get_unknown_errors() {
        let r = rt();
        assert!(matches!(r.get("nope").await.unwrap_err(), AppError::AgentNotFound(_)));
    }

    #[tokio::test]
    async fn kill_removes_from_list() {
        let r = rt();
        r.spawn(cfg("a1", "Alpha")).await.unwrap();
        r.kill("a1").await.unwrap();
        assert!(r.list().await.is_empty());
    }

    #[tokio::test]
    async fn set_llm_key_for_unknown_errors() {
        let r = rt();
        let err = r.set_llm_key("nope", "sk-x").await.unwrap_err();
        assert!(matches!(err, AppError::AgentNotFound(_)));
    }
}
```

- [ ] **Step 2: Re-export from mod.rs**

In `apps/desktop/src-tauri/src/agent/mod.rs`, add:

```rust
pub use runtime::AgentRuntime;
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::runtime
```

Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/runtime.rs apps/desktop/src-tauri/src/agent/mod.rs
git commit -m "feat(agent): AgentRuntime fleet manager with spawn/arm/pause/kill/dispatch"
```

---

## Task 12: Brain persistence wiring

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/runtime.rs`

The runtime currently spawns agents in-memory only. We add: on spawn, write the agent config + register with brainctl. The default-agent seed (Task 19) needs this to survive across launches conceptually (v1 doesn't actually rehydrate yet — that's deferred — but the config write is the seed that #11's polish will use).

- [ ] **Step 1: Add a `persist_new_agent` helper**

In `apps/desktop/src-tauri/src/agent/runtime.rs`, before the `#[cfg(test)]` block, add:

```rust
impl AgentRuntime {
    async fn persist_new_agent(&self, config: &AgentConfig) -> Result<()> {
        // Register the agent with brainctl's internal accounting.
        let _ = self
            .brainctl
            .agent_register(&config.id, &config.name, Some("trading-agent"))
            .await;

        // Write the config blob as a memory in scope:agent:<id>.
        let blob = serde_json::to_string(config)
            .map_err(|e| AppError::LlmBackend(format!("config serialize: {e}")))?;
        let scope = format!("agent:{}", config.id);
        let _ = self
            .brainctl
            .memory_add(&blob, "convention", Some(&scope), Some("agent-config,v1"))
            .await;
        Ok(())
    }
}
```

- [ ] **Step 2: Wire it into `spawn`**

In the existing `pub async fn spawn`, immediately after `config.validate()?;`, add:

```rust
        // Best-effort persistence; we accept that brainctl may be unavailable
        // during unit tests and proceed with in-memory state regardless.
        let _ = self.persist_new_agent(&config).await;
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test agent::runtime
```

Expected: existing 6 tests still pass (brainctl writes are best-effort and silently fail in tests against the nonexistent binary).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/agent/runtime.rs
git commit -m "feat(agent): persist agent config + registration to brain.db on spawn"
```

---

## Task 13: Tauri commands (8 commands)

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs`

- [ ] **Step 1: Add imports**

At the top of `apps/desktop/src-tauri/src/commands.rs`, add:

```rust
use crate::agent::{
    config::{AgentConfigInput, AgentDetails, AgentSummary},
    signal::Signal,
    AgentRuntime,
};
```

- [ ] **Step 2: Add the 8 commands**

Append to the end of `apps/desktop/src-tauri/src/commands.rs`:

```rust
// --- Agent runtime commands ---

#[tauri::command]
pub async fn agent_create(
    config: AgentConfigInput,
    runtime: State<'_, Arc<AgentRuntime>>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<AgentSummary> {
    // Use brainctl entity_create to mint a stable id, fall back to a generated short id.
    let id = match brainctl
        .entity_create(&config.name, "agent", Some("project:mantic"))
        .await
    {
        Ok(v) => {
            // brainctl returns either an integer entity_id or a "name (id)" envelope; we accept
            // either path and stringify what we find.
            v.get("entity_id")
                .or_else(|| v.get("id"))
                .map(|n| n.to_string())
                .unwrap_or_else(crate::agent::signal::short_id)
        }
        Err(_) => crate::agent::signal::short_id(),
    };
    let cfg = config.into_config_with_id(id);
    runtime.spawn(cfg).await
}

#[tauri::command]
pub async fn agent_list(runtime: State<'_, Arc<AgentRuntime>>) -> Result<Vec<AgentSummary>> {
    Ok(runtime.list().await)
}

#[tauri::command]
pub async fn agent_get(id: String, runtime: State<'_, Arc<AgentRuntime>>) -> Result<AgentDetails> {
    runtime.get(&id).await
}

#[tauri::command]
pub async fn agent_arm(id: String, runtime: State<'_, Arc<AgentRuntime>>) -> Result<AgentSummary> {
    runtime.arm(&id).await
}

#[tauri::command]
pub async fn agent_pause(id: String, runtime: State<'_, Arc<AgentRuntime>>) -> Result<AgentSummary> {
    runtime.pause(&id).await
}

#[tauri::command]
pub async fn agent_kill(id: String, runtime: State<'_, Arc<AgentRuntime>>) -> Result<()> {
    runtime.kill(&id).await
}

#[tauri::command]
pub async fn agent_fire_test_signal(
    id: String,
    signal: Signal,
    runtime: State<'_, Arc<AgentRuntime>>,
) -> Result<()> {
    runtime.dispatch(&id, signal).await
}

#[tauri::command]
pub async fn agent_set_llm_key(
    id: String,
    api_key: String,
    runtime: State<'_, Arc<AgentRuntime>>,
) -> Result<()> {
    runtime.set_llm_key(&id, &api_key).await
}
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
```

Expected: compiles. (Commands aren't invocable yet — that's Task 14.)

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/commands.rs
git commit -m "feat(agent): 8 tauri commands (create/list/get/arm/pause/kill/fire/set_key)"
```

---

## Task 14: AppState wiring + setup callback

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add `agent_runtime` to `AppState`**

In `apps/desktop/src-tauri/src/state.rs`, modify the imports and struct:

```rust
use crate::agent::AgentRuntime;
```

And in the `AppState` struct:

```rust
pub struct AppState {
    pub keyring: Arc<KeyringStore>,
    pub brain: Arc<BrainDb>,
    pub brainctl: Arc<BrainctlClient>,
    pub wallet: Arc<WalletStore>,
    pub agent_runtime: Arc<AgentRuntime>,
}
```

Modify `AppState::new` to construct the runtime after the brainctl is built:

```rust
        let wallet = Arc::new(WalletStore::new());
        let agent_runtime = Arc::new(AgentRuntime::new(brainctl.clone()));
        Ok(Self {
            keyring,
            brain,
            brainctl,
            wallet,
            agent_runtime,
        })
```

- [ ] **Step 2: Register in `lib.rs`**

In `apps/desktop/src-tauri/src/lib.rs`, in the `setup` callback after `app.manage(state.wallet.clone());` add:

```rust
            app.manage(state.agent_runtime.clone());
```

And in the `invoke_handler!` macro, append the 8 new commands (after `commands::wallet_sign_message`):

```rust
            commands::agent_create,
            commands::agent_list,
            commands::agent_get,
            commands::agent_arm,
            commands::agent_pause,
            commands::agent_kill,
            commands::agent_fire_test_signal,
            commands::agent_set_llm_key,
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
cargo test
```

Expected: full library compile; all existing tests still pass plus new agent tests (running total ~44 baseline + ~35 new = ~79).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/state.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(agent): wire AgentRuntime into AppState + register 8 commands"
```

---

## Task 15: TS bridge — typed wrappers for the 8 commands

**Files:**
- Modify: `apps/desktop/src/lib/tauri-bridge.ts`
- Create: `apps/desktop/src/lib/tauri-bridge.agent.test.ts`

- [ ] **Step 1: Append types and wrappers**

Append to `apps/desktop/src/lib/tauri-bridge.ts`:

```typescript
// ---- Agent runtime ----

export type LlmBackendKind = "anthropic-direct" | "mantic-proxy";

export interface AgentConfigInput {
  name: string;
  max_position_sol: number;
  daily_loss_cap_sol: number;
  nl_overlay: string;
  strategy_template_id: string;
  llm_backend: LlmBackendKind;
  llm_model: string;
  max_tokens: number;
  temperature: number;
}

export interface AgentConfig extends AgentConfigInput {
  id: string;
}

export type RunStep = "orienting" | "calling_llm" | "executing" | "logging";

export type AgentState =
  | { kind: "idle" }
  | { kind: "armed" }
  | { kind: "running"; step: RunStep }
  | { kind: "paused" }
  | { kind: "error"; reason: string };

export interface AgentSummary {
  id: string;
  name: string;
  state: AgentState;
  has_llm_key: boolean;
}

export interface AgentDetails {
  config: AgentConfig;
  state: AgentState;
  has_llm_key: boolean;
  open_position_ids: string[];
}

export interface Signal {
  id: string;
  token_symbol: string;
  source: string;
  context_tags: string[];
  payload: unknown;
}

export function agentCreate(config: AgentConfigInput): Promise<AgentSummary> {
  return invoke<AgentSummary>("agent_create", { config });
}

export function agentList(): Promise<AgentSummary[]> {
  return invoke<AgentSummary[]>("agent_list");
}

export function agentGet(id: string): Promise<AgentDetails> {
  return invoke<AgentDetails>("agent_get", { id });
}

export function agentArm(id: string): Promise<AgentSummary> {
  return invoke<AgentSummary>("agent_arm", { id });
}

export function agentPause(id: string): Promise<AgentSummary> {
  return invoke<AgentSummary>("agent_pause", { id });
}

export function agentKill(id: string): Promise<void> {
  return invoke<void>("agent_kill", { id });
}

export function agentFireTestSignal(id: string, signal: Signal): Promise<void> {
  return invoke<void>("agent_fire_test_signal", { id, signal });
}

export function agentSetLlmKey(id: string, apiKey: string): Promise<void> {
  return invoke<void>("agent_set_llm_key", { id, apiKey });
}
```

- [ ] **Step 2: Create wrapper tests**

Create `apps/desktop/src/lib/tauri-bridge.agent.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  agentCreate, agentList, agentGet, agentArm, agentPause, agentKill,
  agentFireTestSignal, agentSetLlmKey,
} from "./tauri-bridge";

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());

describe("agent bridge", () => {
  it("agentCreate passes config", async () => {
    invokeMock.mockResolvedValueOnce({ id: "1", name: "x", state: { kind: "idle" }, has_llm_key: false });
    const r = await agentCreate({
      name: "x", max_position_sol: 0.5, daily_loss_cap_sol: 2,
      nl_overlay: "", strategy_template_id: "mock",
      llm_backend: "anthropic-direct", llm_model: "claude-sonnet-4-6",
      max_tokens: 1024, temperature: 0,
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_create", expect.objectContaining({
      config: expect.objectContaining({ name: "x" }),
    }));
    expect(r.id).toBe("1");
  });

  it("agentList returns array", async () => {
    invokeMock.mockResolvedValueOnce([]);
    expect(await agentList()).toEqual([]);
    expect(invokeMock).toHaveBeenCalledWith("agent_list");
  });

  it("agentGet uses id", async () => {
    invokeMock.mockResolvedValueOnce({ config: {}, state: { kind: "idle" }, has_llm_key: false, open_position_ids: [] });
    await agentGet("a1");
    expect(invokeMock).toHaveBeenCalledWith("agent_get", { id: "a1" });
  });

  it("agentArm/pause/kill use id", async () => {
    invokeMock.mockResolvedValue({ id: "a1", name: "x", state: { kind: "armed" }, has_llm_key: false });
    await agentArm("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_arm", { id: "a1" });
    await agentPause("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_pause", { id: "a1" });
    invokeMock.mockResolvedValueOnce(undefined as unknown as never);
    await agentKill("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_kill", { id: "a1" });
  });

  it("agentFireTestSignal passes signal", async () => {
    invokeMock.mockResolvedValueOnce(undefined as unknown as never);
    await agentFireTestSignal("a1", {
      id: "sig1", token_symbol: "BONK", source: "test", context_tags: [], payload: {},
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_fire_test_signal", {
      id: "a1",
      signal: expect.objectContaining({ token_symbol: "BONK" }),
    });
  });

  it("agentSetLlmKey passes apiKey", async () => {
    invokeMock.mockResolvedValueOnce(undefined as unknown as never);
    await agentSetLlmKey("a1", "sk-xxx");
    expect(invokeMock).toHaveBeenCalledWith("agent_set_llm_key", { id: "a1", apiKey: "sk-xxx" });
  });
});
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test
```

Expected: existing 34 frontend tests + 6 new wrapper tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/tauri-bridge.ts apps/desktop/src/lib/tauri-bridge.agent.test.ts
git commit -m "feat(agent): typed TS wrappers for 8 agent commands"
```

---

## Task 16: `Fleet.tsx` + `LiveTape.tsx`

**Files:**
- Create: `apps/desktop/src/components/LiveTape.tsx`
- Create: `apps/desktop/src/components/LiveTape.test.tsx`
- Create: `apps/desktop/src/routes/Fleet.tsx`
- Create: `apps/desktop/src/routes/Fleet.test.tsx`

- [ ] **Step 1: Create `LiveTape.tsx`**

Create `apps/desktop/src/components/LiveTape.tsx`:

```tsx
import { useEffect, useState } from "react";
import { recentEvents, EventSummary } from "../lib/tauri-bridge";

interface Props {
  pollMs?: number;
  limit?: number;
}

export default function LiveTape({ pollMs = 5000, limit = 50 }: Props) {
  const [events, setEvents] = useState<EventSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function tick() {
      try {
        const e = await recentEvents(limit);
        if (!cancelled) setEvents(e);
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    }
    tick();
    const handle = setInterval(tick, pollMs);
    return () => { cancelled = true; clearInterval(handle); };
  }, [pollMs, limit]);

  return (
    <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-medium">Live tape</h2>
        <span className="text-xs text-neutral-500">{events.length} event{events.length === 1 ? "" : "s"}</span>
      </header>
      {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      <ol className="mt-4 space-y-2">
        {events.length === 0 && !error && (
          <li className="text-sm text-neutral-500">No events yet. Fire a test signal to populate the tape.</li>
        )}
        {events.map((e) => (
          <li key={e.id} className="rounded-lg border border-neutral-800 bg-neutral-950 p-3 text-xs">
            <div className="flex items-baseline justify-between">
              <span className="font-mono uppercase text-neutral-400">{e.event_type}</span>
              <time className="text-neutral-600">
                {new Date(e.created_at * 1000).toLocaleTimeString()}
              </time>
            </div>
            <pre className="mt-1 whitespace-pre-wrap break-words text-neutral-200">{e.content}</pre>
          </li>
        ))}
      </ol>
    </section>
  );
}
```

- [ ] **Step 2: Create `LiveTape.test.tsx`**

Create `apps/desktop/src/components/LiveTape.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import LiveTape from "./LiveTape";

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());

describe("<LiveTape />", () => {
  it("renders empty state when no events", async () => {
    invokeMock.mockResolvedValue([]);
    render(<LiveTape pollMs={9999} />);
    expect(await screen.findByText(/no events yet/i)).toBeInTheDocument();
  });

  it("renders event rows", async () => {
    invokeMock.mockResolvedValue([
      { id: 1, event_type: "decision", content: "thought one", created_at: 1700000000 },
      { id: 2, event_type: "result", content: "trade record", created_at: 1700000050 },
    ]);
    render(<LiveTape pollMs={9999} />);
    await waitFor(() => expect(screen.getByText(/thought one/i)).toBeInTheDocument());
    expect(screen.getByText(/trade record/i)).toBeInTheDocument();
    expect(screen.getByText(/decision/i)).toBeInTheDocument();
  });

  it("surfaces fetch errors", async () => {
    invokeMock.mockRejectedValue("brain unavailable");
    render(<LiveTape pollMs={9999} />);
    expect(await screen.findByText(/brain unavailable/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Create `Fleet.tsx`**

Create `apps/desktop/src/routes/Fleet.tsx`:

```tsx
import { useEffect, useState } from "react";
import {
  agentList, agentArm, agentPause, agentKill, agentFireTestSignal,
  AgentSummary, Signal,
} from "../lib/tauri-bridge";
import LiveTape from "../components/LiveTape";
import AgentConfigForm from "./AgentConfig";

function randomSignalId(): string {
  return Math.random().toString(16).slice(2, 18).padStart(16, "0");
}

function makeTestSignal(token: string): Signal {
  return { id: randomSignalId(), token_symbol: token, source: "test", context_tags: ["debug"], payload: { test: true } };
}

function describeState(state: AgentSummary["state"]): string {
  switch (state.kind) {
    case "idle": return "Idle";
    case "armed": return "Armed";
    case "running": return `Running (${state.step})`;
    case "paused": return "Paused";
    case "error": return `Error: ${state.reason}`;
  }
}

interface Props {
  onOpenAccount: () => void;
}

export default function Fleet({ onOpenAccount }: Props) {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      setAgents(await agentList());
      setError(null);
    } catch (e) { setError(String(e)); }
  }

  useEffect(() => { refresh(); const i = setInterval(refresh, 3000); return () => clearInterval(i); }, []);

  async function withRefresh(fn: () => Promise<unknown>) {
    try { await fn(); await refresh(); }
    catch (e) { setError(String(e)); }
  }

  return (
    <div className="min-h-screen bg-neutral-950 p-8 text-neutral-100">
      <div className="mx-auto max-w-5xl space-y-8">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-semibold">Mantic</h1>
            <p className="mt-1 text-sm text-neutral-400">read the tape</p>
          </div>
          <button
            type="button"
            onClick={onOpenAccount}
            className="rounded-lg border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-800"
          >
            Account
          </button>
        </header>

        <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-medium">Fleet</h2>
            <button
              type="button"
              onClick={() => setCreating(true)}
              className="rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-medium hover:bg-blue-500"
            >
              Create Agent
            </button>
          </div>
          {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

          {agents.length === 0 ? (
            <p className="mt-4 text-sm text-neutral-500">
              No agents yet. The first-run default should appear shortly, or click Create Agent.
            </p>
          ) : (
            <ul className="mt-4 space-y-2">
              {agents.map((a) => (
                <li key={a.id} className="rounded-lg border border-neutral-800 bg-neutral-950 p-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="font-medium">{a.name}</h3>
                      <p className="text-xs text-neutral-500">id {a.id} · {describeState(a.state)}</p>
                      {!a.has_llm_key && (
                        <p className="text-xs text-amber-400">no LLM key set</p>
                      )}
                    </div>
                    <div className="flex gap-2">
                      <button
                        type="button"
                        disabled={a.state.kind === "armed" || a.state.kind === "running"}
                        onClick={() => withRefresh(() => agentArm(a.id))}
                        className="rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-medium hover:bg-emerald-500 disabled:bg-neutral-700"
                      >Arm</button>
                      <button
                        type="button"
                        disabled={a.state.kind !== "armed" && a.state.kind !== "running"}
                        onClick={() => withRefresh(() => agentPause(a.id))}
                        className="rounded-lg bg-yellow-600 px-3 py-1.5 text-xs font-medium hover:bg-yellow-500 disabled:bg-neutral-700"
                      >Pause</button>
                      <button
                        type="button"
                        disabled={a.state.kind !== "armed"}
                        onClick={() => withRefresh(() => agentFireTestSignal(a.id, makeTestSignal("BONK")))}
                        className="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium hover:bg-blue-500 disabled:bg-neutral-700"
                      >Fire test signal</button>
                      <button
                        type="button"
                        onClick={() => withRefresh(() => agentKill(a.id))}
                        className="rounded-lg border border-red-700 px-3 py-1.5 text-xs font-medium text-red-300 hover:bg-red-950"
                      >Kill</button>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>

        <LiveTape />

        {creating && (
          <AgentConfigForm
            onCancel={() => setCreating(false)}
            onCreated={async () => { setCreating(false); await refresh(); }}
          />
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create `Fleet.test.tsx`**

Create `apps/desktop/src/routes/Fleet.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import Fleet from "./Fleet";

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());

describe("<Fleet />", () => {
  it("renders empty state when no agents", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") return [];
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    expect(await screen.findByText(/no agents yet/i)).toBeInTheDocument();
  });

  it("renders one agent with state and arm button", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") {
        return [{ id: "a1", name: "Default Paper Agent", state: { kind: "idle" }, has_llm_key: false }];
      }
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    expect(await screen.findByText(/Default Paper Agent/)).toBeInTheDocument();
    expect(screen.getByText(/Idle/)).toBeInTheDocument();
    expect(screen.getByText(/no LLM key set/i)).toBeInTheDocument();
  });

  it("opens Create modal when button clicked", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") return [];
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    await waitFor(() => screen.getByRole("button", { name: /create agent/i }));
    fireEvent.click(screen.getByRole("button", { name: /create agent/i }));
    expect(await screen.findByText(/New agent|Agent configuration/i)).toBeInTheDocument();
  });

  it("invokes agent_arm on Arm click", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") return [{ id: "a1", name: "A", state: { kind: "idle" }, has_llm_key: true }];
      if (cmd === "recent_events") return [];
      if (cmd === "agent_arm") return { id: "a1", name: "A", state: { kind: "armed" }, has_llm_key: true };
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    const btn = await screen.findByRole("button", { name: /arm/i });
    fireEvent.click(btn);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("agent_arm", { id: "a1" });
    });
  });
});
```

- [ ] **Step 5: Verify**

This task references `AgentConfig.tsx` (Task 17). To keep the test runnable in this task, temporarily inline a placeholder export at the top of `apps/desktop/src/routes/AgentConfig.tsx`:

```tsx
export default function AgentConfigForm(_: { onCancel: () => void; onCreated: () => void }) {
  return <div>Agent configuration</div>;
}
```

Then run:

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test Fleet LiveTape
```

Expected: 7 new tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/components/LiveTape.tsx apps/desktop/src/components/LiveTape.test.tsx \
        apps/desktop/src/routes/Fleet.tsx apps/desktop/src/routes/Fleet.test.tsx \
        apps/desktop/src/routes/AgentConfig.tsx
git commit -m "feat(agent): Fleet route + LiveTape component with tests"
```

---

## Task 17: `AgentConfig.tsx` form

**Files:**
- Modify: `apps/desktop/src/routes/AgentConfig.tsx`
- Create: `apps/desktop/src/routes/AgentConfig.test.tsx`

- [ ] **Step 1: Replace the placeholder with a real form**

Replace `apps/desktop/src/routes/AgentConfig.tsx`:

```tsx
import { useState } from "react";
import { agentCreate, agentSetLlmKey, AgentConfigInput, LlmBackendKind } from "../lib/tauri-bridge";

interface Props {
  onCancel: () => void;
  onCreated: () => void;
}

const DEFAULT: AgentConfigInput = {
  name: "",
  max_position_sol: 0.5,
  daily_loss_cap_sol: 2.0,
  nl_overlay: "",
  strategy_template_id: "mock",
  llm_backend: "anthropic-direct",
  llm_model: "claude-sonnet-4-6",
  max_tokens: 1024,
  temperature: 0.0,
};

export default function AgentConfigForm({ onCancel, onCreated }: Props) {
  const [form, setForm] = useState<AgentConfigInput>(DEFAULT);
  const [apiKey, setApiKey] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function update<K extends keyof AgentConfigInput>(key: K, value: AgentConfigInput[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!form.name.trim()) { setError("Name is required"); return; }
    if (form.max_position_sol <= 0) { setError("Max position size must be > 0"); return; }
    setSubmitting(true);
    setError(null);
    try {
      const summary = await agentCreate(form);
      if (form.llm_backend === "anthropic-direct" && apiKey.trim()) {
        await agentSetLlmKey(summary.id, apiKey.trim());
      }
      onCreated();
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-6">
      <form onSubmit={submit} className="w-full max-w-xl space-y-4 rounded-2xl bg-neutral-900 p-6 shadow-xl">
        <h2 className="text-lg font-medium">New agent</h2>
        {error && <p className="text-sm text-red-400">{error}</p>}

        <label className="block">
          <span className="text-sm text-neutral-300">Name</span>
          <input
            type="text" value={form.name}
            onChange={(e) => update("name", e.target.value)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            required
          />
        </label>

        <div className="grid grid-cols-2 gap-4">
          <label className="block">
            <span className="text-sm text-neutral-300">Max position (SOL)</span>
            <input
              type="number" step="0.01" min="0.01" value={form.max_position_sol}
              onChange={(e) => update("max_position_sol", parseFloat(e.target.value))}
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            />
          </label>
          <label className="block">
            <span className="text-sm text-neutral-300">Daily loss cap (SOL)</span>
            <input
              type="number" step="0.1" min="0" value={form.daily_loss_cap_sol}
              onChange={(e) => update("daily_loss_cap_sol", parseFloat(e.target.value))}
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            />
          </label>
        </div>

        <label className="block">
          <span className="text-sm text-neutral-300">Behavior notes (NL overlay)</span>
          <textarea
            value={form.nl_overlay} onChange={(e) => update("nl_overlay", e.target.value)}
            rows={3}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            placeholder="e.g. be aggressive on KOL signals, skip if context contains 'rug'"
          />
        </label>

        <label className="block">
          <span className="text-sm text-neutral-300">LLM backend</span>
          <select
            value={form.llm_backend}
            onChange={(e) => update("llm_backend", e.target.value as LlmBackendKind)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
          >
            <option value="anthropic-direct">Anthropic (BYO key)</option>
            <option value="mantic-proxy">Mantic proxy (coming soon)</option>
          </select>
        </label>

        {form.llm_backend === "anthropic-direct" && (
          <label className="block">
            <span className="text-sm text-neutral-300">Anthropic API key</span>
            <input
              type="password" value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm font-mono"
              placeholder="sk-ant-..."
              autoComplete="off"
            />
          </label>
        )}

        <label className="block">
          <span className="text-sm text-neutral-300">Strategy template</span>
          <select
            value={form.strategy_template_id}
            onChange={(e) => update("strategy_template_id", e.target.value)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
          >
            <option value="mock">Mock — accepts any signal</option>
          </select>
        </label>

        <footer className="flex justify-end gap-2 pt-2">
          <button
            type="button" onClick={onCancel}
            className="rounded-lg border border-neutral-700 px-3 py-1.5 text-sm hover:bg-neutral-800"
          >Cancel</button>
          <button
            type="submit" disabled={submitting}
            className="rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-medium hover:bg-blue-500 disabled:bg-neutral-700"
          >{submitting ? "Creating…" : "Create"}</button>
        </footer>
      </form>
    </div>
  );
}
```

- [ ] **Step 2: Create the test**

Create `apps/desktop/src/routes/AgentConfig.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import AgentConfigForm from "./AgentConfig";

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());

describe("<AgentConfigForm />", () => {
  it("blocks submit when name is empty", async () => {
    const onCreated = vi.fn();
    render(<AgentConfigForm onCancel={() => {}} onCreated={onCreated} />);
    fireEvent.click(screen.getByRole("button", { name: /create/i }));
    // HTML5 required prevents form submission; agent_create should not be invoked.
    await waitFor(() => expect(invokeMock).not.toHaveBeenCalledWith("agent_create", expect.anything()));
    expect(onCreated).not.toHaveBeenCalled();
  });

  it("invokes agent_create then agent_set_llm_key when key provided", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_create") return { id: "a1", name: "X", state: { kind: "idle" }, has_llm_key: false };
      if (cmd === "agent_set_llm_key") return undefined;
      throw new Error("unexpected: " + cmd);
    });
    const onCreated = vi.fn();
    render(<AgentConfigForm onCancel={() => {}} onCreated={onCreated} />);

    fireEvent.change(screen.getByLabelText(/^name$/i), { target: { value: "Bonk Hunter" } });
    fireEvent.change(screen.getByLabelText(/anthropic api key/i), { target: { value: "sk-test" } });
    fireEvent.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("agent_create", expect.objectContaining({
      config: expect.objectContaining({ name: "Bonk Hunter" }),
    })));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("agent_set_llm_key", { id: "a1", apiKey: "sk-test" }));
    expect(onCreated).toHaveBeenCalled();
  });

  it("Cancel button calls onCancel", () => {
    const onCancel = vi.fn();
    render(<AgentConfigForm onCancel={onCancel} onCreated={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("does not show API key field when mantic-proxy chosen", () => {
    render(<AgentConfigForm onCancel={() => {}} onCreated={() => {}} />);
    fireEvent.change(screen.getByLabelText(/llm backend/i), { target: { value: "mantic-proxy" } });
    expect(screen.queryByLabelText(/anthropic api key/i)).toBeNull();
  });
});
```

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test AgentConfig
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/routes/AgentConfig.tsx apps/desktop/src/routes/AgentConfig.test.tsx
git commit -m "feat(agent): AgentConfigForm with validation, LLM key wiring, tests"
```

---

## Task 18: `App.tsx` routing — paired+wallet → Fleet

**Files:**
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/src/App.test.tsx`

- [ ] **Step 1: Replace the post-wallet branch with Fleet**

Edit `apps/desktop/src/App.tsx`. Add a new local state for the Account drawer:

```tsx
import { useEffect, useState } from "react";
import {
  AccountInfo,
  WalletCredentials,
  currentAccount,
  walletStatus,
} from "./lib/tauri-bridge";
import Pair from "./routes/Pair";
import Wallet from "./routes/Wallet";
import Account from "./routes/Account";
import Fleet from "./routes/Fleet";

type Status = "loading" | "unpaired" | "paired-no-wallet" | "paired-with-wallet";

export default function App() {
  const [status, setStatus] = useState<Status>("loading");
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [wallet, setWallet] = useState<WalletCredentials | null>(null);
  const [showAccount, setShowAccount] = useState(false);

  useEffect(() => {
    let active = true;
    (async () => {
      const acct = await currentAccount();
      if (!active) return;
      if (!acct) { setStatus("unpaired"); return; }
      setAccount(acct);
      const w = await walletStatus();
      if (!active) return;
      if (w) { setWallet(w); setStatus("paired-with-wallet"); }
      else setStatus("paired-no-wallet");
    })().catch(() => { if (active) setStatus("unpaired"); });
    return () => { active = false; };
  }, []);

  if (status === "loading") {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-neutral-500">Loading…</p>
      </div>
    );
  }

  if (status === "unpaired" || !account) {
    return (
      <Pair onPaired={async (info) => {
        setAccount(info);
        const w = await walletStatus();
        if (w) { setWallet(w); setStatus("paired-with-wallet"); }
        else setStatus("paired-no-wallet");
      }} />
    );
  }

  if (status === "paired-no-wallet" || !wallet) {
    return (
      <Wallet initialStatus={wallet} onConnected={(creds) => {
        setWallet(creds);
        setStatus("paired-with-wallet");
      }} />
    );
  }

  if (showAccount) {
    return (
      <Account
        account={account}
        onSignOut={() => {
          setAccount(null); setWallet(null); setShowAccount(false); setStatus("unpaired");
        }}
      />
    );
  }

  return <Fleet onOpenAccount={() => setShowAccount(true)} />;
}
```

- [ ] **Step 2: Update `App.test.tsx`**

In `apps/desktop/src/App.test.tsx`, extend the paired-with-wallet test (or add a new one) to expect Fleet:

```tsx
it("shows Fleet route when paired and wallet connected", async () => {
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === "current_account") {
      return { account_id: "acct_x", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
    }
    if (cmd === "wallet_status") {
      return {
        session_pubkey_b58: "s", master_pubkey_b58: "m",
        authorization: { master_pubkey_b58: "m", session_pubkey_b58: "s", message: "msg", signature_b58: "sig", signed_at: "now" },
      };
    }
    if (cmd === "agent_list") return [];
    if (cmd === "recent_events") return [];
    throw new Error(`unexpected: ${cmd}`);
  });
  render(<MemoryRouter><App /></MemoryRouter>);
  expect(await screen.findByText(/Fleet/i)).toBeInTheDocument();
});
```

If an existing test asserts Account is shown post-wallet, change it to navigate via the Account button: render App, find the Account header button, click it, then assert Account content.

- [ ] **Step 3: Verify**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test App
```

Expected: all existing App tests pass plus the new Fleet test.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/App.tsx apps/desktop/src/App.test.tsx
git commit -m "feat(agent): route paired+wallet to Fleet; Account moves behind a header button"
```

---

## Task 19: First-run auto-create-default-agent

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add a setup task that seeds the default agent**

In `apps/desktop/src-tauri/src/lib.rs`, inside the `setup` callback, after `app.manage(state.agent_runtime.clone());` add:

```rust
            let runtime_for_seed = state.agent_runtime.clone();
            tauri::async_runtime::spawn(async move {
                seed_default_agent_if_empty(runtime_for_seed).await;
            });
```

Then add a free function at the bottom of `lib.rs`:

```rust
async fn seed_default_agent_if_empty(runtime: std::sync::Arc<agent::AgentRuntime>) {
    if !runtime.list().await.is_empty() {
        return;
    }
    let id = agent::signal::short_id();
    let cfg = agent::AgentConfig {
        id,
        name: "Default Paper Agent".to_string(),
        max_position_sol: 0.5,
        daily_loss_cap_sol: 2.0,
        nl_overlay: String::new(),
        strategy_template_id: "mock".to_string(),
        llm_backend: agent::config::LlmBackendKind::AnthropicDirect,
        llm_model: "claude-sonnet-4-6".to_string(),
        max_tokens: 1024,
        temperature: 0.0,
    };
    let _ = runtime.spawn(cfg).await;
}
```

You'll need to expose `agent::config` as a public path. The agent `mod.rs` already has `pub mod config;` so this resolves.

Also expose `signal::short_id` and `AgentConfig` through `agent::mod.rs` re-exports (already done in earlier tasks: `pub use config::{AgentConfig, ...}; pub use signal::Signal;`). The function above uses the longer path explicitly so re-exports aren't required.

- [ ] **Step 2: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo check
cargo test
```

Expected: compiles; all existing tests pass. Cannot easily test the first-run path in a Rust unit test without spinning up Tauri, so we rely on the Fleet test ("renders one agent ... has_llm_key: false") + the Playwright smoke test (Task 21) for end-to-end verification.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(agent): seed Default Paper Agent on first launch"
```

---

## Task 20: Integration test against real brainctl-mcp

**Files:**
- Create: `apps/desktop/src-tauri/tests/agent_loop_integration.rs`

- [ ] **Step 1: Write the integration test**

Create `apps/desktop/src-tauri/tests/agent_loop_integration.rs`:

```rust
//! Integration test that spawns a real `AgentRuntime` against a real
//! brainctl-mcp sidecar and a fake LLM backend. Skipped cleanly if
//! `MANTIC_BRAINCTL_BIN` is not set and no default sidecar is present.

use desktop_lib::agent::config::{AgentConfig, LlmBackendKind};
use desktop_lib::agent::llm::{ChatRequest, ChatResponse, LlmBackend, TokenUsage};
use desktop_lib::agent::signal::Signal;
use desktop_lib::agent::state::AgentState;
use desktop_lib::agent::AgentRuntime;
use desktop_lib::brainctl_client::BrainctlClient;
use desktop_lib::error::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn brainctl_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MANTIC_BRAINCTL_BIN") {
        let path = PathBuf::from(p);
        if path.exists() { return Some(path); }
    }
    // common dev location
    let homedir = std::env::var("HOME").ok()?;
    let candidate = PathBuf::from(homedir).join(".local/bin/brainctl-mcp");
    if candidate.exists() { Some(candidate) } else { None }
}

struct ScriptedLlm;

#[async_trait]
impl LlmBackend for ScriptedLlm {
    async fn chat(&self, _r: ChatRequest) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: r#"{"action":"buy","size_sol":0.1,"take_profit_pct":50,"stop_loss_pct":20,"reasoning_summary":"integration test","reasoning_chain":"force buy"}"#.into(),
            stop_reason: "end_turn".into(),
            usage: TokenUsage::default(),
        })
    }
}

#[tokio::test]
async fn agent_loop_writes_decisions_to_real_brainctl() {
    let Some(bin) = brainctl_binary() else {
        eprintln!("skipping: MANTIC_BRAINCTL_BIN not set and ~/.local/bin/brainctl-mcp not found");
        return;
    };

    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("brain.db");

    let brainctl = Arc::new(BrainctlClient::new(bin, db.clone(), "mantic-integration-test"));
    let runtime = AgentRuntime::new(brainctl.clone());

    let cfg = AgentConfig {
        id: "int-1".into(),
        name: "Integration".into(),
        max_position_sol: 0.5,
        daily_loss_cap_sol: 2.0,
        nl_overlay: "".into(),
        strategy_template_id: "mock".into(),
        llm_backend: LlmBackendKind::AnthropicDirect,
        llm_model: "claude-sonnet-4-6".into(),
        max_tokens: 1024,
        temperature: 0.0,
    };

    // Inject our scripted LLM by spawning with the public API + overriding internals isn't
    // currently exposed. Two acceptable paths:
    //   (a) Add a `spawn_with_backend` helper to AgentRuntime gated behind #[cfg(feature="test-helpers")].
    //   (b) Use the live AnthropicDirect — but it 401s without a key.
    // We pick (a) — Task 20 also adds the helper.
    runtime.spawn_with_backend(cfg, Arc::new(ScriptedLlm)).await.unwrap();

    runtime.arm("int-1").await.unwrap();
    runtime.dispatch("int-1", Signal::new_test("BONK")).await.unwrap();

    // Poll until the executor sees the position.
    let executor = runtime.executor();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if !executor.list_for("int-1").is_empty() { break; }
        if std::time::Instant::now() > deadline {
            panic!("agent did not open a position within 5s");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(executor.list_for("int-1").len(), 1);

    // Verify brainctl recorded an event by re-opening the db read-only and counting.
    let conn = rusqlite::Connection::open_with_flags(
        &db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).unwrap();
    let event_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap_or(0);
    assert!(event_count >= 1, "expected at least one event written to brain.db, got {event_count}");
}
```

- [ ] **Step 2: Add `spawn_with_backend` helper gated on a test feature**

In `apps/desktop/src-tauri/Cargo.toml`, add to `[features]` (creating the section if absent):

```toml
[features]
test-helpers = []
```

In `apps/desktop/src-tauri/src/agent/runtime.rs`, add:

```rust
impl AgentRuntime {
    /// Test-only spawn that bypasses keychain key lookup and uses the
    /// provided backend. Available to integration tests via the
    /// `test-helpers` feature.
    #[cfg(any(test, feature = "test-helpers"))]
    pub async fn spawn_with_backend(
        &self,
        config: AgentConfig,
        llm: Arc<dyn LlmBackend>,
    ) -> Result<AgentSummary> {
        config.validate()?;
        let _ = self.persist_new_agent(&config).await;
        let agent = crate::agent::agent::Agent::new(
            config.clone(), llm, self.executor.clone(), self.brainctl.clone(),
        );
        let cloned = agent.clone();
        tokio::spawn(async move { cloned.run().await });
        self.agents.lock().await.insert(config.id.clone(), agent.clone());
        Ok(AgentSummary {
            id: config.id.clone(),
            name: config.name.clone(),
            state: agent.current_state().await,
            has_llm_key: self.has_llm_key(&config.id).await,
        })
    }
}
```

Note: `executor()` is already gated on `any(test, feature = "test-helpers")` from Task 11.

Make `BrainctlClient::new` (already pub) and the `desktop_lib` types accessible from the integration test by ensuring they're re-exported. `lib.rs` already has `pub mod brainctl_client;` and `pub mod error;`. Ensure `pub mod agent;` is `pub` (not just `mod`):

```rust
pub mod agent;
```

(Change in Task 2 was `mod agent;` — promote it to `pub mod agent;` now, plus make `agent::config`, `agent::signal`, `agent::state`, `agent::llm` reachable, which they already are via `pub mod` declarations in `agent/mod.rs`.)

- [ ] **Step 3: Run the test (will skip locally if no brainctl-mcp)**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test --features test-helpers --test agent_loop_integration -- --nocapture
```

Expected outcomes:
- If `~/.local/bin/brainctl-mcp` is present (or `MANTIC_BRAINCTL_BIN` is set): the test runs and passes.
- Otherwise: stdout shows "skipping: MANTIC_BRAINCTL_BIN not set..." and the test exits with success (returns from the test function, which is interpreted as pass).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/agent/runtime.rs \
        apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/tests/agent_loop_integration.rs
git commit -m "test(agent): integration test against real brainctl-mcp (gated)"
```

---

## Task 21: Real-browser smoke test (Playwright against Vite dev server)

**Files:**
- Create: `apps/desktop-smoke/package.json`
- Create: `apps/desktop-smoke/playwright.config.ts`
- Create: `apps/desktop-smoke/tests/fleet.spec.ts`
- Modify: `package.json` (root)

This catches CSS misconfiguration and white-page bugs that jsdom can't see. We run Vite (not Tauri) and mock `window.__TAURI__.core.invoke` so the React app thinks it's paired+wallet-connected and lists one agent.

- [ ] **Step 1: Create the package**

Create `apps/desktop-smoke/package.json`:

```json
{
  "name": "mantic-desktop-smoke",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "playwright test",
    "test:headed": "playwright test --headed",
    "install:browsers": "playwright install chromium"
  },
  "devDependencies": {
    "@playwright/test": "^1.48.0",
    "typescript": "^5.6.0"
  }
}
```

- [ ] **Step 2: Create `playwright.config.ts`**

Create `apps/desktop-smoke/playwright.config.ts`:

```typescript
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  webServer: {
    command: "pnpm --filter mantic-desktop dev",
    url: "http://localhost:1420",
    timeout: 30_000,
    reuseExistingServer: !process.env.CI,
    cwd: "../..",
  },
  use: {
    baseURL: "http://localhost:1420",
    trace: "off",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
```

- [ ] **Step 3: Create the test**

Create `apps/desktop-smoke/tests/fleet.spec.ts`:

```typescript
import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  // Stub the Tauri bridge before any React code runs.
  await page.addInitScript(() => {
    const fakeInvoke = async (cmd: string, _args?: unknown) => {
      switch (cmd) {
        case "current_account":
          return { account_id: "acct_smoke", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
        case "wallet_status":
          return {
            session_pubkey_b58: "s", master_pubkey_b58: "m",
            authorization: { master_pubkey_b58: "m", session_pubkey_b58: "s", message: "msg", signature_b58: "sig", signed_at: "now" },
          };
        case "agent_list":
          return [{ id: "smoke-1", name: "Default Paper Agent", state: { kind: "idle" }, has_llm_key: false }];
        case "recent_events":
          return [];
        default:
          throw new Error("smoke: unexpected command " + cmd);
      }
    };
    // @tauri-apps/api/core imports this shape internally.
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke: fakeInvoke };
    (window as unknown as { __TAURI__?: unknown }).__TAURI__ = { core: { invoke: fakeInvoke } };
  });
});

test("Fleet renders Default Paper Agent on dark background", async ({ page }) => {
  await page.goto("/");
  // Wait for Fleet content.
  await expect(page.getByText("Default Paper Agent")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole("button", { name: /create agent/i })).toBeVisible();

  // Tailwind CSS rendering check — body or the root container should be dark.
  const bg = await page.evaluate(() => {
    const probe = document.querySelector(".bg-neutral-950") as HTMLElement | null;
    if (!probe) return null;
    return getComputedStyle(probe).backgroundColor;
  });
  expect(bg, "expected .bg-neutral-950 to resolve to a computed background-color").not.toBeNull();
  // rgb(...) form; neutral-950 in Tailwind 4 default palette resolves to a near-black colour.
  // We just assert it's not the unstyled default `rgba(0, 0, 0, 0)` / white.
  expect(bg).not.toBe("rgba(0, 0, 0, 0)");
  expect(bg).not.toMatch(/255,\s*255,\s*255/);
});
```

- [ ] **Step 4: Add `test:smoke` script**

In root `package.json`, extend `scripts`:

```json
  "test:smoke": "pnpm --filter mantic-desktop-smoke test"
```

- [ ] **Step 5: Install + run**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm install
pnpm --filter mantic-desktop-smoke install:browsers
pnpm test:smoke
```

Expected: Vite dev server boots on http://localhost:1420, the test loads `/`, finds "Default Paper Agent", and confirms Tailwind classes resolve. Total runtime ~10–15 s.

If `playwright install` is blocked in CI / sandboxed, the test gracefully fails with a clear error pointing to `playwright install chromium`. This is acceptable — it's a real-browser test and needs Chromium.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop-smoke package.json
git commit -m "test(agent): Playwright smoke test for Fleet route + Tailwind rendering"
```

---

## Task 22: README polish + final test sweep

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add an Agent Runtime section to README**

Append a section to `README.md` (after the existing Wallet section):

```markdown
## Agent Runtime (sub-project #4)

Mantic ships a paper-trading agent runtime that drives an LLM-based decision
loop. On first launch a "Default Paper Agent" is auto-created. To exercise
the loop end-to-end:

1. Open Fleet (post-wallet landing screen).
2. Click "Create Agent" — set a name, max position, NL overlay, and paste
   an Anthropic API key. Click Create.
3. Click "Arm" on the agent row.
4. Click "Fire test signal" — a synthetic signal for `BONK` is dispatched.
5. Watch the live tape: decision event + (if buy/sell) trade record appear
   within 1–3 seconds.

### LLM backends

- `anthropic-direct` (BYO key) — works today. Key is stored in the OS
  keychain at `agent-llm-anthropic-key-<agent-id>`.
- `mantic-proxy` — returns "Mantic proxy is not available yet" until
  sub-project #8 lands.

### Brain persistence

Every decision is logged via `decision_add`; every trade via
`event_add(event_type="result")`; every skip via `event_add(event_type="observation")`;
every reasoning chain via `event_add(event_type="decision")`. The Mantic
client uses agent-id `mantic-desktop`; per-agent identity lives as a
brain.db entity in scope `project:mantic`.
```

- [ ] **Step 2: Final full-tree test sweep**

```bash
cd /Users/r4vager/Documents/Mantic

# Rust
(cd apps/desktop/src-tauri && cargo check && cargo test)

# Frontend
pnpm --filter mantic-desktop test
pnpm --filter mantic-desktop lint

# Mock license (sanity)
pnpm --filter mock-license test
```

Expected (running totals after this plan):
- mock-license: 6 (unchanged)
- desktop frontend: 34 baseline + 6 (bridge) + 3 (LiveTape) + 4 (Fleet) + 4 (AgentConfig) + 1 (App Fleet route) ≈ 52
- desktop Rust lib: 44 baseline + 3 (anthropic_direct) + 1 (mantic_proxy) + 3 (LlmKeyStore) + 3 (signal) + 8 (config) + 7 (state) + 7 (executor) + 7 (decision/prompt) + 4 (agent) + 6 (runtime) ≈ 93
- Rust integration: 3 (brainctl + wallet_bridge + agent_loop)
- Playwright smoke: 1 (gated on `playwright install chromium`)

- [ ] **Step 3: Commit + open PR (or push to main if Terrence requests)**

```bash
git add README.md
git commit -m "docs(agent): README section for agent runtime"

# Push branch — parent agent decides on PR vs direct main merge
git push -u origin build/agent-runtime
```

---

## Definition of done

- [ ] All 5 new `AppError` variants compile and are wired
- [ ] `LlmBackend` trait + `AnthropicDirect` + `ManticProxy` stub, 4 mockito tests
- [ ] `LlmKeyStore` round-trip tested (3 tests)
- [ ] `Signal` + `MockSignalSource` tested (3 tests)
- [ ] `AgentConfig` validation tested (8 tests)
- [ ] `AgentState` machine tested (7 tests)
- [ ] `PaperExecutor` open/close/P&L tested (7 tests)
- [ ] `decision::build_prompt` + `parse_response` happy + error paths tested (7 tests)
- [ ] `Agent::handle_signal` lifecycle tested with fake LLM (4 tests)
- [ ] `AgentRuntime` spawn/list/arm/pause/kill/dispatch tested (6 tests)
- [ ] 8 Tauri commands compile and are registered in `invoke_handler!`
- [ ] `AgentRuntime` is in `AppState` and managed in `setup`
- [ ] First-run seed creates Default Paper Agent
- [ ] TypeScript wrappers + 6 bridge tests
- [ ] `Fleet.tsx` + `LiveTape.tsx` rendered, post-wallet landing
- [ ] `AgentConfig.tsx` form with validation + LLM key wiring
- [ ] `App.tsx` routes paired+wallet → Fleet; Account behind a header button
- [ ] Integration test against real brainctl-mcp (skips cleanly when binary absent)
- [ ] Playwright smoke test verifies Fleet renders with Tailwind styles applied
- [ ] README has Agent Runtime section
- [ ] `cargo check` clean, `cargo test` all green, `pnpm test` all green, `pnpm lint` clean
- [ ] No `pnpm tauri build`, `pnpm build:desktop`, or `pnpm dev:desktop` was ever run
- [ ] Branch pushed; parent decides on PR vs direct merge
