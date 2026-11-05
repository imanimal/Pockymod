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
pub async fn nexus_status(app: tauri::AppHandle, state: State<'_, AppState>, refresh: Option<bool>) -> Result<NexusStatus> {
    let has_key = state.nexus.has_key();
    if has_key && (refresh.unwrap_or(false) || state.user.read().await.is_none()) {
        // validate once per session; a bad key shows as no user, the error goes to the caller
        match state.nexus.validate().await {
            Ok(u) => {
                let upgraded = { let mut q = state.queue.lock().unwrap(); let was = q.premium; q.premium = u.is_premium; u.is_premium && !was };
                *state.user.write().await = Some(u);
                // just went Premium: the files waiting for a click download by themselves now
                if upgraded { crate::runner::emit(&app); state.pump.notify_one(); }
            }
            Err(e @ (Error::BadKey | Error::Throttled { .. })) => return Err(e),
            Err(_) => {}
        }
    }
    Ok(NexusStatus { has_key, user: state.user.read().await.clone(), rate_limit: state.nexus.rate_limit() })
}

/// Check the key with Nexus, then store it in Windows Credential Manager. A key Nexus rejects is not stored.
#[tauri::command]
pub async fn nexus_set_key(app: tauri::AppHandle, state: State<'_, AppState>, key: String) -> Result<NexusStatus> {
    let key = key.trim().to_owned();
    if key.len() < 16 || key.chars().any(char::is_whitespace) { return Err(Error::Invalid("That does not look like a Nexus API key.".into())); }
    let previous = state.keys.get().ok().flatten();
    state.nexus.set_key(Some(Secret::new(key.clone())));
    match state.nexus.validate().await {
        Ok(user) => {
            state.keys.set(&Secret::new(key))?;
            state.queue.lock().unwrap().premium = user.is_premium;
            *state.user.write().await = Some(user);
            nexus_status(app, state, None).await
        }
        Err(e) => {
            state.nexus.set_key(previous);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn nexus_clear_key(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<NexusStatus> {
    state.keys.clear()?;
    state.nexus.set_key(None);
    *state.user.write().await = None;
    nexus_status(app, state, None).await
}

#[tauri::command]
pub fn rate_limit(state: State<'_, AppState>) -> RateDisplay { state.nexus.rate_limit() }

/// Endorse a mod (or take the endorsement back). Nexus only accepts it from accounts that downloaded the mod.
#[tauri::command]
pub async fn mod_endorse(state: State<'_, AppState>, mod_id: u32, version: String, on: bool) -> Result<()> {
    if !state.nexus.has_key() { return Err(Error::NoKey); }
    let r = if on { state.nexus.endorse(mod_id, &version).await } else { state.nexus.abstain(mod_id, &version).await };
    r.map(|_| ()).map_err(|e| match e {
        Error::Http { status: 403, .. } => Error::Invalid("Nexus accepts endorsements only after you have downloaded the mod (and waited a few minutes).".into()),
        other => other,
    })
}

/// Track a mod on Nexus (its updates show in your Nexus feed), or stop tracking it.
#[tauri::command]
pub async fn mod_track(state: State<'_, AppState>, mod_id: u32, on: bool) -> Result<()> {
    if !state.nexus.has_key() { return Err(Error::NoKey); }
    if on { state.nexus.track(mod_id).await.map(|_| ()) } else { state.nexus.untrack(mod_id).await.map(|_| ()) }
}

// ── endorsing, made easy ──
// The mods you have that you have not endorsed (nor said "not for me" to), so Library can offer Endorse all.
// Endorsed and declined mods are remembered locally ("endorse.done", "endorse.no"), and Nexus's own list is read
// when there is a key (cached a day).

fn ids(state: &State<'_, AppState>, key: &str) -> std::collections::BTreeSet<u32> {
    state.db.lock().unwrap().setting(key).ok().flatten().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}
fn put_ids(state: &State<'_, AppState>, key: &str, v: &std::collections::BTreeSet<u32>) -> Result<()> {
    state.db.lock().unwrap().set_setting(key, &serde_json::to_string(v).unwrap_or_default())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndorseCandidate { pub mod_id: u32, pub name: String, pub version: String }
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndorseStatus { pub candidates: Vec<EndorseCandidate>, pub endorsed: Vec<u32>, pub has_key: bool }

#[tauri::command]
pub async fn endorse_status(state: State<'_, AppState>) -> Result<EndorseStatus> {
    let mut done = ids(&state, "endorse.done");
    let no = ids(&state, "endorse.no");
    let has_key = state.nexus.has_key();
    if has_key {
        let now = pocket_core::db::now();
        let fresh = state.db.lock().unwrap().cache_get("endorse.remote", 86400, now).ok().flatten();
        let list: Option<Vec<pocket_core::nexus::rest::Endorsement>> = match fresh {
            Some(t) => serde_json::from_str(&t).ok(),
            None => match state.nexus.endorsements().await {
                Ok(l) => { let _ = state.db.lock().unwrap().cache_put("endorse.remote", &serde_json::to_string(&l).unwrap_or_default(), now); Some(l) }
                Err(_) => None,
            },
        };
        for e in list.into_iter().flatten().filter(|e| e.domain_name == pocket_core::GAME_DOMAIN || e.domain_name.is_empty()) {
            if e.status.eq_ignore_ascii_case("endorsed") || e.status.eq_ignore_ascii_case("abstained") { done.insert(e.mod_id); }
        }
    }
    // installed Nexus mods, once each, that have been here a day (Nexus wants a download and a little time first)
    let rows = pocket_core::install::list_installs(&state.db.lock().unwrap())?;
    let day_ago = pocket_core::db::now() - 86400;
    let mut seen = std::collections::BTreeSet::new();
    let candidates = rows.into_iter()
        .filter(|r| r.mod_id < 900_000_000 && !done.contains(&r.mod_id) && !no.contains(&r.mod_id) && r.installed_at < day_ago && seen.insert(r.mod_id))
        .map(|r| EndorseCandidate { mod_id: r.mod_id, name: r.name, version: r.version }).collect();
    Ok(EndorseStatus { candidates, endorsed: done.into_iter().collect(), has_key })
}

/// Endorse several mods. Returns how many Nexus took and the first refusal, if any.
#[tauri::command]
pub async fn endorse_many(state: State<'_, AppState>, mods: Vec<(u32, String)>) -> Result<(usize, Option<String>)> {
    if !state.nexus.has_key() { return Err(Error::NoKey); }
    let mut done = ids(&state, "endorse.done");
    let (mut ok, mut first) = (0, None);
    for (id, version) in mods {
        match state.nexus.endorse(id, &version).await {
            Ok(_) => { ok += 1; done.insert(id); }
            Err(Error::Http { status: 403, .. }) => { first.get_or_insert_with(|| "Nexus takes an endorsement only after you downloaded the mod from it (and a little time has passed).".to_string()); }
            Err(e @ Error::Throttled { .. }) => { first.get_or_insert(e.to_string()); break; }
            Err(e) => { first.get_or_insert(e.to_string()); }
        }
    }
    put_ids(&state, "endorse.done", &done)?;
    Ok((ok, first))
}

/// "Not for me": never offer these mods for endorsing again.
#[tauri::command]
pub async fn endorse_decline(state: State<'_, AppState>, mod_ids: Vec<u32>) -> Result<()> {
    let mut no = ids(&state, "endorse.no");
    no.extend(mod_ids);
    put_ids(&state, "endorse.no", &no)
}

/// Remember one endorsed from elsewhere in the app (a mod's page or menu).
#[tauri::command]
pub async fn endorse_mark(state: State<'_, AppState>, mod_id: u32) -> Result<()> {
    let mut done = ids(&state, "endorse.done");
    done.insert(mod_id);
    put_ids(&state, "endorse.done", &done)
}
