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
