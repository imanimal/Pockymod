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
pub fn redmod_order(state: State<'_, AppState>) -> Result<Vec<RedmodEntry>> { profiles::redmod_order(&state.db.lock().unwrap()) }

#[tauri::command]
pub fn profiles_list(state: State<'_, AppState>) -> Result<Vec<Profile>> { profiles::list(&state.db.lock().unwrap()) }

#[tauri::command]
pub fn profile_create(state: State<'_, AppState>, name: String, copy_from: Option<i64>) -> Result<i64> { profiles::create(&state.db.lock().unwrap(), &name, copy_from) }

#[tauri::command]
pub fn profile_rename(state: State<'_, AppState>, id: i64, name: String) -> Result<()> { profiles::rename(&state.db.lock().unwrap(), id, &name) }

#[tauri::command]
pub fn profile_delete(state: State<'_, AppState>, id: i64) -> Result<()> { profiles::delete(&state.db.lock().unwrap(), id) }

#[tauri::command]
pub async fn profile_activate(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<SwitchReport> {
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
    let p = Paths::load(&state.db.lock().unwrap());
    let (game, library) = p.require()?;
    let opts = p.deploy_options();
    let db = state.db.clone();
    let r = blocking(move || profiles::set_enabled(&db.lock().unwrap(), &game, &library, install_id, on, &opts)).await?;
    let _ = app.emit("library-changed", ());
    Ok(r)
}

#[tauri::command]
pub fn load_order_set(app: AppHandle, state: State<'_, AppState>, install_ids: Vec<i64>) -> Result<()> {
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
pub fn updates_last(state: State<'_, AppState>) -> Vec<Update> { updates::last(&state.db.lock().unwrap()) }
