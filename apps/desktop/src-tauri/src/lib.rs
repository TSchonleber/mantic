mod account;
mod agent;
mod brain_db;
pub mod brainctl_client;
mod bundle;
mod commands;
pub mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
mod state;
pub mod wallet;

use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_data = app.path().app_data_dir().expect("app data dir");
            let brain_path = app_data.join("mantic").join("brain.db");

            let brainctl_binary = bundle::brainctl_mcp_path(&app.handle())
                .unwrap_or_else(|_| PathBuf::from("brainctl-mcp"));

            let state = AppState::new(brain_path, brainctl_binary)
                .expect("failed to build app state");

            // Heartbeat task
            let keyring_for_task = state.keyring.clone();
            tauri::async_runtime::spawn(heartbeat::run_forever(
                keyring_for_task,
                commands::server_url(),
                Duration::from_secs(60 * 5),
                60 * 60 * 2,
            ));

            // Register individual Arcs so commands can take State<'_, Arc<KeyringStore>>, etc.
            app.manage(state.keyring.clone());
            app.manage(state.brain.clone());
            app.manage(state.brainctl.clone());
            app.manage(state.wallet.clone());
            app.manage(state.agent_runtime.clone());

            let runtime_for_seed = state.agent_runtime.clone();
            tauri::async_runtime::spawn(async move {
                seed_default_agent_if_empty(runtime_for_seed).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
            commands::brain_status,
            commands::recent_events,
            commands::recent_memories,
            commands::memory_add,
            commands::event_add,
            commands::decision_add,
            commands::entity_create,
            commands::entity_observe,
            commands::agent_register,
            commands::agent_wrap_up,
            commands::agent_orient,
            commands::memory_search,
            commands::wallet_connect,
            commands::wallet_status,
            commands::wallet_revoke,
            commands::wallet_sign_message,
            commands::agent_create,
            commands::agent_list,
            commands::agent_get,
            commands::agent_arm,
            commands::agent_pause,
            commands::agent_kill,
            commands::agent_fire_test_signal,
            commands::agent_set_llm_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn seed_default_agent_if_empty(runtime: std::sync::Arc<agent::AgentRuntime>) {
    if !runtime.list().await.is_empty() {
        return;
    }
    let id = agent::signal::short_id();
    let cfg = agent::AgentConfig {
        id,
        name: "Default Paper Agent".to_string(),
        max_position_sol: 0.5,
        daily_loss_cap_sol: 2.0,
        nl_overlay: String::new(),
        strategy_template_id: "mock".to_string(),
        llm_backend: agent::config::LlmBackendKind::AnthropicDirect,
        llm_model: "claude-sonnet-4-6".to_string(),
        max_tokens: 1024,
        temperature: 0.0,
    };
    let _ = runtime.spawn(cfg).await;
}
