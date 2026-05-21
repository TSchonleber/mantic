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

pub fn parse_response(raw: &str) -> Result<DecisionOutcome> {
    let trimmed = strip_fences(raw.trim());
    serde_json::from_str::<DecisionOutcome>(trimmed).map_err(|e| {
        AppError::LlmBackend(format!("llm json parse failed: {e}; raw={trimmed}"))
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
            DecisionOutcome::Buy {
                size_sol,
                take_profit_pct,
                ..
            } => {
                assert!((size_sol - 0.25).abs() < 1e-9);
                assert!((take_profit_pct - 50.0).abs() < 1e-9);
            }
            _ => panic!("expected buy"),
        }
    }

    #[test]
    fn parses_sell() {
        let raw =
            r#"{"action":"sell","position_id":"abc","reasoning_summary":"tp hit","reasoning_chain":"."}"#;
        match parse_response(raw).unwrap() {
            DecisionOutcome::Sell { position_id, .. } => assert_eq!(position_id, "abc"),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_skip() {
        let raw =
            r#"{"action":"skip","reasoning_summary":"low conviction","reasoning_chain":"."}"#;
        match parse_response(raw).unwrap() {
            DecisionOutcome::Skip {
                reasoning_summary, ..
            } => assert_eq!(reasoning_summary, "low conviction"),
            _ => panic!(),
        }
    }

    #[test]
    fn strips_markdown_fences() {
        let raw =
            "```json\n{\"action\":\"skip\",\"reasoning_summary\":\"x\",\"reasoning_chain\":\"y\"}\n```";
        assert!(parse_response(raw).is_ok());
    }

    #[test]
    fn malformed_json_returns_llm_backend_error() {
        let err = parse_response("not json").unwrap_err();
        assert!(matches!(err, AppError::LlmBackend(_)));
    }

    #[test]
    fn unknown_action_returns_error() {
        let err = parse_response(
            r#"{"action":"yolo","reasoning_summary":"","reasoning_chain":""}"#,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::LlmBackend(_)));
    }
}
