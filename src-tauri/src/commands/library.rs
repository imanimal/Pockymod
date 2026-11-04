//! Library, install, queue, paths and nxm handler commands.
use crate::runner::{self, QueueSnapshot};
use crate::state::{AppState, Paths};
use pocket_core::deploy::{self, DeployOptions, DeployOutcome, UndeployReport};
use pocket_core::install::{self, InstallOutcome, InstallRequest, InstallRow};
use pocket_core::platform::{self, HandlerStatus};
use pocket_core::queue::{self, QueueItem};
use pocket_core::{Error, Result};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsView {
    pub game_dir: Option<PathBuf>,
    pub library_dir: Option<PathBuf>,
    pub default_library: Option<PathBuf>,
    pub allow_copy: bool,
    pub game_looks_right: bool,
    pub same_volume: Option<bool>,
    pub in_program_files: bool,
}

#[tauri::command]
pub fn paths_get(state: State<'_, AppState>) -> PathsView {
    let p = Paths::load(&state.db.lock().unwrap());
    let game_looks_right = p.game_dir.as_ref().is_some_and(|g| g.join("bin/x64/Cyberpunk2077.exe").exists() || g.join("bin/x64").exists());
    let same_volume = match (&p.game_dir, &p.library_dir) { (Some(g), Some(l)) => Some(deploy::same_volume(g, l)), _ => None };
    let in_program_files = p.game_dir.as_ref().is_some_and(|g| g.to_string_lossy().to_lowercase().contains("program files"));
    PathsView { default_library: p.game_dir.as_deref().map(install::default_library), game_dir: p.game_dir, library_dir: p.library_dir, allow_copy: p.allow_copy, game_looks_right, same_volume, in_program_files }
}

#[tauri::command]
pub fn paths_set(state: State<'_, AppState>, game_dir: Option<String>, library_dir: Option<String>, allow_copy: Option<bool>) -> Result<PathsView> {
    {
        let db = state.db.lock().unwrap();
        if let Some(g) = game_dir { db.set_setting("game.dir", g.trim())?; }
        if let Some(l) = library_dir { db.set_setting("library.dir", l.trim())?; }
        if let Some(c) = allow_copy { db.set_setting("deploy.allowCopy", if c { "1" } else { "0" })?; }
    }
    Ok(paths_get(state))
}

