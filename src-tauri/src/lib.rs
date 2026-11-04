//! Tauri shell: window setup and commands. All Nexus and file logic lives in `pocket-core`.

mod commands;
mod state;

use tauri::{Emitter, Manager};

#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({ "name": pocket_core::APP_NAME, "version": pocket_core::APP_VERSION, "game": pocket_core::GAME_DOMAIN })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::new()?;
            // every Nexus response updates the rate-limit LCD
            let handle = app.handle().clone();
            state.nexus.set_observer(std::sync::Arc::new(move |rl| { let _ = handle.emit("rate-limit", rl); }));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::nexus::nexus_status,
            commands::nexus::nexus_set_key,
            commands::nexus::nexus_clear_key,
            commands::nexus::rate_limit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Mods");
}
