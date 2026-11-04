//! Tauri shell: window setup and commands. All Nexus and file logic lives in `pocket-core`.

mod commands;
mod runner;
mod state;

use tauri::{Emitter, Manager};

#[tauri::command]
fn app_info() -> serde_json::Value {
    serde_json::json!({ "name": pocket_core::APP_NAME, "version": pocket_core::APP_VERSION, "game": pocket_core::GAME_DOMAIN })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // a second start (for example Windows opening an nxm:// link) hands its arguments to this window and exits
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") { let _ = w.unminimize(); let _ = w.set_focus(); }
            for a in argv.into_iter().filter(|a| a.starts_with("nxm://")) {
                let h = app.clone();
                tauri::async_runtime::spawn(async move { runner::handle_nxm(&h, &a).await });
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = state::AppState::new(data_dir)?;
            // every Nexus response updates the rate-limit LCD
            let handle = app.handle().clone();
            state.nexus.set_observer(std::sync::Arc::new(move |rl| { let _ = handle.emit("rate-limit", rl); }));
            app.manage(state);
            runner::start(app.handle().clone());
            // started by Windows for an nxm:// link
            for a in std::env::args().filter(|a| a.starts_with("nxm://")) {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move { runner::handle_nxm(&h, &a).await });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::nexus::nexus_status,
            commands::nexus::nexus_set_key,
            commands::nexus::nexus_clear_key,
            commands::nexus::rate_limit,
            commands::browse::browse_search,
            commands::browse::browse_detail,
            commands::browse::set_mod_flag,
            commands::browse::settings_get,
            commands::browse::settings_set,
            commands::browse::patches,
            commands::library::paths_get,
            commands::library::paths_set,
            commands::library::library_list,
            commands::library::library_uninstall,
            commands::library::library_deploy,
            commands::library::library_install_file,
            commands::library::library_downloads,
            commands::library::queue_get,
            commands::library::queue_plan,
            commands::library::queue_add,
            commands::library::queue_add_mod,
            commands::collections::collections_browse,
            commands::collections::collection_revision,
            commands::collections::collection_check,
            commands::collections::collection_install,
            commands::collections::local_collection_create,
            commands::collections::local_collection_export,
            commands::collections::local_collection_import,
            commands::extras::app_update_check,
            commands::extras::modio_has_key,
            commands::extras::modio_set_key,
            commands::extras::modio_clear_key,
            commands::extras::modio_browse,
            commands::game::game_detect,
            commands::game::game_inspect,
            commands::game::health_check,
            commands::game::redmod_deploy,
            commands::game::redmod_order,
            commands::game::profiles_list,
            commands::game::profile_create,
            commands::game::profile_rename,
            commands::game::profile_delete,
            commands::game::profile_activate,
            commands::game::mod_set_enabled,
            commands::game::load_order_set,
            commands::game::updates_check,
            commands::game::updates_last,
            commands::library::queue_remove,
            commands::library::queue_retry,
            commands::library::queue_clear_finished,
            commands::library::queue_show_page,
            commands::library::open_nexus_page,
            commands::library::open_nexus_url,
            commands::library::nxm_handler_status,
            commands::library::nxm_handler_register,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Mods");
}
