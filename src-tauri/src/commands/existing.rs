//! Existing mods (Vortex's and hand-installed ones), groups, and bulk library actions.
use crate::state::{AppState, Paths};
use pocket_core::found::{self, FoundMod};
use pocket_core::install::{self, PRESET_GROUPS};
use pocket_core::vortex::{self, TrackReport};
use pocket_core::vortex_profiles::{self, ImportReport, VortexProfile};
use pocket_core::{profiles, Error, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use tauri::{AppHandle, Emitter, State};

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> impl std::future::Future<Output = Result<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Invalid(e.to_string()))? }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VortexSummary { pub staging: Option<String>, pub mods: usize, pub deployed: usize, pub already_tracked: usize, pub sample: Vec<String>, pub profiles: Vec<ProfileBrief> }

/// A Vortex profile as the scan shows it: its name, how many mods are on in it, and whether Vortex has it deployed.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBrief { pub name: String, pub on: usize, pub active: bool }

fn briefs(list: &[VortexProfile]) -> Vec<ProfileBrief> { list.iter().map(|p| ProfileBrief { name: p.name.clone(), on: p.enabled.len(), active: p.active }).collect() }

/// Vortex's profiles for the game, or none when Vortex's settings are not there or cannot be read.
fn vortex_profiles_now() -> Vec<VortexProfile> { vortex_profiles::state_dir().and_then(|d| vortex_profiles::read(&d).ok()).unwrap_or_default() }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingScan { pub vortex: Option<VortexSummary>, pub found: Vec<FoundMod> }

/// What is already in the game: Vortex's mods and mods no manager owns. Reads only; nothing changes.
#[tauri::command]
pub async fn existing_scan(app: AppHandle, state: State<'_, AppState>) -> Result<ExistingScan> {
    let Some(game) = Paths::load(&state.db.lock().unwrap()).game_dir else { return Ok(ExistingScan { vortex: None, found: Vec::new() }) };
    let db = state.db.clone();
    blocking(move || {
        let say = |stage: &str, name: String| { let _ = app.emit("existing-progress", serde_json::json!({ "stage": stage, "done": 0, "total": 0, "name": name })); };
        say("vortex", std::env::var("APPDATA").map(|a| format!("{a}\\Vortex")).unwrap_or_else(|_| "Vortex".into()));
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
                    profiles: briefs(&vortex_profiles_now()),
                }), files)
            }
            None => (None, BTreeSet::new()),
        };
        for d in ["archive\\pc\\mod", "mods", "r6\\scripts", "r6\\tweaks", "bin\\x64\\plugins\\cyber_engine_tweaks\\mods", "red4ext\\plugins"] {
            say("folder", format!("{}\\{d}", game.display()));
        }
        let found = found::scan(&db.lock().unwrap(), &game, &vortex_files)?;
        Ok(ExistingScan { vortex: summary.filter(|s| s.mods > 0), found })
    }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackResult { pub vortex: TrackReport, pub found: usize, pub profiles: Option<ImportReport> }

