//! The Tauri shell: a thin layer that forwards commands to `pocket_core::app::App`, shows Nexus pages in an
//! in-app window (never the external browser), catches nxm:// links, and keeps the app to one instance.

use pocket_core::app::{App, InstallMeta, Settings, Shell};
use pocket_core::collections::Collection;
use pocket_core::nexus::BrowseQuery;
use pocket_core::{Error, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State, Url, WebviewUrl, WebviewWindowBuilder};

type AppState<'a> = State<'a, Arc<App>>;
type R<T> = std::result::Result<T, Error>;

const NEXUS_WINDOW: &str = "nexus";

struct TauriShell {
    handle: AppHandle,
}

impl Shell for TauriShell {
    fn open_nexus_page(&self, url: &str, title: &str) {
        let handle = self.handle.clone();
        let (url, title) = (url.to_string(), title.to_string());
        tauri::async_runtime::spawn(async move {
            if let Err(e) = open_nexus_window(&handle, &url, &title) {
                let _ = handle.emit("error", json!({ "message": format!("Couldn't open the Nexus page: {e}") }));
            }
        });
    }
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.handle.emit(event, payload);
    }
}

fn shell(handle: &AppHandle) -> Arc<dyn Shell> {
    Arc::new(TauriShell { handle: handle.clone() })
}

fn is_nexus(url: &Url) -> bool {
    url.host_str().is_some_and(|h| h == "nexusmods.com" || h.ends_with(".nexusmods.com"))
}

