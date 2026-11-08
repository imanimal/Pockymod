//! Library, install, queue, paths and nxm handler commands.
use crate::runner::{self, QueueSnapshot};
use crate::state::{AppState, Paths};
use pocket_core::deploy::{self, DeployOptions, DeployOutcome, UndeployReport};
use pocket_core::install::{self, InstallOutcome, InstallRequest, InstallRow};
use pocket_core::platform::{self, HandlerStatus};
use pocket_core::queue::{self, ItemState, QueueItem};
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
pub async fn paths_get(state: State<'_, AppState>) -> Result<PathsView> {
    let p = Paths::load(&state.db.lock().unwrap());
    let game_looks_right = p.game_dir.as_ref().is_some_and(|g| g.join("bin/x64/Cyberpunk2077.exe").exists() || g.join("bin/x64").exists());
    let same_volume = match (&p.game_dir, &p.library_dir) { (Some(g), Some(l)) => Some(deploy::same_volume(g, l)), _ => None };
    let in_program_files = p.game_dir.as_ref().is_some_and(|g| g.to_string_lossy().to_lowercase().contains("program files"));
    Ok(PathsView { default_library: p.game_dir.as_deref().map(install::default_library), game_dir: p.game_dir, library_dir: p.library_dir, allow_copy: p.allow_copy, game_looks_right, same_volume, in_program_files })
}

#[tauri::command]
pub async fn paths_set(state: State<'_, AppState>, game_dir: Option<String>, library_dir: Option<String>, allow_copy: Option<bool>) -> Result<PathsView> {
    {
        let db = state.db.lock().unwrap();
        if let Some(g) = game_dir { db.set_setting("game.dir", g.trim())?; }
        if let Some(l) = library_dir { db.set_setting("library.dir", l.trim())?; }
        if let Some(c) = allow_copy { db.set_setting("deploy.allowCopy", if c { "1" } else { "0" })?; }
    }
    paths_get(state).await
}

#[tauri::command]
pub async fn library_list(state: State<'_, AppState>) -> Result<Vec<InstallRow>> { install::list_installs(&state.db.lock().unwrap()) }

#[tauri::command]
pub async fn library_uninstall(app: AppHandle, state: State<'_, AppState>, install_id: i64) -> Result<UndeployReport> {
    crate::commands::not_while_running()?;
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::uninstall(&db.lock().unwrap(), &game, install_id)).await.map_err(|e| Error::Invalid(e.to_string()))?;
    // also after a failure: part of it may have happened
    let _ = app.emit("library-changed", ());
    r
}

/// Deploy a staged install after the user decided on conflicts or the copy fallback.
#[tauri::command]
pub async fn library_deploy(app: AppHandle, state: State<'_, AppState>, install_id: i64, options: DeployOptions) -> Result<DeployOutcome> {
    crate::commands::not_while_running()?;
    let (game, library) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::redeploy_any(&db.lock().unwrap(), &game, &library, install_id, &options)).await.map_err(|e| Error::Invalid(e.to_string()))?;
    // also after a failure: part of it may have happened
    let _ = app.emit("library-changed", ());
    r
}

/// Install an archive from disk: a manual download, or a queued download that needed review.
#[tauri::command]
pub async fn library_install_file(app: AppHandle, state: State<'_, AppState>, mut request: InstallRequest) -> Result<InstallOutcome> {
    // the game locks its files (as for the queue's installs)
    crate::commands::not_while_running()?;
    if request.replace_only.is_none() { request.replace_only = Some(runner::replaced_by_file(&state.db.lock().unwrap(), request.mod_id, request.file_id, request.file_name.as_deref())); }
    let paths = Paths::load(&state.db.lock().unwrap());
    let (game, library) = paths.require()?;
    let db = state.db.clone();
    let r = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &game, &library, &request)).await.map_err(|e| Error::Invalid(e.to_string()))?;
    // also after a failure: part of it may have happened
    let _ = app.emit("library-changed", ());
    r
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRow { pub mod_id: u32, pub file_id: u32, pub name: String, pub version: String, pub path: String, pub size: i64, pub md5: String, pub installed: bool }

#[tauri::command]
pub async fn library_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadRow>> {
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
    let already = installed_or_present(&state).await?;
    queue::with_dependencies(&state.nexus, &state.db, mod_id, file_id, &already).await
}

