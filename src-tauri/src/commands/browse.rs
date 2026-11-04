//! Browser commands.
use crate::state::AppState;
use pocket_core::browse::{self, BrowseResult, DetailView, LocalFilter};
use pocket_core::cyberpunk::patches::{Patch, PATCHES};
use pocket_core::db::Flag;
use pocket_core::nexus::graphql::BrowseQuery;
use pocket_core::Result;
use tauri::State;

#[tauri::command]
pub async fn browse_search(state: State<'_, AppState>, query: BrowseQuery, local: LocalFilter) -> Result<BrowseResult> {
    browse::search(&state.nexus, &state.db, &query, &local).await
}

#[tauri::command]
pub async fn browse_detail(state: State<'_, AppState>, mod_id: u32, updated_at: Option<String>) -> Result<DetailView> {
    browse::detail(&state.nexus, &state.db, mod_id, updated_at.as_deref()).await
}

#[tauri::command]
pub async fn set_mod_flag(state: State<'_, AppState>, mod_id: u32, flag: Flag, on: bool) -> Result<()> {
    state.db.lock().unwrap().set_flag(mod_id, flag, on)
}

/// Settings the UI keeps in the local database (game patch, deploy method and so on). Not for secrets.
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>, key: String) -> Result<Option<String>> {
    state.db.lock().unwrap().setting(&key)
}

#[tauri::command]
pub async fn settings_set(state: State<'_, AppState>, key: String, value: String) -> Result<()> {
    state.db.lock().unwrap().set_setting(&key, &value)
}

#[tauri::command]
pub fn patches() -> Vec<(Patch, Option<i64>)> {
    PATCHES.iter().map(|p| (*p, pocket_core::cyberpunk::patches::released_at(p.version))).collect()
}

#[tauri::command]
pub async fn media_browse(state: State<'_, AppState>, kind: String, sort: String, offset: u32) -> Result<serde_json::Value> {
    browse::media(&state.nexus, &state.db, &kind, &sort, offset).await
}
