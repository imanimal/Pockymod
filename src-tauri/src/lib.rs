//! Tauri shell: window setup and commands. All Nexus and file logic lives in `pocket-core`.

#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({ "name": pocket_core::APP_NAME, "version": pocket_core::APP_VERSION, "game": pocket_core::GAME_DOMAIN })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Mods");
}
