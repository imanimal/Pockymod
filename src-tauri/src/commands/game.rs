//! Game, health, REDmod, profile, load order and update commands.
use crate::state::{AppState, Paths};
use pocket_core::cyberpunk::game::{self, GameInfo, GameInstall};
use pocket_core::cyberpunk::redmod::{self, RedmodResult};
use pocket_core::db::now;
use pocket_core::deploy::DeployOutcome;
use pocket_core::health::{self, HealthReport, Latest};
use pocket_core::profiles::{self, Profile, RedmodEntry, SwitchReport};
use pocket_core::updates::{self, Update};
use pocket_core::{Error, Result};
use tauri::{AppHandle, Emitter, State};

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> impl std::future::Future<Output = Result<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Invalid(e.to_string()))? }
}

#[tauri::command]
pub async fn game_detect() -> Result<Vec<GameInstall>> { blocking(|| Ok(game::detect())).await }

#[tauri::command]
pub async fn game_inspect(state: State<'_, AppState>) -> Result<Option<GameInfo>> {
    let g = Paths::load(&state.db.lock().unwrap()).game_dir;
    blocking(move || Ok(g.as_deref().map(game::inspect))).await
}

/// Health: the game, frameworks (with Nexus's newest versions, cached for an hour), deploy state, REDmod.
#[tauri::command]
pub async fn health_check(state: State<'_, AppState>, refresh: Option<bool>) -> Result<HealthReport> {
    let latest = latest_frameworks(&state, refresh.unwrap_or(false)).await;
    let (g, patch) = { let d = state.db.lock().unwrap(); (Paths::load(&d).game_dir, d.setting("game.patch").ok().flatten()) };
    let info = blocking(move || Ok(g.as_deref().map(game::inspect))).await?;
    let db = state.db.clone();
    blocking(move || health::check(&db.lock().unwrap(), info.as_ref(), patch, &latest)).await
}

async fn latest_frameworks(state: &State<'_, AppState>, refresh: bool) -> Vec<Latest> {
    const KEY: &str = "latest-frameworks";
    if !refresh {
        if let Some(s) = state.db.lock().unwrap().cache_get(KEY, 3600, now()).ok().flatten() {
            if let Ok(v) = serde_json::from_str::<Vec<(u32, String, String)>>(&s) { return v.into_iter().map(|(mod_id, version, updated_at)| Latest { mod_id, version, updated_at }).collect(); }
        }
    }
    let Ok(nodes) = state.nexus.mods_by_id(&health::framework_ids()).await else { return Vec::new() };
    let rows: Vec<(u32, String, String)> = nodes.iter().filter_map(|n| Some((n["modId"].as_u64()? as u32, n["version"].as_str()?.to_string(), n["updatedAt"].as_str()?.to_string()))).collect();
    let _ = state.db.lock().unwrap().cache_put(KEY, &serde_json::to_string(&rows).unwrap_or_default(), now());
    rows.into_iter().map(|(mod_id, version, updated_at)| Latest { mod_id, version, updated_at }).collect()
}

