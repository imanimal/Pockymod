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
    if key.len() < 8 { return Err(Error::Invalid("That does not look like a mod.io API key.".into())); }
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
