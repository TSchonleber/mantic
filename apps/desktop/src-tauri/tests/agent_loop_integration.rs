#![cfg(feature = "test-helpers")]

use async_trait::async_trait;
use desktop_lib::agent::config::{AgentConfig, LlmBackendKind};
use desktop_lib::agent::llm::{ChatRequest, ChatResponse, LlmBackend, TokenUsage};
use desktop_lib::agent::signal::Signal;
use desktop_lib::agent::AgentRuntime;
use desktop_lib::brainctl_client::BrainctlClient;
use desktop_lib::error::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn brainctl_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MANTIC_BRAINCTL_BIN") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let candidate = PathBuf::from(home).join(".local/bin/brainctl-mcp");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
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
    let runtime = AgentRuntime::new(brainctl);

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

    runtime
        .spawn_with_backend(cfg, Arc::new(ScriptedLlm))
        .await
        .unwrap();
    runtime.arm("int-1").await.unwrap();
    runtime
        .dispatch("int-1", Signal::new_test("BONK"))
        .await
        .unwrap();

    let executor = runtime.executor();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if !executor.list_for("int-1").is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("agent did not open a position within 5s");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let conn =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let event_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .unwrap_or(0);
    assert!(
        event_count >= 1,
        "expected at least one event written to brain.db, got {event_count}"
    );
}
