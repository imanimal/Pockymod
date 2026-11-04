//! Tauri shell: window setup and commands. All Nexus and file logic lives in `pocket-core`.

mod commands;
mod power;
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
        .manage(power::Power::default())
        // closing the main window turns the picture off like a tube first (power.rs), then the app quits
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    power::off(window.app_handle());
                }
            }
        })
        // extension panels: their page, with its own strict CSP, for a sandboxed frame (docs/extensions.md)
        .register_uri_scheme_protocol("pmext", |ctx, request| commands::extensions::panel_protocol(ctx.app_handle(), request))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = state::AppState::new(data_dir)?;
            // every Nexus response updates the rate-limit LCD
            let handle = app.handle().clone();
            state.nexus.set_observer(std::sync::Arc::new(move |rl| { let _ = handle.emit("rate-limit", rl); }));
            app.manage(state);
            runner::start(app.handle().clone());
            // half-finished extractions from a crash (<library>/staging/*.new) are never used again
            if let Ok((_, lib)) = state::Paths::load(&app.state::<state::AppState>().db.lock().unwrap()).require() {
                for e in std::fs::read_dir(lib.join("staging")).into_iter().flatten().flatten() {
                    if e.path().extension().is_some_and(|x| x == "new") { let _ = std::fs::remove_dir_all(e.path()); }
                }
            }
            // the app was closed while the game ran without mods: put them back once the game is gone
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let st = h.state::<state::AppState>();
                if st.db.lock().unwrap().setting(pocket_core::launch::VANILLA_KEY).ok().flatten().is_none() { return; }
                while tauri::async_runtime::spawn_blocking(pocket_core::launch::game_running).await.unwrap_or(false) {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                let p = state::Paths::load(&st.db.lock().unwrap());
                if let Ok((g, l)) = p.require() {
                    let n = pocket_core::launch::vanilla_restore(&st.db.lock().unwrap(), &g, &l, &p.deploy_options()).unwrap_or(0);
                    let _ = h.emit("library-changed", ());
                    let _ = h.emit("vanilla", serde_json::json!({ "state": "restored", "back": n }));
                }
            });
            // started by Windows for an nxm:// link
            for a in std::env::args().filter(|a| a.starts_with("nxm://")) {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move { runner::handle_nxm(&h, &a).await });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            power::power_done,
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
            commands::browse::media_browse,
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
            commands::library::library_identify,
            commands::library::queue_cancel,
            commands::nexus::mod_endorse,
            commands::nexus::mod_track,
            commands::game::updates_skip,
            commands::game::game_launch_vanilla,
            commands::game::scripts_reset,
            commands::game::game_vanilla_restore,
            commands::game::game_vanilla_pending,
            commands::game::game_open_folder,
            commands::existing::library_restore_replaced,
            commands::existing::library_trash_list,
            commands::existing::library_trash_restore,
            commands::existing::library_trash_purge,
            commands::existing::library_duplicates,
            commands::existing::library_keep_one,
            commands::addons::addon_list,
            commands::addons::addon_save,
            commands::addons::addon_remove,
            commands::addons::market_get,
            commands::addons::market_fetch,
            commands::extensions::ext_states,
            commands::extensions::ext_state_set,
            commands::extensions::ext_tool_start,
            commands::extensions::ext_launch,
            commands::extensions::ext_open_link,
            commands::extensions::open_external,
            commands::files::text_file_open,
            commands::files::text_file_save_as,
            commands::extras::app_update_check,
            commands::extras::modio_has_key,
            commands::extras::modio_set_key,
            commands::extras::modio_clear_key,
            commands::extras::modio_browse,
            commands::extras::creator_check,
            commands::extras::creator_package,
            commands::extras::creator_publish,
            commands::existing::existing_scan,
            commands::existing::existing_track,
            commands::existing::groups_list,
            commands::existing::groups_set,
            commands::existing::group_create,
            commands::existing::group_rename,
            commands::existing::group_delete,
            commands::existing::library_bulk,
            commands::game::game_launch,
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
            commands::library::nxm_handler_test,
            commands::library::downloads_usage,
            commands::library::downloads_cleanup,
            commands::creator::creator_my_mods,
            commands::creator::creator_mod_files,
            commands::creator::creator_upload_targets,
            commands::creator::creator_pin,
            commands::creator::creator_unpin,
            commands::troubleshoot::logs_scan,
            commands::troubleshoot::logs_report,
            commands::troubleshoot::log_open,
            commands::troubleshoot::archive_overlaps,
            commands::troubleshoot::archive_load_first,
            commands::troubleshoot::archive_rename_undo,
            commands::troubleshoot::modlist_disable,
            commands::troubleshoot::rules_list,
            commands::troubleshoot::rule_forget,
            commands::troubleshoot::fomod_options,
            commands::troubleshoot::fomod_change,
            commands::troubleshoot::library_move_check,
            commands::troubleshoot::library_move,
            commands::troubleshoot::game_folder_set,
            commands::troubleshoot::game_relink,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Mods");
}