#[tauri::command]
pub fn library_list(state: State<'_, AppState>) -> Result<Vec<InstallRow>> { install::list_installs(&state.db.lock().unwrap()) }

#[tauri::command]
pub async fn library_uninstall(app: AppHandle, state: State<'_, AppState>, install_id: i64) -> Result<UndeployReport> {
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::uninstall(&db.lock().unwrap(), &game, install_id)).await.map_err(|e| Error::Invalid(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

/// Deploy a staged install after the user decided on conflicts or the copy fallback.
#[tauri::command]
pub async fn library_deploy(app: AppHandle, state: State<'_, AppState>, install_id: i64, options: DeployOptions) -> Result<DeployOutcome> {
    let (game, library) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::redeploy(&db.lock().unwrap(), &game, &library, install_id, &options)).await.map_err(|e| Error::Invalid(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

/// Install an archive from disk: a manual download, or a queued download that needed review.
#[tauri::command]
pub async fn library_install_file(app: AppHandle, state: State<'_, AppState>, request: InstallRequest) -> Result<InstallOutcome> {
    let paths = Paths::load(&state.db.lock().unwrap());
    let (game, library) = paths.require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &game, &library, &request)).await.map_err(|e| Error::Invalid(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRow { pub mod_id: u32, pub file_id: u32, pub name: String, pub version: String, pub path: String, pub size: i64, pub md5: String, pub installed: bool }

#[tauri::command]
pub fn library_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadRow>> {
    let db = state.db.lock().unwrap();
    let mut st = db.conn.prepare("SELECT d.mod_id, d.file_id, d.name, d.version, d.path, d.size, d.md5, EXISTS(SELECT 1 FROM installs i WHERE i.mod_id = d.mod_id AND i.file_id = d.file_id) FROM downloads d ORDER BY d.downloaded_at DESC")?;
    let rows = st.query_map([], |r| Ok(DownloadRow { mod_id: r.get(0)?, file_id: r.get(1)?, name: r.get(2)?, version: r.get(3)?, path: r.get(4)?, size: r.get(5)?, md5: r.get(6)?, installed: r.get(7)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

// ── queue ──

#[tauri::command]
pub fn queue_get(app: AppHandle) -> QueueSnapshot { runner::snapshot(&app) }

/// The files a mod install needs (the mod and its missing requirements), without queueing them: the cost up front.
#[tauri::command]
pub async fn queue_plan(state: State<'_, AppState>, mod_id: u32, file_id: u32) -> Result<Vec<QueueItem>> {
    let already = installed_or_present(&state)?;
    queue::with_dependencies(&state.nexus, &state.db, mod_id, file_id, &already).await
}

#[tauri::command]
pub async fn queue_add(app: AppHandle, state: State<'_, AppState>, mod_id: u32, file_id: u32, with_dependencies: bool) -> Result<QueueSnapshot> {
    let already = installed_or_present(&state)?;
    let mut items = queue::with_dependencies(&state.nexus, &state.db, mod_id, file_id, &already).await?;
    if !with_dependencies { items.retain(|i| i.mod_id == mod_id); }
    {
        let premium = state.user.read().await.as_ref().is_some_and(|u| u.is_premium);
        let mut q = state.queue.lock().unwrap();
        q.premium = premium;
        for i in items { q.add(i); }
    }
    state.pump.notify_one();
    runner::emit(&app);
    Ok(runner::snapshot(&app))
}

/// Queue a mod's newest main file (with its missing requirements): Health's install and update fixes.
#[tauri::command]
pub async fn queue_add_mod(app: AppHandle, state: State<'_, AppState>, mod_id: u32) -> Result<QueueSnapshot> {
    let d = pocket_core::browse::detail(&state.nexus, &state.db, mod_id, None).await?;
    let file = pocket_core::browse::main_file(&d.files).ok_or_else(|| Error::Invalid(format!("{} has no current file on Nexus.", d.detail.name)))?.file_id;
    queue_add(app, state, mod_id, file, true).await
}

#[tauri::command]
pub fn queue_remove(app: AppHandle, state: State<'_, AppState>, id: u64) -> QueueSnapshot { state.queue.lock().unwrap().remove(id); state.pump.notify_one(); runner::emit(&app); runner::snapshot(&app) }

#[tauri::command]
pub fn queue_retry(app: AppHandle, state: State<'_, AppState>, id: u64) -> QueueSnapshot { state.queue.lock().unwrap().retry(id); state.pump.notify_one(); runner::snapshot(&app) }

#[tauri::command]
pub fn queue_clear_finished(app: AppHandle, state: State<'_, AppState>) -> QueueSnapshot { state.queue.lock().unwrap().clear_finished(); runner::emit(&app); runner::snapshot(&app) }

/// Show the Nexus page of the item waiting for a click again (the window was closed).
#[tauri::command]
pub fn queue_show_page(app: AppHandle, state: State<'_, AppState>, id: u64) -> Result<()> {
    let item = state.queue.lock().unwrap().get(id).cloned().ok_or_else(|| Error::Invalid("That download is no longer queued.".into()))?;
    runner::open_nexus(&app, &runner::file_page(item.mod_id, (!item.manual_only).then_some(item.file_id))).map_err(|e| Error::Invalid(e.to_string()))
}

#[tauri::command]
pub fn open_nexus_page(app: AppHandle, mod_id: u32, file_id: Option<u32>) -> Result<()> {
    runner::open_nexus(&app, &runner::file_page(mod_id, file_id)).map_err(|e| Error::Invalid(e.to_string()))
}

/// Pages on nexusmods.com the app links to (account settings, API keys), opened in the Nexus window.
#[tauri::command]
pub fn open_nexus_url(app: AppHandle, url: String) -> Result<()> {
    let ok = url.starts_with("https://www.nexusmods.com/") || url.starts_with("https://next.nexusmods.com/") || url.starts_with("https://users.nexusmods.com/");
    if !ok { return Err(Error::Invalid("Only Nexus Mods pages open in the Nexus window.".into())); }
    runner::open_nexus(&app, &url).map_err(|e| Error::Invalid(e.to_string()))
}

fn installed_or_present(state: &State<'_, AppState>) -> Result<std::collections::BTreeSet<u32>> {
    let mut ids = install::installed_mod_ids(&state.db.lock().unwrap())?;
    // frameworks found in the game folder count as installed even when another manager put them there
    let paths = Paths::load(&state.db.lock().unwrap());
    if let Some(g) = paths.game_dir {
        for f in pocket_core::cyberpunk::frameworks::FRAMEWORKS {
            if f.files.iter().any(|p| g.join(p).exists()) { ids.insert(f.nexus_mod_id); }
        }
    }
    Ok(ids)
}

// ── nxm:// handler ──

#[tauri::command]
pub fn nxm_handler_status() -> Result<HandlerStatus> { platform::nxm_handler_status(&std::env::current_exe()?) }

#[tauri::command]
pub fn nxm_handler_register() -> Result<HandlerStatus> { platform::register_nxm_handler(&std::env::current_exe()?) }
