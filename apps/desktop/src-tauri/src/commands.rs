use crate::account::{decode_claims, AccountInfo};
use crate::brain_db::{BrainDb, BrainStatus, EventSummary, MemorySummary};
use crate::brainctl_client::BrainctlClient;
use crate::error::Result;
use crate::license::KeyringStore;
use crate::pairing;
use crate::wallet::{WalletCredentials, WalletStore};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::{AppHandle, Emitter, State};

static CONNECT_IN_PROGRESS: StdMutex<bool> = StdMutex::new(false);

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
    identifier: String,
    observations: String,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.entity_observe(&identifier, &observations).await
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
    summary: String,
    goal: Option<String>,
    open_loops: Option<String>,
    next_step: Option<String>,
    project: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_wrap_up(
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
    project: Option<String>,
    query: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_orient(project.as_deref(), query.as_deref())
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

// --- Wallet commands ---

#[derive(Debug, Serialize)]
pub struct WalletConnectStarted {
    pub url: String,
}

#[tauri::command]
pub async fn wallet_connect(
    app: AppHandle,
    wallet: State<'_, Arc<WalletStore>>,
) -> Result<WalletConnectStarted> {
    {
        let mut guard = CONNECT_IN_PROGRESS.lock().unwrap();
        if *guard {
            return Err(crate::error::AppError::WalletConnectInProgress);
        }
        *guard = true;
    }

    let bridge_html = load_bridge_html(&app)?;
    let (url, server) = crate::wallet::BridgeServer::start(bridge_html).await?;

    // Spawn the authorize-wait in a background task so wallet_connect can return immediately
    // with the URL the frontend should open.
    let wallet = wallet.inner().clone();
    let app_handle = app.clone();
    let url_for_open = url.clone();
    tauri::async_runtime::spawn(async move {
        let result = server.await_authorization().await;
        {
            let mut guard = CONNECT_IN_PROGRESS.lock().unwrap();
            *guard = false;
        }
        match result {
            Ok(outcome) => {
                if let Err(e) = wallet.save(
                    &outcome.session_key,
                    &outcome.stored.master_pubkey_b58,
                    &outcome.stored,
                ) {
                    let _ = app_handle.emit("wallet:error", e.to_string());
                    return;
                }
                let creds = WalletCredentials {
                    session_pubkey_b58: outcome.session_key.pubkey_base58(),
                    master_pubkey_b58: outcome.stored.master_pubkey_b58.clone(),
                    authorization: outcome.stored,
                };
                let _ = app_handle.emit("wallet:connected", creds);
            }
            Err(e) => {
                let _ = app_handle.emit("wallet:error", e.to_string());
            }
        }
    });

    // Open the user's default browser to the bridge URL
    open_browser(&url_for_open);
    Ok(WalletConnectStarted { url })
}

#[tauri::command]
pub fn wallet_status(wallet: State<'_, Arc<WalletStore>>) -> Result<Option<WalletCredentials>> {
    wallet.status()
}

#[tauri::command]
pub fn wallet_revoke(wallet: State<'_, Arc<WalletStore>>) -> Result<()> {
    wallet.clear()
}

#[tauri::command]
pub async fn wallet_sign_message(
    message: Vec<u8>,
    wallet: State<'_, Arc<WalletStore>>,
) -> Result<String> {
    let (session, _creds) = wallet.load()?;
    let sig = session.sign(&message);
    Ok(bs58::encode(sig).into_string())
}

fn load_bridge_html(app: &AppHandle) -> Result<String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve("resources/wallet-bridge.html", tauri::path::BaseDirectory::Resource)
        .map_err(|e| crate::error::AppError::WalletBridge(format!("resolve resource: {e}")))?;
    std::fs::read_to_string(&path)
        .map_err(|e| crate::error::AppError::WalletBridge(format!("read {}: {e}", path.display())))
}

fn open_browser(url: &str) {
    // Best-effort: use the OS default URL handler. Failures are surfaced as an
    // event because wallet_connect has already returned.
    let _ = std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    })
    .args(if cfg!(target_os = "windows") {
        vec!["/C", "start", "", url]
    } else {
        vec![url]
    })
    .spawn();
}