/// Track what the scan found. Vortex's mods are matched to Nexus (so they get update checks) and stay Vortex's.
#[tauri::command]
pub async fn existing_track(app: AppHandle, state: State<'_, AppState>, vortex_mods: bool, found_mods: bool, vortex_profiles: Option<bool>) -> Result<TrackResult> {
    let paths = Paths::load(&state.db.lock().unwrap());
    let (game, library) = paths.require()?;
    let mut res = TrackResult { vortex: TrackReport::default(), found: 0, profiles: None };
    if vortex_mods {
        if let Some(setup) = vortex::find(&game) {
            let list = vortex::list_mods(&setup, &game);
            let total = list.len();
            let _ = app.emit("existing-progress", serde_json::json!({ "stage": "matching", "done": 0, "total": total, "name": "" }));
            let app2 = app.clone();
            let resolved = vortex::resolve(&state.nexus, &state.db, list, move |done, total, name| {
                let _ = app2.emit("existing-progress", serde_json::json!({ "stage": "matching", "done": done, "total": total, "name": name }));
            }).await;
            let _ = app.emit("existing-progress", serde_json::json!({ "stage": "tracking", "done": total, "total": total, "name": "" }));
            let db = state.db.clone();
            let g = game.clone();
            res.vortex = blocking(move || vortex::track(&db.lock().unwrap(), &g, &setup, &resolved)).await?;
        }
    }
    if found_mods {
        let _ = app.emit("existing-progress", serde_json::json!({ "stage": "linking", "done": 0, "total": 0, "name": "" }));
        let db = state.db.clone();
        let (g, l) = (game.clone(), library.clone());
        res.found = blocking(move || {
            let d = db.lock().unwrap();
            let skip: BTreeSet<String> = vortex::find(&g).map(|s| vortex::list_mods(&s, &g).into_iter().flat_map(|m| m.files).map(|f| f.to_lowercase()).collect()).unwrap_or_default();
            let mods = found::scan(&d, &g, &skip)?;
            Ok(found::track(&d, &g, &l, &mods)?.len())
        }).await?;
    }
    // Vortex's profiles come along, so its mods stay in the sets they were in
    if vortex_mods && vortex_profiles.unwrap_or(true) {
        let _ = app.emit("existing-progress", serde_json::json!({ "stage": "profiles", "done": 0, "total": 0, "name": "" }));
        let list = blocking(|| Ok(vortex_profiles_now())).await?;
        if list.len() > 1 || list.first().is_some_and(|p| p.name != "Default") {
            let db = state.db.clone();
            let (g, l, o) = (game.clone(), library.clone(), paths.deploy_options());
            res.profiles = Some(blocking(move || vortex_profiles::import(&db.lock().unwrap(), &g, &l, &o, &list)).await?);
        }
    }
    state.db.lock().unwrap().set_setting("existing.asked", "1")?;
    let _ = app.emit("library-changed", ());
    Ok(res)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VortexProfilesView { pub profiles: Vec<ProfileBrief>, pub imported: bool, pub tracked: usize }

/// Unmanaged mods sorted for cleaning up: copies of Vortex mods, frameworks put in by hand, and the rest.
#[tauri::command]
pub async fn unmanaged_list(state: State<'_, AppState>) -> Result<pocket_core::found::Unmanaged> { pocket_core::found::unmanaged(&state.db.lock().unwrap()) }

/// Vortex's profiles, whether they were brought in already, and how many Vortex mods are tracked here.
#[tauri::command]
pub async fn vortex_profiles_scan(state: State<'_, AppState>) -> Result<VortexProfilesView> {
    let list = blocking(|| Ok(vortex_profiles_now())).await?;
    let d = state.db.lock().unwrap();
    let tracked: i64 = d.conn.query_row("SELECT COUNT(*) FROM installs WHERE source = 'vortex'", [], |r| r.get(0))?;
    Ok(VortexProfilesView { profiles: briefs(&list), imported: d.setting("vortex.profilesImported")?.is_some(), tracked: tracked as usize })
}

/// Bring Vortex's profiles in for the Vortex mods already tracked (also after an import made before this existed).
#[tauri::command]
pub async fn vortex_profiles_import(app: AppHandle, state: State<'_, AppState>) -> Result<ImportReport> {
    crate::commands::not_while_running()?;
    let paths = Paths::load(&state.db.lock().unwrap());
    let (game, library) = paths.require()?;
    let dir = vortex_profiles::state_dir().ok_or_else(|| Error::Invalid("Vortex's settings folder (%APPDATA%\\Vortex\\state.v2) is missing".into()))?;
    let list = blocking(move || vortex_profiles::read(&dir)).await?;
    if list.is_empty() { return Err(Error::Invalid("Vortex has no Cyberpunk 2077 profiles".into())); }
    let db = state.db.clone();
    let o = paths.deploy_options();
    let r = blocking(move || vortex_profiles::import(&db.lock().unwrap(), &game, &library, &o, &list)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Groups { pub presets: Vec<&'static str>, pub custom: Vec<String> }

#[tauri::command]
pub async fn groups_list(state: State<'_, AppState>) -> Result<Groups> {
    Ok(Groups { presets: PRESET_GROUPS.to_vec(), custom: install::custom_groups(&state.db.lock().unwrap())? })
}

#[tauri::command]
pub async fn groups_set(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>, group: Option<String>) -> Result<()> {
    install::set_group(&state.db.lock().unwrap(), &install_ids, group.as_deref())?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn group_create(state: State<'_, AppState>, name: String) -> Result<()> { install::create_group(&state.db.lock().unwrap(), &name) }

#[tauri::command]
pub async fn group_rename(app: AppHandle, state: State<'_, AppState>, from: String, to: String) -> Result<()> {
    install::rename_group(&state.db.lock().unwrap(), &from, &to)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn group_delete(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
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
    super::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let rep = blocking(move || {
        let d = db.lock().unwrap();
        let mut rep = BulkReport::default();
        for id in install_ids {
            let r = match action.as_str() {
                // on, but clashing with mods that are on: say so instead of counting it as done
                "enable" => match profiles::set_enabled(&d, &game, &library, id, true, &opts) {
                    Ok(Some(pocket_core::deploy::DeployOutcome::Conflicts { conflicts })) => Err(Error::Invalid(format!("{} of its files clash with other mods. Choose in Library", conflicts.len()))),
                    Ok(Some(pocket_core::deploy::DeployOutcome::NeedsCopy { reason })) => Err(Error::Invalid(reason)),
                    other => other.map(|_| ()),
                },
                // undoing "turned off": back as it was, winning its clashes again
                "enable-win" => profiles::set_enabled(&d, &game, &library, id, true, &pocket_core::deploy::DeployOptions { take_conflicts: true, ..opts.clone() }).map(|_| ()),
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

/// Go back to the user's own files that an install set aside, keeping them tracked as the user's copy.
#[tauri::command]
pub async fn library_restore_replaced(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>) -> Result<usize> {
    super::not_while_running()?;
    let (game, library) = Paths::load(&state.db.lock().unwrap()).require()?;
    let db = state.db.clone();
    let n = blocking(move || {
        let d = db.lock().unwrap();
        let mut n = 0;
        for id in install_ids { if install::restore_replaced(&d, &game, &library, id).is_ok() { n += 1; } }
        Ok(n)
    }).await?;
    let _ = app.emit("library-changed", ());
    Ok(n)
}

/// Sets of installs of the same Nexus mod: the one to keep first, then the extra copies.
#[tauri::command]
pub async fn library_duplicates(state: State<'_, AppState>) -> Result<Vec<Vec<i64>>> { install::duplicates(&state.db.lock().unwrap()) }

/// Keep one install of a mod and remove the other copies. A copy from Vortex stays in Vortex; the kept one is put
/// back in the game if it is on, so files the removed copies held go back to it.
#[tauri::command]
pub async fn library_keep_one(app: AppHandle, state: State<'_, AppState>, keep: i64, remove: Vec<i64>) -> Result<()> {
    super::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    blocking(move || {
        let d = db.lock().unwrap();
        // removing a copy puts back whatever it had set aside, then the kept one takes its files again
        for id in remove.iter().filter(|x| **x != keep) { install::uninstall(&d, &game, *id)?; }
        let on: bool = d.conn.query_row("SELECT COALESCE((SELECT enabled FROM profile_mods WHERE install_id = ?1 AND profile_id = (SELECT id FROM profiles WHERE active = 1 LIMIT 1)), 0)", [keep], |r| r.get(0))?;
        if on { let _ = install::redeploy_any(&d, &game, &library, keep, &opts); }
        Ok(())
    }).await?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

// ── recently removed ──

#[tauri::command]
pub async fn library_trash_list(state: State<'_, AppState>) -> Result<Vec<install::TrashRow>> { install::list_trash(&state.db.lock().unwrap()) }

/// Put removed mods back. Returns the install ids, in the same order (0 for one that could not be).
#[tauri::command]
pub async fn library_trash_restore(app: AppHandle, state: State<'_, AppState>, trash_ids: Vec<i64>) -> Result<Vec<i64>> {
    super::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let out = blocking(move || {
        let d = db.lock().unwrap();
        let mut out = Vec::new();
        let mut first_err = None;
        for id in trash_ids {
            match install::restore_trash(&d, &game, &library, id, &opts) { Ok(i) => out.push(i), Err(e) => { out.push(0); first_err.get_or_insert(e); } }
        }
        match first_err { Some(e) if out.iter().all(|x| *x == 0) => Err(e), _ => Ok(out) }
    }).await?;
    let _ = app.emit("library-changed", ());
    Ok(out)
}

/// Delete removed mods for good: one, or (without an id) everything past the time and size limits.
#[tauri::command]
pub async fn library_trash_purge(state: State<'_, AppState>, trash_id: Option<i64>, all: Option<bool>) -> Result<usize> {
    let d = state.db.lock().unwrap();
    if all.unwrap_or(false) {
        let ids: Vec<i64> = install::list_trash(&d)?.into_iter().map(|t| t.id).collect();
        let mut n = 0;
        for id in ids { n += install::purge_trash(&d, Some(id))?; }
        return Ok(n);
    }
    install::purge_trash(&d, trash_id)
}
