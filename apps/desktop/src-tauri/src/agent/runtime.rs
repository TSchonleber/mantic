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
        let name = a.config.read().await.name.clone();
        let state = a.current_state().await;
        let has_llm_key = self.has_llm_key(id).await;
        Ok(AgentSummary {
            id: id.into(),
            name,
            state,
            has_llm_key,
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
        let name = a.config.read().await.name.clone();
        let state = a.current_state().await;
        let has_llm_key = self.has_llm_key(id).await;
        Ok(AgentSummary {
            id: id.into(),
            name,
            state,
            has_llm_key,
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
