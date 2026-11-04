//! Account and key commands.
use crate::state::AppState;
use pocket_core::credentials::Secret;
use pocket_core::nexus::ratelimit::RateDisplay;
use pocket_core::nexus::rest::NexusUser;
use pocket_core::{Error, Result};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusStatus {
    pub has_key: bool,
    pub user: Option<NexusUser>,
    pub rate_limit: RateDisplay,
}

#[tauri::command]
pub async fn nexus_status(state: State<'_, AppState>, refresh: Option<bool>) -> Result<NexusStatus> {
    let has_key = state.nexus.has_key();
    if has_key && (refresh.unwrap_or(false) || state.user.read().await.is_none()) {
        // validate once per session; a bad key shows as no user, the error goes to the caller
        match state.nexus.validate().await {
            Ok(u) => *state.user.write().await = Some(u),
            Err(e @ (Error::BadKey | Error::Throttled { .. })) => return Err(e),
            Err(_) => {}
        }
    }
    Ok(NexusStatus { has_key, user: state.user.read().await.clone(), rate_limit: state.nexus.rate_limit() })
}

/// Check the key with Nexus, then store it in Windows Credential Manager. A key Nexus rejects is not stored.
#[tauri::command]
pub async fn nexus_set_key(state: State<'_, AppState>, key: String) -> Result<NexusStatus> {
    let key = key.trim().to_owned();
    if key.len() < 16 || key.chars().any(char::is_whitespace) { return Err(Error::Invalid("That does not look like a Nexus API key.".into())); }
    let previous = state.keys.get().ok().flatten();
    state.nexus.set_key(Some(Secret::new(key.clone())));
    match state.nexus.validate().await {
        Ok(user) => {
            state.keys.set(&Secret::new(key))?;
            *state.user.write().await = Some(user);
            nexus_status(state, None).await
        }
        Err(e) => {
            state.nexus.set_key(previous);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn nexus_clear_key(state: State<'_, AppState>) -> Result<NexusStatus> {
    state.keys.clear()?;
    state.nexus.set_key(None);
    *state.user.write().await = None;
    nexus_status(state, None).await
}

#[tauri::command]
pub fn rate_limit(state: State<'_, AppState>) -> RateDisplay { state.nexus.rate_limit() }
