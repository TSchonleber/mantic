// Minimal AgentState enum for Task 6 compilation. Transitions + tests land in Task 7.

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