/// Opens (or reuses) the in-app Nexus window. Navigations to nxm:// are caught here and handed to the queue;
/// links that would open a new window stay in this one when they're on Nexus, and are dropped otherwise, so the
/// person is never sent to an external browser.
/// UNVERIFIED: that WebView2 reports the site's nxm:// navigation through on_navigation. If it doesn't, the
/// per-user nxm:// handler still delivers the link through the single-instance callback.
fn open_nexus_window(handle: &AppHandle, url: &str, title: &str) -> tauri::Result<()> {
    let target: Url = url.parse().map_err(tauri::Error::InvalidUrl)?;
    if let Some(w) = handle.get_webview_window(NEXUS_WINDOW) {
        w.navigate(target)?;
        w.set_title(title)?;
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    let nav_handle = handle.clone();
    let popup_handle = handle.clone();
    WebviewWindowBuilder::new(handle, NEXUS_WINDOW, WebviewUrl::External(target))
        .title(title)
        .inner_size(1120.0, 820.0)
        .min_inner_size(720.0, 520.0)
        .on_navigation(move |u: &Url| {
            if u.scheme() == "nxm" {
                deliver_nxm(&nav_handle, u.as_str());
                return false;
            }
            matches!(u.scheme(), "https" | "about" | "data" | "blob")
        })
        .on_new_window(move |u, _features| {
            if u.scheme() == "nxm" {
                deliver_nxm(&popup_handle, u.as_str());
            } else if is_nexus(&u) {
                if let Some(w) = popup_handle.get_webview_window(NEXUS_WINDOW) {
                    let _ = w.navigate(u);
                }
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}

/// Hands an nxm:// link to the core, from any source (in-app page, browser handler, second instance).
fn deliver_nxm(handle: &AppHandle, link: &str) {
    let handle = handle.clone();
    let link = link.to_string();
    tauri::async_runtime::spawn(async move {
        let app = handle.state::<Arc<App>>().inner().clone();
        let sh = shell(&handle);
        match app.handle_nxm(&link, sh).await {
            Ok(v) => {
                let _ = handle.emit("nxm", v);
            }
            Err(e) => {
                let _ = handle.emit("error", json!({ "message": e.to_string() }));
            }
        }
        if let Some(main) = handle.get_webview_window("main") {
            let _ = main.set_focus();
        }
    });
}

fn nxm_from_args(args: &[String]) -> Option<String> {
    args.iter().find(|a| a.trim_matches('"').to_ascii_lowercase().starts_with("nxm://")).map(|a| a.trim_matches('"').to_string())
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Other(e.to_string()))?
}

// status and settings ---------------------------------------------------------------------------------------

#[tauri::command]
fn status(app: AppState) -> R<pocket_core::app::AppStatus> {
    app.status()
}

#[tauri::command]
fn settings_get(app: AppState) -> Settings {
    app.settings()
}

#[tauri::command]
fn settings_save(app: AppState, settings: Settings) -> R<Settings> {
    app.save_settings(settings)
}

#[tauri::command]
async fn set_api_key(app: AppState<'_>, key: String) -> R<pocket_core::nexus::Account> {
    app.set_api_key(&key).await
}

#[tauri::command]
fn clear_api_key(app: AppState) -> R<()> {
    app.clear_api_key()
}

#[tauri::command]
async fn refresh_account(app: AppState<'_>) -> R<pocket_core::nexus::Account> {
    app.refresh_account().await
}

#[tauri::command]
async fn nexus_probe(app: AppState<'_>) -> R<Value> {
    app.probe().await
}

#[tauri::command]
fn detect_games(app: AppState) -> Vec<pocket_core::game::GameInstall> {
    app.detect_games()
}

#[tauri::command]
fn set_game_root(app: AppState, path: PathBuf) -> R<pocket_core::game::GameInstall> {
    app.set_game_root(&path)
}

// browse ----------------------------------------------------------------------------------------------------

#[tauri::command]
async fn browse(app: AppState<'_>, query: BrowseQuery) -> R<pocket_core::nexus::ModPage> {
    app.browse(&query).await
}

#[tauri::command]
async fn categories(app: AppState<'_>) -> R<Vec<pocket_core::nexus::Category>> {
    app.categories().await
}

#[tauri::command]
async fn mod_detail(app: AppState<'_>, mod_id: u64) -> R<pocket_core::nexus::ModDetail> {
    app.mod_detail(mod_id).await
}

#[tauri::command]
fn set_hidden(app: AppState, mod_id: u64, hidden: bool) -> R<()> {
    app.set_hidden(mod_id, hidden)
}

#[tauri::command]
async fn endorse(app: AppState<'_>, mod_id: u64, endorse: bool) -> R<()> {
    app.endorse(mod_id, endorse).await
}

#[tauri::command]
async fn set_tracked(app: AppState<'_>, mod_id: u64, tracked: bool) -> R<()> {
    app.set_tracked(mod_id, tracked).await
}

#[tauri::command]
async fn check_updates(app: AppState<'_>) -> R<usize> {
    app.check_updates().await
}

/// Opens a mod's Nexus page inside the app.
#[tauri::command]
fn open_nexus(handle: AppHandle, mod_id: u64, file_id: Option<u64>) {
    let url = match file_id {
        Some(f) => App::file_page_url(mod_id, f),
        None => App::mod_page_url(mod_id),
    };
    shell(&handle).open_nexus_page(&url, "Nexus Mods");
}

// queue -----------------------------------------------------------------------------------------------------

#[tauri::command]
fn queue_view(app: AppState) -> pocket_core::app::QueueView {
    app.queue_view()
}

#[tauri::command]
async fn queue_mod(handle: AppHandle, app: AppState<'_>, mod_id: u64, file_id: Option<u64>) -> R<Vec<u64>> {
    let ids = app.queue_mod(mod_id, file_id, "browse").await?;
    app.inner().pump(&shell(&handle));
    Ok(ids)
}

#[tauri::command]
async fn queue_updates(handle: AppHandle, app: AppState<'_>) -> R<usize> {
    let mods = app.library()?;
    let mut n = 0;
    for row in mods {
        if let (Some(id), Some(u)) = (row.m.mod_id, row.m.update_available.as_ref()) {
            app.queue_mod(id, Some(u.file_id), "update").await?;
            n += 1;
        }
    }
    app.inner().pump(&shell(&handle));
    Ok(n)
}

#[tauri::command]
fn queue_action(handle: AppHandle, app: AppState, action: String, id: Option<u64>) {
    match (action.as_str(), id) {
        ("retry", Some(id)) => app.queue_retry(id),
        ("skip", Some(id)) => app.queue_skip(id),
        ("remove", Some(id)) => app.queue_remove(id),
        ("clear", _) => app.queue_clear_finished(),
        ("pause", _) => app.queue_pause(true),
        ("resume", _) => app.queue_pause(false),
        // re-open the page the queue is waiting on
        ("open", Some(id)) => {
            if let Some(item) = app.queue_view().items.into_iter().find(|i| i.id == id) {
                shell(&handle).open_nexus_page(&App::file_page_url(item.mod_id, item.file_id), &item.mod_name);
            }
        }
        _ => {}
    }
    app.inner().pump(&shell(&handle));
}

#[tauri::command]
async fn handle_nxm(handle: AppHandle, app: AppState<'_>, link: String) -> R<Value> {
    app.inner().handle_nxm(&link, shell(&handle)).await
}

// library and profiles --------------------------------------------------------------------------------------

#[tauri::command]
fn library(app: AppState) -> R<Vec<pocket_core::app::LibraryRow>> {
    app.library()
}

#[tauri::command]
async fn install_file(app: AppState<'_>, path: PathBuf) -> R<pocket_core::app::InstallReport> {
    let a = app.inner().clone();
    blocking(move || a.install_archive(&path, InstallMeta::default())).await
}

#[tauri::command]
async fn set_enabled(app: AppState<'_>, key: String, enabled: bool) -> R<()> {
    let a = app.inner().clone();
    blocking(move || a.set_enabled(&key, enabled)).await
}

#[tauri::command]
async fn remove_mod(app: AppState<'_>, key: String) -> R<()> {
    let a = app.inner().clone();
    blocking(move || a.remove_mod(&key)).await
}

#[tauri::command]
async fn redeploy(app: AppState<'_>, key: String) -> R<()> {
    let a = app.inner().clone();
    blocking(move || a.redeploy(&key)).await
}

#[tauri::command]
fn profiles(app: AppState) -> R<Value> {
    let (list, active) = app.profiles()?;
    Ok(json!({ "profiles": list, "active": active }))
}

#[tauri::command]
fn create_profile(app: AppState, name: String, copy_current: bool) -> R<()> {
    app.create_profile(&name, copy_current)
}

#[tauri::command]
fn delete_profile(app: AppState, name: String) -> R<()> {
    app.delete_profile(&name)
}

#[tauri::command]
async fn switch_profile(app: AppState<'_>, name: String) -> R<()> {
    let a = app.inner().clone();
    blocking(move || a.switch_profile(&name)).await
}

// load order and health -------------------------------------------------------------------------------------

#[tauri::command]
fn load_order(app: AppState) -> R<pocket_core::app::LoadOrderView> {
    app.load_order()
}

#[tauri::command]
fn set_load_order(app: AppState, order: Vec<String>) -> R<()> {
    app.set_load_order(&order)
}

#[tauri::command]
async fn redmod_deploy(app: AppState<'_>) -> R<pocket_core::redmod::DeployOutcome> {
    app.redmod_deploy().await
}

#[tauri::command]
fn health(app: AppState) -> R<pocket_core::health::HealthReport> {
    app.health()
}

// collections -----------------------------------------------------------------------------------------------

#[tauri::command]
async fn collections_browse(app: AppState<'_>, text: Option<String>, matching_version: bool, sort: String, offset: u32) -> R<Value> {
    let (cards, total) = app.collections_browse(text.as_deref(), matching_version, &sort, offset).await?;
    Ok(json!({ "collections": cards, "total": total }))
}

#[tauri::command]
async fn collection_fetch(app: AppState<'_>, slug: String, revision: Option<u64>, include_optional: bool) -> R<Value> {
    let c = app.fetch_collection(&slug, revision).await?;
    let check = app.collection_precheck(&c, include_optional)?;
    Ok(json!({ "collection": c, "check": check }))
}

#[tauri::command]
fn collection_precheck(app: AppState, collection: Collection, include_optional: bool) -> R<pocket_core::collections::PreCheck> {
    app.collection_precheck(&collection, include_optional)
}

#[tauri::command]
fn collection_install(handle: AppHandle, app: AppState, collection: Collection, slug: String, include_optional: bool) -> R<Vec<u64>> {
    let ids = app.collection_install(&collection, &slug, include_optional)?;
    app.inner().pump(&shell(&handle));
    Ok(ids)
}

#[tauri::command]
fn collection_import(app: AppState, path: PathBuf, include_optional: bool) -> R<Value> {
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io_at(e, &path))?;
    let c = pocket_core::collections::parse_manifest(&text)?;
    let check = app.collection_precheck(&c, include_optional)?;
    Ok(json!({ "collection": c, "check": check }))
}

#[tauri::command]
fn collection_export(app: AppState, name: String, author: String, description: String, keys: Vec<String>, path: Option<PathBuf>) -> R<Collection> {
    let c = app.collection_export(&name, &author, &description, &keys)?;
    if let Some(p) = path {
        std::fs::write(&p, serde_json::to_vec_pretty(&c)?).map_err(|e| Error::io_at(e, &p))?;
    }
    Ok(c)
}

#[tauri::command]
fn saved_collections(app: AppState) -> R<Value> {
    Ok(json!(app.saved_collections()?.into_iter().map(|(name, data)| json!({ "name": name, "collection": data })).collect::<Vec<_>>()))
}

#[tauri::command]
fn delete_saved_collection(app: AppState, name: String) -> R<()> {
    app.delete_saved_collection(&name)
}

/// Experiment flag only: publishing to Nexus from a third-party app is unconfirmed, so this explains rather
/// than calls an API it can't be sure exists.
#[tauri::command]
fn collection_publish(app: AppState) -> R<()> {
    if !app.settings().experiments.publish_collections {
        return Err(Error::Invalid("Publishing is an experiment; turn it on in Settings first.".into()));
    }
    Err(Error::Other(
        "Publishing to Nexus isn't available: it's unconfirmed that a third-party app may create collections. Export the file and \
         publish it from Vortex or the Nexus site for now."
            .into(),
    ))
}

// mod.io (experiment) ---------------------------------------------------------------------------------------

#[tauri::command]
async fn modio_browse(app: AppState<'_>, text: String, offset: u32) -> R<Value> {
    let (mods, total) = app.modio_browse(&text, offset).await?;
    Ok(json!({ "mods": mods, "total": total }))
}

#[tauri::command]
fn modio_set_key(key: String) -> R<()> {
    if key.trim().is_empty() {
        pocket_core::secrets::delete(pocket_core::secrets::MODIO_KEY_TARGET)
    } else {
        pocket_core::secrets::set(pocket_core::secrets::MODIO_KEY_TARGET, &key)
    }
}

// nxm handler and app updates -------------------------------------------------------------------------------

fn our_exe() -> R<PathBuf> {
    std::env::current_exe().map_err(Error::from)
}

#[tauri::command]
fn nxm_status() -> R<pocket_core::nxm_handler::HandlerStatus> {
    Ok(pocket_core::nxm_handler::status(&our_exe()?))
}

#[tauri::command]
fn nxm_register() -> R<pocket_core::nxm_handler::HandlerStatus> {
    let exe = our_exe()?;
    pocket_core::nxm_handler::register(&exe)?;
    Ok(pocket_core::nxm_handler::status(&exe))
}

#[tauri::command]
fn nxm_restore() -> R<pocket_core::nxm_handler::HandlerStatus> {
    pocket_core::nxm_handler::restore_previous()?;
    Ok(pocket_core::nxm_handler::status(&our_exe()?))
}

#[tauri::command]
async fn app_update_check() -> R<Option<pocket_core::appupdate::Release>> {
    pocket_core::appupdate::check_latest().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // must be first: a second launch (the browser opening an nxm link) hands its arguments to this one
        .plugin(tauri_plugin_single_instance::init(|handle, argv, _cwd| {
            if let Some(link) = nxm_from_args(&argv) {
                deliver_nxm(handle, &link);
            }
            if let Some(main) = handle.get_webview_window("main") {
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let core = Arc::new(App::open(&data_dir).map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?);
            app.manage(core.clone());
            let handle = app.handle().clone();
            // first run: take the nxm:// handler only when nobody holds it; otherwise Settings offers to take it back
            if let Ok(exe) = our_exe() {
                let st = pocket_core::nxm_handler::status(&exe);
                if st.supported && st.command.is_none() {
                    let _ = pocket_core::nxm_handler::register(&exe);
                }
            }
            // a link that launched this first instance
            let args: Vec<String> = std::env::args().collect();
            if let Some(link) = nxm_from_args(&args) {
                deliver_nxm(&handle, &link);
            }
            // resume a queue left from last time
            let sh = shell(&handle);
            tauri::async_runtime::spawn(async move { core.pump(&sh) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            settings_get,
            settings_save,
            set_api_key,
            clear_api_key,
            refresh_account,
            nexus_probe,
            detect_games,
            set_game_root,
            browse,
            categories,
            mod_detail,
            set_hidden,
            endorse,
            set_tracked,
            check_updates,
            open_nexus,
            queue_view,
            queue_mod,
            queue_updates,
            queue_action,
            handle_nxm,
            library,
            install_file,
            set_enabled,
            remove_mod,
            redeploy,
            profiles,
            create_profile,
            delete_profile,
            switch_profile,
            load_order,
            set_load_order,
            redmod_deploy,
            health,
            collections_browse,
            collection_fetch,
            collection_precheck,
            collection_install,
            collection_import,
            collection_export,
            saved_collections,
            delete_saved_collection,
            collection_publish,
            modio_browse,
            modio_set_key,
            nxm_status,
            nxm_register,
            nxm_restore,
            app_update_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pocket Mods");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nxm_in_args() {
        let args = vec!["Pocket Mods.exe".to_string(), "\"nxm://cyberpunk2077/mods/1/files/2?key=a&expires=1\"".to_string()];
        assert_eq!(nxm_from_args(&args).as_deref(), Some("nxm://cyberpunk2077/mods/1/files/2?key=a&expires=1"));
        assert_eq!(nxm_from_args(&["x".into()]), None);
    }

    #[test]
    fn nexus_hosts() {
        assert!(is_nexus(&"https://www.nexusmods.com/cyberpunk2077".parse().unwrap()));
        assert!(is_nexus(&"https://users.nexusmods.com/".parse().unwrap()));
        assert!(!is_nexus(&"https://evilnexusmods.com/".parse().unwrap()));
    }
}
