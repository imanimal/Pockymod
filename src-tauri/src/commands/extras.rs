//! App update check and the read-only mod.io experiment.
use crate::state::AppState;
use pocket_core::appupdate::{self, AppUpdate};
use pocket_core::credentials::{Secret, SecretStore};
use pocket_core::modio::{self, Game, Mod, Page};
use pocket_core::{Error, Result};
use serde::Serialize;
use tauri::State;

#[tauri::command]
pub async fn app_update_check(state: State<'_, AppState>) -> Result<AppUpdate> { appupdate::check(state.nexus.http()).await }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModioBrowse { pub game: Option<Game>, pub page: Option<Page<Mod>> }

#[tauri::command]
pub fn modio_has_key() -> Result<bool> { Ok(modio::key_store().get()?.is_some()) }

#[tauri::command]
pub async fn modio_set_key(key: String) -> Result<Option<Game>> {
    let key = key.trim().to_owned();
    if key.len() < 8 { return Err(Error::Invalid("That isn't a mod.io API key".into())); }
    // keep the key only if mod.io accepts it
    let c = modio::Client::new(Secret::new(key.clone()))?;
    let g = c.find_game().await?;
    modio::key_store().set(&Secret::new(key))?;
    Ok(g)
}

#[tauri::command]
pub fn modio_clear_key() -> Result<()> { modio::key_store().clear() }

#[tauri::command]
pub async fn modio_browse(state: State<'_, AppState>, text: Option<String>, offset: u32) -> Result<ModioBrowse> {
    let c = modio::Client::from_store()?;
    let cached = state.db.lock().unwrap().setting("modio.game")?.and_then(|s| serde_json::from_str::<Game>(&s).ok());
    let game = match cached { Some(g) => Some(g), None => {
        let g = c.find_game().await?;
        if let Some(g) = &g { state.db.lock().unwrap().set_setting("modio.game", &serde_json::to_string(g).unwrap_or_default())?; }
        g
    } };
    let page = match &game { Some(g) => Some(c.mods(g.id, text.as_deref(), offset).await?), None => None };
    Ok(ModioBrowse { game, page })
}

// ── for mod authors ──

#[tauri::command]
pub async fn creator_check(path: String, name: String) -> Result<pocket_core::creator::Check> {
    tauri::async_runtime::spawn_blocking(move || pocket_core::creator::check(std::path::Path::new(&path), &name)).await.map_err(|e| Error::Invalid(e.to_string()))?
}

/// Pack a mod folder into a clean zip next to it. Progress goes out as `package-progress` events ([bytes, total]).
#[tauri::command]
pub async fn creator_package(app: tauri::AppHandle, path: String, name: String, version: String) -> Result<String> {
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::PathBuf::from(&path);
        let out = p.parent().map(|d| d.to_path_buf()).unwrap_or_else(|| p.clone());
        let mut last = 0u64;
        pocket_core::creator::package_with(&p, &out, &name, &version, |done, total| {
            if done >= last + (4 << 20) || done == total { last = done; let _ = app.emit("package-progress", (done, total)); }
        }).map(|f| f.to_string_lossy().into_owned())
    }).await.map_err(|e| Error::Invalid(e.to_string()))?
}

/// Publish a new version of one of your files through the Upload API, with the key in Credential Manager.
#[tauri::command]
pub async fn creator_publish(app: tauri::AppHandle, state: State<'_, AppState>, request: pocket_core::creator::Publish) -> Result<u64> {
    use tauri::Emitter;
    let key = state.keys.get()?.ok_or_else(|| Error::Invalid("Publishing uses your personal API key. Paste it in Settings, under Account".into()))?;
    let app2 = app.clone();
    pocket_core::creator::publish(state.nexus.http(), key.expose(), &request, move |sent, total| {
        let _ = app2.emit("publish-progress", (sent, total));
    }).await
}
