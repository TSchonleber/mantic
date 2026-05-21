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
        matches!(
            self,
            AgentState::Idle | AgentState::Paused | AgentState::Error { .. }
        )
    }

    pub fn can_pause(&self) -> bool {
        matches!(self, AgentState::Armed | AgentState::Running { .. })
    }

    pub fn require_armed(&self) -> Result<()> {
        if self.can_dispatch() {
            Ok(())
        } else {
            Err(AppError::AgentInvalidState(format!(
                "agent must be Armed to dispatch a signal (current: {:?})",
                self
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
        let s = AgentState::Running {
            step: RunStep::CallingLlm,
        };
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
        let s = AgentState::Running {
            step: RunStep::Orienting,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"running\""));
        assert!(json.contains("\"step\":\"orienting\""));
        let back: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
