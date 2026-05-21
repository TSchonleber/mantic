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
