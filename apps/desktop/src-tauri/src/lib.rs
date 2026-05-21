mod account;
mod commands;
mod error;
mod heartbeat;
mod license;
mod pairing;

use std::time::Duration;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            tauri::async_runtime::spawn(heartbeat::run_forever(
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
