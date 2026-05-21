import { invoke } from "@tauri-apps/api/core";

export interface AccountInfo {
  account_id: string;
  tier: "free" | "starter" | "pro" | "whale";
  expires_at: number;
}

export async function pairWithCode(code: string): Promise<AccountInfo> {
  return invoke<AccountInfo>("pair_with_code", { code });
}

export async function currentAccount(): Promise<AccountInfo | null> {
  try {
    return await invoke<AccountInfo>("current_account");
  } catch (e) {
    const msg = typeof e === "string" ? e : String(e);
    if (msg.includes("no license stored") || msg.includes("license expired")) {
      return null;
    }
    throw e;
  }
}

export async function signOut(): Promise<void> {
  await invoke("sign_out");
}

// ---- Brain types ----

export interface BrainStatus {
  memory_count: number;
  event_count: number;
}

export interface EventSummary {
  id: number;
  event_type: string;
  content: string;
  created_at: number;
}

export interface MemorySummary {
  id: number;
  category: string;
  content: string;
  created_at: number;
}

// ---- Reads ----

export function brainStatus(): Promise<BrainStatus> {
  return invoke<BrainStatus>("brain_status");
}

export function recentEvents(limit: number): Promise<EventSummary[]> {
  return invoke<EventSummary[]>("recent_events", { limit });
}

export function recentMemories(limit: number): Promise<MemorySummary[]> {
  return invoke<MemorySummary[]>("recent_memories", { limit });
}

// ---- Writes ----

export interface MemoryAddInput {
  content: string;
  category: string;
  scope?: string;
  tags?: string;
}

export function memoryAdd(input: MemoryAddInput): Promise<unknown> {
  return invoke("memory_add", {
    content: input.content,
    category: input.category,
    scope: input.scope,
    tags: input.tags,
  });
}

export interface EventAddInput {
  eventType: string;
  content: string;
  importance?: number;
}

export function eventAdd(input: EventAddInput): Promise<unknown> {
  return invoke("event_add", input);
}

export interface DecisionAddInput {
  title: string;
  rationale: string;
  project?: string;
}

export function decisionAdd(input: DecisionAddInput): Promise<unknown> {
  return invoke("decision_add", {
    title: input.title,
    rationale: input.rationale,
    project: input.project,
  });
}

export interface EntityCreateInput {
  name: string;
  entityType: string;
  scope?: string;
}

export function entityCreate(input: EntityCreateInput): Promise<unknown> {
  return invoke("entity_create", input);
}

export interface EntityObserveInput {
  entityId: number;
  observation: string;
}

export function entityObserve(input: EntityObserveInput): Promise<unknown> {
  return invoke("entity_observe", input);
}

export interface AgentRegisterInput {
  id: string;
  name: string;
  agentType?: string;
}

export function agentRegister(input: AgentRegisterInput): Promise<unknown> {
  return invoke("agent_register", {
    id: input.id,
    name: input.name,
    agentType: input.agentType,
  });
}

export interface AgentWrapUpInput {
  agentId: string;
  summary: string;
  goal?: string;
  openLoops?: string;
  nextStep?: string;
  project?: string;
}

export function agentWrapUp(input: AgentWrapUpInput): Promise<unknown> {
  return invoke("agent_wrap_up", input);
}

// ---- Complex reads ----

export interface AgentOrientInput {
  agentId: string;
  project?: string;
  query?: string;
}

export function agentOrient(input: AgentOrientInput): Promise<unknown> {
  return invoke("agent_orient", input);
}

export interface MemorySearchInput {
  query: string;
  limit?: number;
}

export function memorySearch(input: MemorySearchInput): Promise<unknown> {
  return invoke("memory_search", input);
}

// ---- Wallet ----

export interface WalletConnectStarted {
  url: string;
}

export interface StoredAuthorization {
  master_pubkey_b58: string;
  session_pubkey_b58: string;
  message: string;
  signature_b58: string;
  signed_at: string;
}

export interface WalletCredentials {
  session_pubkey_b58: string;
  master_pubkey_b58: string;
  authorization: StoredAuthorization;
}

export function walletConnect(): Promise<WalletConnectStarted> {
  return invoke<WalletConnectStarted>("wallet_connect");
}

export function walletStatus(): Promise<WalletCredentials | null> {
  return invoke<WalletCredentials | null>("wallet_status");
}

export function walletRevoke(): Promise<void> {
  return invoke<void>("wallet_revoke");
}

export function walletSignMessage(message: Uint8Array): Promise<string> {
  return invoke<string>("wallet_sign_message", { message: Array.from(message) });
}

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