/// Run the game's REDmod deploy with the active profile's REDmods in load order.
#[tauri::command]
pub async fn redmod_deploy(app: AppHandle, state: State<'_, AppState>) -> Result<RedmodResult> {
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let order = profiles::redmod_order(&state.db.lock().unwrap())?;
    let mods: Vec<String> = order.into_iter().filter(|e| e.enabled).flat_map(|e| e.folders).collect();
    let r = blocking(move || redmod::run_deploy(&game, &mods)).await?;
    if r.ok { state.db.lock().unwrap().set_setting("redmod.lastDeploy", &now().to_string())?; }
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[tauri::command]
pub async fn redmod_order(state: State<'_, AppState>) -> Result<Vec<RedmodEntry>> { profiles::redmod_order(&state.db.lock().unwrap()) }

#[tauri::command]
pub async fn profiles_list(state: State<'_, AppState>) -> Result<Vec<Profile>> { profiles::list(&state.db.lock().unwrap()) }

#[tauri::command]
pub async fn profile_create(state: State<'_, AppState>, name: String, copy_from: Option<i64>) -> Result<i64> { profiles::create(&state.db.lock().unwrap(), &name, copy_from) }

#[tauri::command]
pub async fn profile_rename(state: State<'_, AppState>, id: i64, name: String) -> Result<()> { profiles::rename(&state.db.lock().unwrap(), id, &name) }

#[tauri::command]
pub async fn profile_delete(state: State<'_, AppState>, id: i64) -> Result<()> { profiles::delete(&state.db.lock().unwrap(), id) }

#[tauri::command]
pub async fn profile_activate(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<SwitchReport> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let r = blocking(move || profiles::activate(&db.lock().unwrap(), &game, &library, id, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[tauri::command]
pub async fn mod_set_enabled(app: AppHandle, state: State<'_, AppState>, install_id: i64, on: bool) -> Result<Option<DeployOutcome>> {
    crate::commands::not_while_running()?;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let r = blocking(move || profiles::set_enabled(&db.lock().unwrap(), &game, &library, install_id, on, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[tauri::command]
pub async fn load_order_set(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>) -> Result<()> {
    profiles::set_order(&state.db.lock().unwrap(), &install_ids)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn updates_check(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<Update>> {
    let u = updates::check(&state.nexus, &state.db).await?;
    let _ = app.emit("updates", &u);
    Ok(u)
}

#[tauri::command]
pub async fn updates_last(state: State<'_, AppState>) -> Result<Vec<Update>> { Ok(updates::last(&state.db.lock().unwrap())) }

/// Start the game, after running the REDmod deploy when REDmods changed. The UI runs the health check first.
#[tauri::command]
pub async fn game_launch(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let (redmods, needs_deploy) = { let d = state.db.lock().unwrap(); (pocket_core::launch::has_redmods(&d)?, profiles::redmod_needs_deploy(&d)?) };
    let mut deployed = None;
    if needs_deploy {
        let order = profiles::redmod_order(&state.db.lock().unwrap())?;
        let mods: Vec<String> = order.into_iter().filter(|e| e.enabled).flat_map(|e| e.folders).collect();
        let g = game.clone();
        let r = blocking(move || redmod::run_deploy(&g, &mods)).await?;
        if !r.ok { return Ok(serde_json::json!({ "started": false, "redmod": r })); }
        state.db.lock().unwrap().set_setting("redmod.lastDeploy", &now().to_string())?;
        deployed = Some(r);
    }
    pocket_core::launch::launch(&game, redmods)?;
    Ok(serde_json::json!({ "started": true, "modded": redmods, "redmod": deployed }))
}

/// Start the game without mods: they come out, the game starts, and they go back when it closes (watched here).
#[tauri::command]
pub async fn game_launch_vanilla(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value> {
    use tauri::Manager;
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let g = game.clone();
    let (out, left) = blocking(move || pocket_core::launch::vanilla_prepare(&db.lock().unwrap(), &g)).await?;
    let _ = app.emit("library-changed", ());
    if let Err(e) = pocket_core::launch::launch(&game, false) {
        let db = state.db.clone();
        let _ = blocking(move || pocket_core::launch::vanilla_restore(&db.lock().unwrap(), &game, &library, &opts)).await;
        return Err(e);
    }
    let _ = app.emit("vanilla", serde_json::json!({ "state": "running", "out": out }));
    // watch for the game to close: seen running, then gone; or never seen within two minutes (it did not start)
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut seen = false;
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            let running = tauri::async_runtime::spawn_blocking(pocket_core::launch::game_running).await.unwrap_or(false);
            if running { seen = true; continue; }
            if seen || started.elapsed().as_secs() > 120 { break; }
        }
        let st = app2.state::<AppState>();
        let db = st.db.clone();
        let n = tauri::async_runtime::spawn_blocking(move || pocket_core::launch::vanilla_restore(&db.lock().unwrap(), &game, &library, &opts)).await.ok().and_then(|r| r.ok()).unwrap_or(0);
        let _ = app2.emit("library-changed", ());
        let _ = app2.emit("vanilla", serde_json::json!({ "state": "restored", "back": n }));
    });
    Ok(serde_json::json!({ "out": out, "untracked": left }))
}

/// Put mods back now (the game closed while the app was not watching, or the user asks).
#[tauri::command]
pub async fn game_vanilla_restore(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<usize> {
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let n = blocking(move || pocket_core::launch::vanilla_restore(&db.lock().unwrap(), &game, &library, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(n)
}

/// Whether mods are out of the game for a start without mods.
#[tauri::command]
pub async fn game_vanilla_pending(state: State<'_, AppState>) -> Result<bool> { Ok(state.db.lock().unwrap().setting(pocket_core::launch::VANILLA_KEY)?.is_some()) }

/// Open the game folder in Explorer.
#[tauri::command]
pub async fn game_open_folder(state: State<'_, AppState>, sub: Option<String>) -> Result<()> {
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let dir = match sub.as_deref() { Some(s) if !s.contains("..") => game.join(s), _ => game };
    #[cfg(windows)]
    { std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| pocket_core::Error::Invalid(format!("Could not open the folder: {e}")))?; }
    #[cfg(not(windows))]
    { let _ = dir; }
    Ok(())
}

/// Put the game's own compiled scripts back (after redscript is removed, or when Health says they are stale).
#[tauri::command]
pub async fn scripts_reset(state: State<'_, AppState>) -> Result<usize> {
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    crate::commands::not_while_running()?;
    blocking(move || game::reset_scripts(&game).map_err(Error::from)).await
}
