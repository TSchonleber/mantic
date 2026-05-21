# Agent Runtime — Sub-project #4 Design Spec

**Date:** 2026-05-21
**Status:** Approved by founder. Ready for implementation plan.
**Builds on:** sub-project #3 (wallet + session signer, complete at main `f34d2b2`)

---

## Goal

Ship the **engine** that runs trading strategies — agent lifecycle, LLM-driven decision loop, paper-trading executor, decision logging to brain.db. NOT the strategies themselves (sub-project #5), NOT real on-chain trades (later), NOT real signal feeds (sub-project #6).

After #4, the user can create one or more agents in the desktop UI, "arm" them, fire test signals via a debug button, and watch the loop work end-to-end: signal → brain orient → Claude decides → paper-execute → decision logged → live tape updates.

## Scope

### In

- **Paper trading only** — agents make decisions, log them, "execute" against an in-process `PaperExecutor` that updates positions in brain.db and a separate in-memory ledger. No on-chain transactions.
- **LLM via swappable backend abstraction**:
  - `AnthropicDirect` (BYO key) — works today via the real Anthropic Messages API
  - `ManticProxy` (placeholder, returns `LlmBackend::ProxyNotAvailable` until sub-project #8 ships the brainctl.org backend)
  - Mantic-side code is identical for both; the only difference is base URL + auth header
- **Mock signal source** — a debug "fire test signal" button + an optional in-process timer that injects synthesized signals. Real signal feeds (Grok-X firehose, KOL maps) are sub-project #6.
- **Agent state machine** — `Idle | Armed | Running { step } | Paused | Error { reason }`
- **brain.db is the source of truth** for agent config + decisions + paper trades. No new SQL tables in Mantic.
- **Fleet UI surface** — lists agents, shows status, has "Create Agent" + "Fire test signal" debug buttons, embedded LiveTape.
- **First-run UX** — auto-create one "Default Paper Agent" so the Fleet screen has something to show immediately.

### Out (deferred to later sub-projects)

- Real on-chain trades (sub-project #5+)
- Real signal feeds (sub-project #6 — Signal API)
- Real strategy templates with logic (sub-project #5 — Strategy Templates)
- Brain inspector UI (sub-project #11 — GUI Surfaces)
- Per-agent autonomy modes (autonomous vs confirm) — v1 agents are autonomous-only since paper trades are riskless
- Automated reflexion-on-loss (deferred — paper P&L is too synthetic to drive real reflexions yet)
- LLM token cost tracking
- Federated brain insight push (sub-project #7)
- Multi-machine sync (Whale tier perk, sub-project #11)

## Architecture

### New Rust modules in `apps/desktop/src-tauri/src/agent/`

- `mod.rs` — re-exports
- `runtime.rs` — `AgentRuntime` owns the fleet, provides spawn/list/kill operations
- `agent.rs` — `Agent` struct: id, config, state machine, owns its own tokio task
- `state.rs` — `AgentState` enum
- `config.rs` — `AgentConfig` (name, risk envelope, NL overlay, strategy_template_id, llm_config). Serializable to brain.db as memories.
- `signal.rs` — `Signal` type + `SignalSource` trait + `MockSignalSource` impl (debug button + optional timer)
- `executor.rs` — `Executor` trait + `PaperExecutor` impl (logs to brain.db, tracks positions in an in-memory map)
- `decision.rs` — per-turn orchestration: orient → prompt build → LLM call → parse → execute → log
- `llm/mod.rs` — `LlmBackend` trait
- `llm/anthropic_direct.rs` — BYO-key Anthropic Messages API client
- `llm/mantic_proxy.rs` — stub returning `ProxyNotAvailable`
- `llm/prompt.rs` — prompt construction helper (signal + NL overlay + brain context + paper state)

### Modifications to existing modules

- `state.rs` — adds `agent_runtime: Arc<AgentRuntime>` field to AppState
- `commands.rs` — new commands: `agent_create`, `agent_list`, `agent_get`, `agent_arm`, `agent_pause`, `agent_kill`, `agent_fire_test_signal`, `agent_set_llm_key`
- `lib.rs` — register new commands + manage agent_runtime in setup
- `error.rs` — new variants: `AgentNotFound`, `AgentInvalidState`, `LlmBackend`, `LlmProxyNotAvailable`, `PaperExecutor`

### Frontend (`apps/desktop/src/`)

- `routes/Fleet.tsx` (new) — replaces the placeholder Account route. Lists agents, status badges, "Create Agent" button, "Fire test signal" debug button per agent.
- `routes/AgentConfig.tsx` (new) — modal/page for create/edit agent config: name, max position size, daily loss cap, allowed-tokens whitelist (optional), NL overlay textarea, strategy template dropdown (just one entry: "Mock — accepts any signal" for v1), Anthropic API key input.
- `components/LiveTape.tsx` (new) — embedded in Fleet view, polls `recent_events(50)` every 5s and renders the most recent decisions/trades.
- `App.tsx` — routing: paired+wallet now lands on Fleet (currently Account). Account is removed or demoted to a settings sub-route.

### LLM client

The `LlmBackend` trait:

```rust
#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}

pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub model: String, // e.g. "claude-sonnet-4-6"
    pub max_tokens: u32,
    pub temperature: f32,
}

pub enum Message { User { content: String }, Assistant { content: String } }

pub struct ChatResponse {
    pub content: String, // raw text — decision.rs parses
    pub stop_reason: String,
    pub usage: TokenUsage,
}
```

`AnthropicDirect` implementation:
- POSTs to `https://api.anthropic.com/v1/messages`
- `x-api-key: <user's BYO key>`, `anthropic-version: 2023-06-01`
- Standard Messages API request/response shapes
- BYO key stored via the existing `KeyringStore` pattern with a new slot `agent-llm-anthropic-key`

`ManticProxy` implementation:
- Same wire format (Anthropic Messages API spec)
- Base URL `https://brainctl.org/v1/llm-proxy` (placeholder)
- `authorization: Bearer <license JWT>` instead of `x-api-key`
- Returns `LlmBackend::ProxyNotAvailable` for v1 since the brainctl.org backend doesn't exist yet

Each `Agent` holds an `Arc<dyn LlmBackend>` chosen at agent creation time based on user preference.

### Decision loop (per signal)

```
1. Signal arrives via runtime.dispatch(signal) — from MockSignalSource debug button or timer
2. Agent transitions Armed → Running { step: "orienting" }
3. brain_orient(agent_id, query: signal.token_symbol) via existing BrainctlClient
4. decision::build_prompt(signal, agent.config, brain.memories, paper.positions)
5. Agent transitions Running { step: "calling_llm" }
6. agent.llm.chat(request) — real Anthropic call or proxy stub
7. decision::parse_response(response) → DecisionOutcome { Buy { size, tp, sl } | Skip { reason } | Sell { position_id, reason } }
8. Agent transitions Running { step: "executing" }
9. PaperExecutor processes the outcome:
   - Buy → insert into in-memory position map + write event_add (type="result", content=trade record)
   - Skip → write event_add (type="observation", content="skipped: " + reason, importance=0.3)
   - Sell → close position, compute P&L, write event_add (type="result", importance scaled to |P&L|)
10. decision_add(rationale: response.reasoning_summary, project: "mantic")
11. Agent transitions back to Armed (ready for next signal)
```

### Prompt construction (the meat of the LLM call)

Pseudocode:

```
SYSTEM:
"You are Mantic, an autonomous trading agent. You evaluate signals and
make paper-trade decisions for the user. You ALWAYS respond with a
single JSON object matching this schema:
{
  \"action\": \"buy\" | \"sell\" | \"skip\",
  \"size_sol\": number (only for buy),
  \"take_profit_pct\": number (only for buy),
  \"stop_loss_pct\": number (only for buy),
  \"position_id\": string (only for sell),
  \"reasoning_summary\": string (≤200 chars),
  \"reasoning_chain\": string (your full thought process)
}
No prose outside the JSON. No markdown fences."

USER:
"Agent config:
  Name: {{name}}
  Max position size: {{max_position_sol}} SOL
  Daily loss cap: {{daily_loss_cap_sol}} SOL
  Behavior notes: {{nl_overlay}}

Current paper positions:
{{positions_json}}

Recent brain memories relevant to this signal:
{{brain_orient_results}}

NEW SIGNAL:
  Token: {{signal.token_symbol}}
  Source: {{signal.source}}
  Context: {{signal.context_tags}}
  Payload: {{signal.payload_json}}

Decide your action."
```

The LLM is expected to return a JSON object that `decision::parse_response` can parse with `serde_json::from_str`. If parsing fails, the agent transitions to `Error { reason: "llm returned malformed JSON" }` and the failed decision is logged for review.

### Tauri commands

- `agent_create(config: AgentConfigInput) -> AgentSummary`
- `agent_list() -> Vec<AgentSummary>`
- `agent_get(id: String) -> AgentDetails` — includes recent decisions
- `agent_arm(id: String) -> AgentSummary`
- `agent_pause(id: String) -> AgentSummary`
- `agent_kill(id: String) -> ()` — removes agent and its config from brain.db (entity remains as historical record)
- `agent_fire_test_signal(id: String, signal: Signal) -> ()`
- `agent_set_llm_key(id: String, api_key: String) -> ()` — stores in keychain at `agent-llm-anthropic-key-<agent-id>`

### Data persistence model

All agent state lives in brain.db. No new SQL tables in Mantic.

- **Agent identity**: `entity_create(name: agent.name, entity_type: "agent", scope: "project:mantic")` → returns entity_id which becomes the agent's primary key
- **Agent config**: `memory_add(content: <JSON config blob>, category: "convention", scope: "agent:<entity_id>", tags: "agent-config,v1")` — JSON serialized AgentConfig; agent_orient retrieves it on app launch
- **Agent registration with brainctl**: `agent_register(id: <entity_id>, name: agent.name, type: "trading-agent")` — for brainctl's internal accounting
- **Decisions**: `decision_add(title: "Decision <turn_id>", rationale: <reasoning_summary>, project: "mantic")`
- **Reasoning chains and signals**: `event_add(event_type: "decision", content: <full chain>, importance: 0.7)`
- **Paper trades**: `event_add(event_type: "result", content: <trade record>, importance: <|P&L|-scaled>)`
- **Skipped signals**: `event_add(event_type: "observation", content: "skipped: <reason>", importance: 0.3)`
- **Wrap-up on agent kill**: `agent_wrap_up(agent_id, summary, ...)`

In-memory side state (NOT persisted across app restarts in v1):
- Open paper positions per agent (just a `HashMap<position_id, Position>` inside PaperExecutor)
- Restart wipes positions; the brain.db decision log persists. We accept this for v1; sub-project #11 polish can add position rehydration.

### First-run UX

On Mantic launch, if `agent_list()` returns 0 agents, auto-create one:

```
name: "Default Paper Agent"
max_position_sol: 0.5
daily_loss_cap_sol: 2.0
nl_overlay: "" (empty)
strategy_template_id: "mock"
llm_backend: "anthropic-direct" (waits for user to paste a key before arming)
status: Idle
```

User can immediately rename, configure, or delete it. Just gives the Fleet screen something to show.

## Failure modes

| Failure | Detection | Recovery |
|---|---|---|
| LLM returns malformed JSON | `serde_json::from_str` error in `decision::parse_response` | Transition to `Error { reason: "llm json parse failed" }`. Log the raw response to event_add. User manually re-arms. |
| LLM API key invalid | 401 from Anthropic | Transition to `Error`. UI surfaces "API key invalid". |
| Anthropic rate limit | 429 | Exponential backoff up to 3 retries; if all fail, transition to `Error`. |
| brain.db write failure | brainctl returns error | Single retry; if persistent, transition to `Error` and pause all agents (brain is required infrastructure). |
| Mantic proxy chosen but not available | `LlmBackend::ProxyNotAvailable` | Transition to `Error { reason: "Mantic proxy is not available yet — switch to BYO mode or wait for sub-project #8" }`. Loud and clear UX. |
| Signal payload malformed | enum parsing in `Signal::try_from` | Reject at runtime.dispatch boundary; agent never enters Running state for it. Log to event_add. |

## Testing

### Unit

- `state.rs` — state machine transitions (all paths and rejections)
- `config.rs` — config validation (risk envelope sanity checks)
- `signal.rs` — `MockSignalSource` injects via timer + manual fire
- `executor.rs` — `PaperExecutor` open/close, P&L math, position tracking
- `decision.rs` — prompt building with synthesized inputs, response parsing happy + malformed paths
- `llm/anthropic_direct.rs` — mockito-backed HTTP tests against the Anthropic Messages API shape
- `llm/mantic_proxy.rs` — returns `ProxyNotAvailable` always (one-line test)
- `runtime.rs` — fleet operations (spawn, list, kill, dispatch routing)
- `agent.rs` — full agent task lifecycle with mocked LLM + signal + executor

### Frontend (Vitest + jsdom)

- `Fleet.tsx` — renders empty state, renders list with statuses, "Create Agent" button opens config
- `AgentConfig.tsx` — form validation, save, edit, cancel
- `LiveTape.tsx` — polls and renders events from a mocked tauri-bridge

### Integration (Rust, gated)

- `tests/agent_loop_integration.rs` — spawns a real `AgentRuntime` with a fake LlmBackend that returns canned decisions, fires a test signal, asserts the full loop runs (orient → decide → paper-execute → events logged to a real brain.db via the bundled brainctl-mcp binary). Skips cleanly if the sidecar binary isn't present.

### Real browser smoke (NEW for this sub-project)

Per the feedback memory from sub-project #3 (Vitest didn't catch the white-box bug), we add a CI-runnable smoke test that:
1. Starts Vite + Tauri dev (no bundler — `cargo run --release` style)
2. Uses `tauri-driver` or a screenshot-then-image-hash check to confirm the Fleet UI actually renders content (non-white pixels in the viewport)
3. Skipped on macOS until tauri-driver lands; runs on Linux CI

If `tauri-driver` is too painful, the fallback is a Playwright test against the dev Vite server (which serves the React app standalone, just without the Rust commands) to verify the Fleet route renders and Tailwind classes resolve.

## Implementation plan decomposition

Single plan, this sub-project fits in one. Estimated 18-22 tasks. Suggested build order:

1. Branch + new agent error variants
2. `LlmBackend` trait + `AnthropicDirect` impl with mockito tests
3. `ManticProxy` stub
4. LLM key storage in keychain (`agent-llm-anthropic-key` slot)
5. `Signal` type + `MockSignalSource`
6. `AgentConfig` + serde
7. `AgentState` machine
8. `PaperExecutor` + position math
9. `decision::build_prompt` + `decision::parse_response`
10. `Agent` task — full lifecycle, holds LlmBackend + Executor + Signal channel
11. `AgentRuntime` — fleet management
12. Brain persistence: write config on create, log decisions/events
13. Tauri commands (8 commands)
14. AppState wiring + setup
15. TS bridge — typed wrappers
16. `Fleet.tsx` + `LiveTape.tsx`
17. `AgentConfig.tsx`
18. App.tsx routing — paired+wallet → Fleet
19. First-run auto-create-default-agent logic
20. Integration test against real brainctl-mcp
21. Real-browser smoke test scaffold (Playwright against Vite dev server)
22. README polish

Estimated 6-9 hours via subagent-driven execution.
