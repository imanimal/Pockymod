//! Existing mods (Vortex's and hand-installed ones), groups, and bulk library actions.
use crate::state::{AppState, Paths};
use pocket_core::found::{self, FoundMod};
use pocket_core::install::{self, PRESET_GROUPS};
use pocket_core::vortex::{self, TrackReport};
use pocket_core::{profiles, Error, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use tauri::{AppHandle, Emitter, State};

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> impl std::future::Future<Output = Result<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Invalid(e.to_string()))? }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VortexSummary { pub staging: Option<String>, pub mods: usize, pub deployed: usize, pub already_tracked: usize, pub sample: Vec<String> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingScan { pub vortex: Option<VortexSummary>, pub found: Vec<FoundMod> }

/// What is already in the game: Vortex's mods and mods no manager owns. Reads only; nothing changes.
#[tauri::command]
pub async fn existing_scan(state: State<'_, AppState>) -> Result<ExistingScan> {
    let Some(game) = Paths::load(&state.db.lock().unwrap()).game_dir else { return Ok(ExistingScan { vortex: None, found: Vec::new() }) };
    let db = state.db.clone();
    blocking(move || {
        let setup = vortex::find(&game);
        let (summary, vortex_files) = match &setup {
            Some(s) => {
                let mods = vortex::list_mods(s, &game);
                let d = db.lock().unwrap();
                let tracked: BTreeSet<String> = {
                    let mut st = d.conn.prepare("SELECT external FROM installs WHERE source = 'vortex'")?;
                    let v = st.query_map([], |r| r.get::<_, String>(0))?.collect::<std::result::Result<_, _>>()?;
                    v
                };
                let files: BTreeSet<String> = mods.iter().flat_map(|m| m.files.iter().map(|f| f.to_lowercase())).collect();
                (Some(VortexSummary {
                    staging: s.staging.as_ref().map(|p| p.to_string_lossy().into_owned()), deployed: mods.iter().filter(|m| m.deployed).count(),
                    already_tracked: mods.iter().filter(|m| tracked.contains(&m.folder)).count(), mods: mods.len(),
                    sample: mods.iter().take(8).map(|m| m.candidates.first().map(|c| c.name.clone()).unwrap_or_else(|| m.folder.clone())).collect(),
                }), files)
            }
            None => (None, BTreeSet::new()),
        };
        let found = found::scan(&db.lock().unwrap(), &game, &vortex_files)?;
        Ok(ExistingScan { vortex: summary.filter(|s| s.mods > 0), found })
    }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackResult { pub vortex: TrackReport, pub found: usize }

/// Track what the scan found. Vortex's mods are matched to Nexus (so they get update checks) and stay Vortex's.
#[tauri::command]
pub async fn existing_track(app: AppHandle, state: State<'_, AppState>, vortex_mods: bool, found_mods: bool) -> Result<TrackResult> {
    let paths = Paths::load(&state.db.lock().unwrap());
    let (game, library) = paths.require()?;
    let mut res = TrackResult { vortex: TrackReport::default(), found: 0 };
    if vortex_mods {
        if let Some(setup) = vortex::find(&game) {
            let list = vortex::list_mods(&setup, &game);
            let resolved = vortex::resolve(&state.nexus, &state.db, list).await;
            let db = state.db.clone();
            let g = game.clone();
            res.vortex = blocking(move || vortex::track(&db.lock().unwrap(), &g, &setup, &resolved)).await?;
        }
    }
    if found_mods {
        let db = state.db.clone();
        let (g, l) = (game.clone(), library.clone());
        res.found = blocking(move || {
            let d = db.lock().unwrap();
            let skip: BTreeSet<String> = vortex::find(&g).map(|s| vortex::list_mods(&s, &g).into_iter().flat_map(|m| m.files).map(|f| f.to_lowercase()).collect()).unwrap_or_default();
            let mods = found::scan(&d, &g, &skip)?;
            Ok(found::track(&d, &g, &l, &mods)?.len())
        }).await?;
    }
    state.db.lock().unwrap().set_setting("existing.asked", "1")?;
    let _ = app.emit("library-changed", ());
    Ok(res)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Groups { pub presets: Vec<&'static str>, pub custom: Vec<String> }

#[tauri::command]
pub fn groups_list(state: State<'_, AppState>) -> Result<Groups> {
    Ok(Groups { presets: PRESET_GROUPS.to_vec(), custom: install::custom_groups(&state.db.lock().unwrap())? })
}

#[tauri::command]
pub fn groups_set(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>, group: Option<String>) -> Result<()> {
    install::set_group(&state.db.lock().unwrap(), &install_ids, group.as_deref())?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub fn group_create(state: State<'_, AppState>, name: String) -> Result<()> { install::create_group(&state.db.lock().unwrap(), &name) }

#[tauri::command]
pub fn group_rename(app: AppHandle, state: State<'_, AppState>, from: String, to: String) -> Result<()> {
    install::rename_group(&state.db.lock().unwrap(), &from, &to)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub fn group_delete(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
    install::delete_group(&state.db.lock().unwrap(), &name)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BulkReport { pub done: usize, pub failed: Vec<(i64, String)> }

/// Turn many mods on or off, or remove them, in one go. Each is done on its own, so one failure stops nothing.
#[tauri::command]
pub async fn library_bulk(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>, action: String) -> Result<BulkReport> {
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let rep = blocking(move || {
        let d = db.lock().unwrap();
        let mut rep = BulkReport::default();
        for id in install_ids {
            let r = match action.as_str() {
                "enable" => profiles::set_enabled(&d, &game, &library, id, true, &opts).map(|_| ()),
                "disable" => profiles::set_enabled(&d, &game, &library, id, false, &opts).map(|_| ()),
                "uninstall" => install::uninstall(&d, &game, id).map(|_| ()),
                other => Err(Error::Invalid(format!("Unknown action {other}"))),
            };
            match r { Ok(()) => rep.done += 1, Err(e) => rep.failed.push((id, e.to_string())) }
        }
        Ok(rep)
    }).await?;
    let _ = app.emit("library-changed", ());
    Ok(rep)
}
