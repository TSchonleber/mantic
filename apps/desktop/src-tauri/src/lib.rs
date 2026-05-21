mod account;
mod brain_db;
mod brainctl_client;
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;

use license::KeyringStore;
use std::sync::Arc;
use std::time::Duration;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let keyring = Arc::new(KeyringStore::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(keyring.clone())
        .setup(move |_app| {
            let keyring_for_task = keyring.clone();
            tauri::async_runtime::spawn(heartbeat::run_forever(
                keyring_for_task,
                commands::server_url(),
                Duration::from_secs(60 * 5),
                60 * 60 * 2,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
