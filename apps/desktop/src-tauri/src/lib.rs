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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
