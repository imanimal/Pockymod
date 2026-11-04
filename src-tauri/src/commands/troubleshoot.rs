//! Finding what broke and fixing the order: the tools' logs, overlapping archives, the user's standing conflict
//! choices, changing a FOMOD mod's options, and moving the library or the game.
use crate::state::{AppState, Paths};
use pocket_core::cyberpunk::logs::{self, LogReport};
use pocket_core::cyberpunk::rdar::{self, OverlapReport, RenameTo};
use pocket_core::deploy::DeployOutcome;
use pocket_core::install::{self, FomodOptions};
use pocket_core::relocate::{self, MoveCheck, MoveReport, RelinkReport};
use pocket_core::rules::{self, Rules};
use pocket_core::{Error, Result};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> impl std::future::Future<Output = Result<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Invalid(e.to_string()))? }
}

fn game_dir(state: &State<'_, AppState>) -> Result<PathBuf> {
    Paths::load(&state.db.lock().unwrap()).game_dir.ok_or_else(|| Error::Invalid("Set the Cyberpunk 2077 folder in Settings first.".into()))
}

// ── logs ──

/// The tools' newest logs, with the last start's errors by mod.
#[tauri::command]
pub async fn logs_scan(state: State<'_, AppState>) -> Result<LogReport> {
    let game = game_dir(&state)?;
    let db = state.db.clone();
    blocking(move || {
        let d = db.lock().unwrap();
        let launched = d.setting("game.lastLaunch")?.and_then(|v| v.parse().ok());
        logs::scan(&d, &game, launched)
    }).await
}

/// A plain-text summary for a forum post or a bug report.
#[tauri::command]
pub async fn logs_report(state: State<'_, AppState>) -> Result<String> {
    let game = game_dir(&state)?;
    let db = state.db.clone();
    blocking(move || {
        let info = pocket_core::cyberpunk::game::inspect(&game);
        let d = db.lock().unwrap();
        let patch = info.detected_patch.clone().or(d.setting("game.patch")?);
        let launched = d.setting("game.lastLaunch")?.and_then(|v| v.parse().ok());
        let r = logs::scan(&d, &game, launched)?;
        logs::report_text(&d, &info, patch.as_deref(), &r)
    }).await
}

/// Open a log (game-relative path) in the program Windows uses for .log files.
#[tauri::command]
pub async fn log_open(state: State<'_, AppState>, path: String) -> Result<()> {
    let game = game_dir(&state)?;
    let ok = path.to_lowercase().ends_with(".log") && !path.contains("..") && !path.contains(':');
    let file = game.join(&path);
    if !ok || !file.is_file() { return Err(Error::Invalid("That log is not there any more.".into())); }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&file).creation_flags(0x0800_0000).spawn()
            .map_err(|e| Error::Invalid(format!("Could not open the log: {e}")))?;
    }
    #[cfg(not(windows))]
    { let _ = file; }
    Ok(())
}

// ── archives ──

/// Mods whose archives change the same game files, and who wins.
#[tauri::command]
pub async fn archive_overlaps(state: State<'_, AppState>) -> Result<OverlapReport> {
    let game = game_dir(&state)?;
    let db = state.db.clone();
    blocking(move || rdar::overlaps(&db.lock().unwrap(), &game)).await
}

