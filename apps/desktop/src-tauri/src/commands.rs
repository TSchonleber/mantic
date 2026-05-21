use crate::account::{decode_claims, AccountInfo};
use crate::brain_db::{BrainDb, BrainStatus, EventSummary, MemorySummary};
use crate::brainctl_client::BrainctlClient;
use crate::error::Result;
use crate::license::KeyringStore;
use crate::pairing;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

const DEFAULT_SERVER: &str = "http://localhost:4001";

pub fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string())
}

#[tauri::command]
pub async fn pair_with_code(
    code: String,
    keyring: State<'_, Arc<KeyringStore>>,
) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    let info = decode_claims(&token)?;
    keyring.save(&token)?;
    Ok(info)
}

#[tauri::command]
pub fn current_account(keyring: State<'_, Arc<KeyringStore>>) -> Result<AccountInfo> {
    let token = keyring.load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out(keyring: State<'_, Arc<KeyringStore>>) -> Result<()> {
    keyring.clear()
}

// --- Read commands (rusqlite direct) ---

#[tauri::command]
pub fn brain_status(brain: State<'_, Arc<BrainDb>>) -> Result<BrainStatus> {
    brain.status()
}

#[tauri::command]
pub fn recent_events(limit: u32, brain: State<'_, Arc<BrainDb>>) -> Result<Vec<EventSummary>> {
    brain.recent_events(limit)
}

#[tauri::command]
pub fn recent_memories(limit: u32, brain: State<'_, Arc<BrainDb>>) -> Result<Vec<MemorySummary>> {
    brain.recent_memories(limit)
}

// --- Subprocess commands (write + complex reads) ---

#[tauri::command]
pub async fn memory_add(
    content: String,
    category: String,
    scope: Option<String>,
    tags: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .memory_add(&content, &category, scope.as_deref(), tags.as_deref())
        .await
}

#[tauri::command]
pub async fn event_add(
    event_type: String,
    content: String,
    importance: Option<f64>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.event_add(&event_type, &content, importance).await
}

#[tauri::command]
pub async fn decision_add(
    title: String,
    rationale: String,
    project: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .decision_add(&title, &rationale, project.as_deref())
        .await
}

#[tauri::command]
pub async fn entity_create(
    name: String,
    entity_type: String,
    scope: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .entity_create(&name, &entity_type, scope.as_deref())
        .await
}

#[tauri::command]
pub async fn entity_observe(
    entity_id: i64,
    observation: String,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.entity_observe(entity_id, &observation).await
}

#[tauri::command]
pub async fn agent_register(
    id: String,
    name: String,
    agent_type: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_register(&id, &name, agent_type.as_deref())
        .await
}

#[tauri::command]
pub async fn agent_wrap_up(
    agent_id: String,
    summary: String,
    goal: Option<String>,
    open_loops: Option<String>,
    next_step: Option<String>,
    project: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_wrap_up(
            &agent_id,
            &summary,
            goal.as_deref(),
            open_loops.as_deref(),
            next_step.as_deref(),
            project.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn agent_orient(
    agent_id: String,
    project: Option<String>,
    query: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_orient(&agent_id, project.as_deref(), query.as_deref())
        .await
}

#[tauri::command]
pub async fn memory_search(
    query: String,
    limit: Option<u32>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.memory_search(&query, limit).await
}