#[tauri::command]
pub async fn queue_add(app: AppHandle, state: State<'_, AppState>, mod_id: u32, file_id: u32, with_dependencies: bool) -> Result<QueueSnapshot> {
    let already = installed_or_present(&state).await?;
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
    // an update follows the installed file (2K stays 2K); a missing mod gets its main file
    let have: Option<(u32, Option<String>)> = state.db.lock().unwrap().conn.query_row(
        "SELECT file_id, file_name FROM installs WHERE mod_id = ?1 ORDER BY installed_at DESC LIMIT 1", [mod_id], |r| Ok((r.get(0)?, r.get(1)?))).ok();
    let next = have.as_ref().and_then(|(fid, name)| pocket_core::browse::successor(&d.files, *fid, name.as_deref()));
    let file = next.or_else(|| pocket_core::browse::main_file(&d.files)).ok_or_else(|| Error::Invalid(format!("{} has no current file on Nexus", d.detail.name)))?.file_id;
    queue_add(app, state, mod_id, file, true).await
}

#[tauri::command]
pub fn queue_remove(app: AppHandle, state: State<'_, AppState>, id: u64) -> QueueSnapshot { state.queue.lock().unwrap().remove(id); state.pump.notify_one(); runner::emit(&app); runner::snapshot(&app) }

#[tauri::command]
pub fn queue_retry(app: AppHandle, state: State<'_, AppState>, id: u64) -> QueueSnapshot { state.queue.lock().unwrap().retry(id); state.pump.notify_one(); runner::snapshot(&app) }

#[tauri::command]
pub fn queue_clear_finished(app: AppHandle, state: State<'_, AppState>) -> QueueSnapshot { state.queue.lock().unwrap().clear_finished(); runner::emit(&app); runner::snapshot(&app) }

/// Show the Nexus page of the item waiting for a click again (the window was closed).
// Commands that open a window are async: on Windows a window made from a synchronous command never finishes loading
// (WebView2 needs the main thread the command would be blocking), which shows as a blank white window.
#[tauri::command]
pub async fn queue_show_page(app: AppHandle, state: State<'_, AppState>, id: u64) -> Result<()> {
    let item = state.queue.lock().unwrap().get(id).cloned().ok_or_else(|| Error::Invalid("That download is no longer queued".into()))?;
    runner::open_nexus(&app, &runner::file_page(item.mod_id, (!item.manual_only).then_some(item.file_id)))
}

/// The user picked a queued item: it goes next. A free account's page for it opens now (a download already running
/// carries on, and the link caught there waits its turn); Premium fetches it once the current one is done.
#[tauri::command]
pub async fn queue_start(app: AppHandle, state: State<'_, AppState>, id: u64) -> Result<QueueSnapshot> {
    let click = {
        let mut q = state.queue.lock().unwrap();
        if !q.start(id) { return Err(Error::Invalid("That download is already running or done".into())); }
        let premium = q.premium;
        let i = q.get(id).cloned().ok_or_else(|| Error::Invalid("That download is no longer queued".into()))?;
        let click = i.nxm.is_none() && (i.manual_only || !premium);
        if click { q.set(id, ItemState::NeedsClick); }
        click.then(|| runner::file_page(i.mod_id, (!i.manual_only).then_some(i.file_id)))
    };
    if let Some(url) = click { runner::open_click_page(&app, &url); }
    state.pump.notify_one();
    runner::emit(&app);
    Ok(runner::snapshot(&app))
}

#[tauri::command]
pub async fn open_nexus_page(app: AppHandle, mod_id: u32, file_id: Option<u32>) -> Result<()> {
    runner::open_nexus(&app, &runner::file_page(mod_id, file_id))
}

/// Pages on nexusmods.com the app links to (account settings, API keys, collections), opened where pages open.
#[tauri::command]
pub async fn open_nexus_url(app: AppHandle, url: String) -> Result<()> {
    let ok = url.starts_with("https://www.nexusmods.com/") || url.starts_with("https://next.nexusmods.com/") || url.starts_with("https://users.nexusmods.com/");
    if !ok { return Err(Error::Invalid("Only Nexus Mods pages open from here".into())); }
    runner::open_nexus(&app, &url)
}

async fn installed_or_present(state: &State<'_, AppState>) -> Result<std::collections::BTreeSet<u32>> { present_ids(&state.db).await }

/// `present_mod_ids` off the async runtime: it reads the game folder and Vortex's folders.
pub async fn present_ids(db: &std::sync::Arc<std::sync::Mutex<pocket_core::db::Db>>) -> Result<std::collections::BTreeSet<u32>> {
    let db = db.clone();
    tauri::async_runtime::spawn_blocking(move || present_mod_ids(&db)).await.map_err(|e| Error::Invalid(e.to_string()))?
}

/// Nexus mods the user already has in any form: installed or tracked here, a framework whose files are in the game
/// folder (whoever put them there), or a mod Vortex has deployed. Requirements in this set are never downloaded.
pub fn present_mod_ids(db: &std::sync::Mutex<pocket_core::db::Db>) -> Result<std::collections::BTreeSet<u32>> {
    let mut ids = install::installed_mod_ids(&db.lock().unwrap())?;
    let paths = Paths::load(&db.lock().unwrap());
    if let Some(g) = paths.game_dir {
        for f in pocket_core::cyberpunk::frameworks::FRAMEWORKS {
            if f.files.iter().any(|p| g.join(p).exists()) { ids.insert(f.nexus_mod_id); }
        }
        if let Some(setup) = pocket_core::vortex::find(&g) { ids.extend(pocket_core::vortex::likely_mod_ids(&setup, &g)); }
        // RED4ext (1.13 and newer) replaces cybercmd: a mod that still lists cybercmd does not need it downloaded
        if g.join("red4ext/RED4ext.dll").exists() { ids.insert(5176); }
    }
    Ok(ids)
}

// ── nxm:// handler ──

#[tauri::command]
pub fn nxm_handler_status() -> Result<HandlerStatus> { platform::nxm_handler_status(&std::env::current_exe()?) }

#[tauri::command]
pub fn nxm_handler_register() -> Result<HandlerStatus> { platform::register_nxm_handler(&std::env::current_exe()?) }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsUsage { pub files: usize, pub bytes: u64, pub installed_files: usize, pub installed_bytes: u64 }

/// How much space downloaded archives take, and how much of it is archives already installed.
#[tauri::command]
pub async fn downloads_usage(state: State<'_, AppState>) -> Result<DownloadsUsage> {
    let rows = library_downloads(state).await?;
    let on_disk: Vec<&DownloadRow> = rows.iter().filter(|r| std::path::Path::new(&r.path).exists()).collect();
    let inst: Vec<&&DownloadRow> = on_disk.iter().filter(|r| r.installed).collect();
    Ok(DownloadsUsage { files: on_disk.len(), bytes: on_disk.iter().map(|r| r.size as u64).sum(), installed_files: inst.len(), installed_bytes: inst.iter().map(|r| r.size as u64).sum() })
}

/// Delete archives of mods that are installed (the staged copy is what the game uses). Reinstalling downloads again.
#[tauri::command]
pub async fn downloads_cleanup(state: State<'_, AppState>) -> Result<u64> {
    let rows = library_downloads(state).await?;
    let mut freed = 0;
    for r in rows.iter().filter(|r| r.installed) {
        if std::fs::remove_file(&r.path).is_ok() { freed += r.size as u64; }
    }
    Ok(freed)
}

/// Send a test nxm:// link through Windows, to show which app gets it.
#[tauri::command]
pub fn nxm_handler_test() -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").args(["/C", "start", "", "nxm://cyberpunk2077/mods/0/files/0"]).creation_flags(0x0800_0000).spawn()
            .map_err(|e| Error::Invalid(format!("Could not send the test link: {e}")))?;
        Ok(())
    }
    #[cfg(not(windows))]
    { Err(Error::Invalid("The link test only runs on Windows".into())) }
}

// ── archives from outside the queue (Install from file, dropped files, manual downloads) ──

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Identified { pub mod_id: Option<u32>, pub file_id: Option<u32>, pub name: String, pub version: String, pub file_name: Option<String> }

/// Which Nexus mod and file an archive is, from its Nexus file name (`Name-<mod>-<version>-<time>.zip`) and the
/// mod's file list, so a mod installed from a file still gets update checks.
#[tauri::command]
pub async fn library_identify(state: State<'_, AppState>, path: String) -> Result<Identified> {
    let file = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = file.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(file.clone());
    let plain = || stem.replace(['_', '-'], " ").trim().to_string();
    for c in pocket_core::vortex::parse_archive_name(&stem) {
        let Ok((d, files)) = pocket_core::browse::mod_files(&state.nexus, &state.db, c.mod_id, None).await else { continue };
        let f = files.iter().find(|f| f.uri.rsplit_once('.').map(|(a, _)| a) == Some(stem.as_str()));
        let name_fits = d.name.to_lowercase().replace(' ', "").starts_with(&c.name.to_lowercase().replace(['_', '-', ' '], "").chars().take(5).collect::<String>());
        if f.is_some() || name_fits {
            return Ok(Identified { mod_id: Some(c.mod_id), file_id: f.map(|f| f.file_id), name: d.name.clone(),
                version: f.map(|f| f.version.clone()).unwrap_or(c.version.clone()), file_name: f.map(|f| f.name.clone()) });
        }
    }
    Ok(Identified { name: plain(), ..Default::default() })
}

/// Stop a running download (its part is kept for a retry), or take a waiting item out.
#[tauri::command]
pub fn queue_cancel(app: AppHandle, state: State<'_, AppState>, id: u64) -> QueueSnapshot {
    let busy = state.queue.lock().unwrap().get(id).is_some_and(|i| i.is_busy());
    if busy { state.cancel.lock().unwrap().insert(id); } else { state.queue.lock().unwrap().remove(id); }
    state.pump.notify_one();
    runner::emit(&app);
    runner::snapshot(&app)
}