/// Rename a mod's archives in the game so they load first (undoable from the rules list).
#[tauri::command]
pub async fn archive_load_first(app: AppHandle, state: State<'_, AppState>, install_id: i64, renames: Vec<RenameTo>) -> Result<DeployOutcome> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let r = blocking(move || rdar::load_first(&db.lock().unwrap(), &game, &library, install_id, &renames, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[tauri::command]
pub async fn archive_rename_undo(app: AppHandle, state: State<'_, AppState>, mod_id: u32, target: String) -> Result<()> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    blocking(move || rdar::undo_rename(&db.lock().unwrap(), &game, &library, mod_id, &target, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Move a stray archive/pc/mod/modlist.txt aside.
#[tauri::command]
pub async fn modlist_disable(state: State<'_, AppState>) -> Result<bool> {
    crate::commands::not_while_running()?;
    let game = game_dir(&state)?;
    blocking(move || rdar::disable_modlist(&game)).await
}

// ── standing conflict choices ──

#[tauri::command]
pub async fn rules_list(state: State<'_, AppState>) -> Result<Rules> { rules::list(&state.db.lock().unwrap()) }

#[tauri::command]
pub async fn rule_forget(state: State<'_, AppState>, id: i64) -> Result<()> { rules::forget_win(&state.db.lock().unwrap(), id) }

// ── FOMOD options after install ──

#[tauri::command]
pub async fn fomod_options(state: State<'_, AppState>, install_id: i64) -> Result<FomodOptions> {
    let db = state.db.clone();
    blocking(move || install::fomod_options(&db.lock().unwrap(), install_id)).await
}

#[tauri::command]
pub async fn fomod_change(app: AppHandle, state: State<'_, AppState>, install_id: i64, choices: Vec<[usize; 3]>) -> Result<Option<DeployOutcome>> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let r = blocking(move || install::change_fomod(&db.lock().unwrap(), &game, &library, install_id, &choices, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

// ── moving the library, and the game ──

#[tauri::command]
pub async fn library_move_check(state: State<'_, AppState>, to: String) -> Result<MoveCheck> {
    let (game, library) = Paths::load(&state.db.lock().unwrap()).require()?;
    blocking(move || Ok(relocate::check_move(&game, &library, &PathBuf::from(to.trim())))).await
}

/// Move the library. Progress goes out as `library-move` events ([bytes copied, total]) when it has to copy.
#[tauri::command]
pub async fn library_move(app: AppHandle, state: State<'_, AppState>, to: String) -> Result<MoveReport> {
    crate::commands::not_while_running()?;
    if state.queue.lock().unwrap().summary().running { return Err(Error::Invalid("Wait for the downloads and installs in progress to finish first.".into())); }
    let (game, library) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let a = app.clone();
    let r = blocking(move || {
        let mut last = 0u64;
        relocate::move_library(&db.lock().unwrap(), &game, &library, &PathBuf::from(to.trim()), &mut |done, total| {
            if done >= last + (8 << 20) || done == total { last = done; let _ = a.emit("library-move", (done, total)); }
        })
    }).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFolderSet { pub paths: crate::commands::library::PathsView, pub relink: Option<RelinkReport> }

/// Set the game folder. When the new one is a moved or copied game with the mods' files in it, they are linked to
/// the library again.
#[tauri::command]
pub async fn game_folder_set(state: State<'_, AppState>, dir: String) -> Result<GameFolderSet> {
    let new = PathBuf::from(dir.trim());
    let old = Paths::load(&state.db.lock().unwrap()).game_dir;
    state.db.lock().unwrap().set_setting("game.dir", dir.trim())?;
    let mut relink = None;
    let changed = old.as_ref().is_some_and(|o| o != &new);
    if changed && new.join("bin/x64").is_dir() {
        let allow_copy = Paths::load(&state.db.lock().unwrap()).allow_copy;
        let db = state.db.clone();
        relink = blocking(move || {
            let d = db.lock().unwrap();
            if relocate::looks_like_moved_copy(&d, &new)? { Ok(Some(relocate::relink_game(&d, &new, allow_copy)?)) } else { Ok(None) }
        }).await?;
    }
    Ok(GameFolderSet { paths: crate::commands::library::paths_get(state).await?, relink })
}

/// Link the game's mod files to the library again (the Settings key).
#[tauri::command]
pub async fn game_relink(app: AppHandle, state: State<'_, AppState>) -> Result<RelinkReport> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, _) = p.require()?;
    let db = state.db.clone();
    let r = blocking(move || relocate::relink_game(&db.lock().unwrap(), &game, p.allow_copy)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}
