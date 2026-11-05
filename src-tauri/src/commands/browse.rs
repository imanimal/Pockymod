//! Browser commands.
use crate::state::AppState;
use pocket_core::browse::{self, BrowseResult, DetailView, LocalFilter};
use pocket_core::cyberpunk::patches::{Patch, PATCHES};
use pocket_core::db::Flag;
use pocket_core::nexus::graphql::BrowseQuery;
use pocket_core::links::{self, LinkPreview};
use pocket_core::{Error, Result};
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

/// A link from a mod's or collection's description, previewed: title, summary, picture, site and icon. A Nexus mod
/// link is read from the Nexus API (its pages sit behind a bot check); anything else from the page itself. Kept a
/// week (half an hour when the page could not be read).
#[tauri::command]
pub async fn link_preview(state: State<'_, AppState>, url: String) -> Result<LinkPreview> {
    let u = links::safe_url(&url).ok_or_else(|| Error::Invalid("That link is not a public web address.".into()))?;
    let key = format!("link:{u}");
    let now = pocket_core::db::now();
    let hit = state.db.lock().unwrap().cache_get(&key, 7 * 86400, now)?.and_then(|b| serde_json::from_str::<LinkPreview>(&b).ok());
    if let Some(p) = hit {
        let fresh_enough = p.error.is_none() || state.db.lock().unwrap().cache_get(&key, 1800, now)?.is_some();
        if fresh_enough { return Ok(p); }
    }
    let p = match links::nexus_mod_id(&u) {
        Some(id) => match nexus_mod(&state, id).await {
            Some(d) => LinkPreview {
                url: url.clone(), final_url: u.to_string(), kind: "mod".into(), title: Some(d.name), description: Some(d.summary).filter(|s| !s.is_empty()),
                image: d.picture_url, site: Some("Nexus Mods".into()), icon: Some("https://www.nexusmods.com/favicon.ico".into()),
                author: d.author.or(d.uploader.map(|x| x.name)), mod_id: Some(id), downloads: Some(d.downloads), endorsements: Some(d.endorsements), error: None,
            },
            None => LinkPreview { url: url.clone(), final_url: u.to_string(), kind: "mod".into(), site: Some("Nexus Mods".into()), mod_id: Some(id),
                error: Some("Nexus did not answer for this mod.".into()), ..Default::default() },
        },
        None => links::fetch(links::client(), &u).await,
    };
    state.db.lock().unwrap().cache_put(&key, &serde_json::to_string(&p).unwrap_or_default(), now)?;
    Ok(p)
}

/// A Nexus mod's detail from the app's cache, else from Nexus (and cached for Browse too).
async fn nexus_mod(state: &State<'_, AppState>, id: u32) -> Option<pocket_core::nexus::types::ModDetail> {
    let cached = state.db.lock().unwrap().mod_get(id, None).ok().flatten();
    if let Some(d) = cached.and_then(|(d, _)| serde_json::from_str(&d).ok()) { return Some(d); }
    let (d, f) = state.nexus.mod_detail(id).await.ok()?;
    let _ = state.db.lock().unwrap().mod_put(id, &d.updated_at, &serde_json::to_string(&d).unwrap_or_default(), &serde_json::to_string(&f).unwrap_or_default(), pocket_core::db::now());
    Some(d)
}

/// Open a link the user clicked in a description, in their own browser. Any public http or https address.
#[tauri::command]
pub async fn open_link(url: String) -> Result<()> {
    let u = links::safe_url(&url).ok_or_else(|| Error::Invalid("That link is not a web address the app opens.".into()))?;
    crate::commands::extensions::open_in_browser(u.as_str())
}
