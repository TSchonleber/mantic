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
    pub llm: Arc<dyn LlmBackend>,
    pub executor: Arc<PaperExecutor>,
    pub brainctl: Arc<BrainctlClient>,
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
