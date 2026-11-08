//! Tauri shell: window setup and commands. All Nexus and file logic lives in `pocket-core`.

mod background;
mod browser;
mod commands;
mod icon;
mod power;
mod tray;
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
            // started again (the Start menu, a shortcut): show the window, made again if it was closed to the tray;
            // a link from the browser does not need the window
            if !argv.iter().any(|a| a.starts_with("nxm://")) { tray::show_main(app); }
            for a in argv.into_iter().filter(|a| a.starts_with("nxm://")) {
                let h = app.clone();
                tauri::async_runtime::spawn(async move { runner::handle_nxm(&h, &a).await });
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(power::Power::default())
        .manage(browser::Browser::default())
        .manage(background::Background::default())
        .manage(icon::Current::default())
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
            // the tray, the quiet checks, and no window when started in the background (after an update, at sign-in)
            if let Err(e) = tray::install(app.handle()) { eprintln!("tray: {e}"); }
            background::start(app.handle().clone());
            if background::started_hidden() { if let Some(w) = app.get_webview_window("main") { let _ = w.destroy(); } }
            // half-finished extractions from a crash (<library>/staging/*.new) are never used again, and removed mods
            // past their time go for good; off the main thread, so a big folder never holds the window back
            let db = app.state::<state::AppState>().db.clone();
            std::thread::spawn(move || {
                let lib = state::Paths::load(&db.lock().unwrap()).require().ok().map(|(_, l)| l);
                for e in lib.iter().flat_map(|l| std::fs::read_dir(l.join("staging")).into_iter().flatten().flatten()) {
                    if e.path().extension().is_some_and(|x| x == "new") { let _ = std::fs::remove_dir_all(e.path()); }
                }
                let _ = pocket_core::install::purge_trash(&db.lock().unwrap(), None);
            });
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
                    let db = st.db.clone();
                    let r = tauri::async_runtime::spawn_blocking(move || pocket_core::launch::vanilla_restore(&db.lock().unwrap(), &g, &l, &p.deploy_options())).await
                        .unwrap_or_else(|e| Err(pocket_core::Error::Invalid(e.to_string())));
                    commands::game::report_restore(&h, r);
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
            icon::app_icon,
            background::background_state,
            background::background_check,
            background::app_update_install,
            background::autostart_get,
            background::autostart_set,
            commands::browser::browser_bounds,
            commands::browser::browser_nav,
            commands::browser::browser_go,
            commands::browser::browser_state,
            commands::browser::browser_close,
            commands::browser::browser_pop,
            commands::browser::browser_external,
            commands::browser::browser_extensions_folder,
            commands::browser::browser_sign_in,
            commands::browser::browser_sign_out,
            commands::nexus::nexus_status,
            commands::nexus::nexus_set_key,
            commands::nexus::nexus_clear_key,
            commands::nexus::nexus_sign_in,
            commands::nexus::nexus_sign_in_cancel,
            commands::nexus::rate_limit,
            commands::browse::browse_search,
            commands::browse::browse_detail,
            commands::browse::link_preview,
            commands::browse::open_link,
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
            commands::collections::collection_detail,
            commands::collections::collection_install,
            commands::collections::local_collection_create,
            commands::collections::local_collection_export,
            commands::collections::local_collection_import,
            commands::library::library_identify,
            commands::library::queue_cancel,
            commands::nexus::mod_endorse,
            commands::nexus::endorse_status,
            commands::nexus::endorse_many,
            commands::nexus::endorse_decline,
            commands::nexus::endorse_mark,
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
            commands::existing::vortex_profiles_scan,
            commands::existing::unmanaged_list,
            commands::existing::vortex_profiles_import,
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
            commands::game::profile_memberships,
            commands::game::profile_mod_set,
            commands::game::load_order_set,
            commands::game::updates_check,
            commands::game::updates_last,
            commands::library::queue_remove,
            commands::library::queue_retry,
            commands::library::queue_clear_finished,
            commands::library::queue_show_page,
            commands::library::queue_start,
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
        .build(tauri::generate_context!())
        .expect("error while building Pockymod")
        .run(|app, event| {
            // the last window closing (to the tray) does not end the app; Quit does
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                if background::enabled(app) && !app.state::<background::Background>().quitting.load(std::sync::atomic::Ordering::Relaxed) { api.prevent_exit(); }
            }
        });
}
