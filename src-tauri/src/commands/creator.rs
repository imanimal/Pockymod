//! For mod authors: their own mods, with files, changelogs and requirements, and the ids the Upload API needs.
//! Checking, packing and publishing stay in `extras.rs`.
use crate::state::AppState;
use pocket_core::creator::mods::{self, ModManage, MyMods, UploadTarget};
use pocket_core::nexus::rest::NexusUser;
use pocket_core::{Error, Result};
use tauri::State;

/// The signed-in member, checking the key once per session. None without a key.
async fn signed_in(state: &AppState) -> Result<Option<NexusUser>> {
    if let Some(u) = state.user.read().await.clone() { return Ok(Some(u)); }
    if !state.nexus.has_key() { return Ok(None); }
    let u = state.nexus.validate().await?;
    *state.user.write().await = Some(u.clone());
    Ok(Some(u))
}

fn pinned_key(member_id: u64) -> String { format!("creator.pinned.{member_id}") }

fn pinned(state: &AppState, member_id: u64) -> Result<Vec<u32>> {
    let raw = state.db.lock().unwrap().setting(&pinned_key(member_id))?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

/// A member's Cyberpunk mods. `user` is a name, member id or profile link; empty means the signed-in member.
/// Without a key this is read-only and shows published mods only.
#[tauri::command]
pub async fn creator_my_mods(state: State<'_, AppState>, user: Option<String>, refresh: Option<bool>) -> Result<MyMods> {
    let typed = user.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    // a typed lookup works even when the key cannot be checked right now
    let me = if typed.is_some() { signed_in(&state).await.ok().flatten() } else { signed_in(&state).await? };
    let member = match typed {
        Some(t) => mods::resolve_member(&state.nexus, &t).await?,
        None => me.as_ref().map(|u| u.user_id)
            .ok_or_else(|| Error::Invalid("Sign in to Nexus in Settings, or type your Nexus user name".into()))?,
    };
    let own = me.is_some_and(|u| u.user_id == member);
    let pins = pinned(&state, member)?;
    mods::my_mods(&state.nexus, &state.db, member, &pins, own, refresh.unwrap_or(false)).await
}

/// One mod's files (every version), changelog, requirements and the mods that require it.
#[tauri::command]
pub async fn creator_mod_files(state: State<'_, AppState>, mod_id: u32) -> Result<ModManage> {
    mods::manage(&state.nexus, &state.db, mod_id).await
}

/// The files the Upload API lists for a mod: the ids a new version goes to. Needs the author's key.
#[tauri::command]
pub async fn creator_upload_targets(state: State<'_, AppState>, mod_id: u32) -> Result<Vec<UploadTarget>> {
    let key = state.keys.get()?.ok_or_else(|| Error::Invalid("Publishing uses your personal API key. Paste it in Settings, under Account".into()))?;
    mods::upload_targets(&state.nexus, key.expose(), mod_id).await
}

/// Add a mod to a member's list by its link or id (a page just made, or one they co-author). Returns the mod id.
#[tauri::command]
pub fn creator_pin(state: State<'_, AppState>, member_id: u64, mod_ref: String) -> Result<u32> {
    let id = mods::mod_ref(&mod_ref).ok_or_else(|| Error::Invalid("Paste the mod's page link (nexusmods.com/cyberpunk2077/mods/…) or its number".into()))?;
    let mut list = pinned(&state, member_id)?;
    if !list.contains(&id) { list.push(id); }
    state.db.lock().unwrap().set_setting(&pinned_key(member_id), &serde_json::to_string(&list).unwrap_or_default())?;
    Ok(id)
}

#[tauri::command]
pub fn creator_unpin(state: State<'_, AppState>, member_id: u64, mod_id: u32) -> Result<()> {
    let list: Vec<u32> = pinned(&state, member_id)?.into_iter().filter(|x| *x != mod_id).collect();
    state.db.lock().unwrap().set_setting(&pinned_key(member_id), &serde_json::to_string(&list).unwrap_or_default())
}
