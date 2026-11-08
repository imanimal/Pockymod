//! Account commands: signing in with Nexus, or a personal API key.
use crate::state::AppState;
use pocket_core::credentials::Secret;
use pocket_core::nexus::client::Auth;
use pocket_core::nexus::oauth;
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
    /// "nexus" for a sign-in, "key" for a pasted key.
    pub signed_in_with: Option<&'static str>,
    /// A personal key is in Credential Manager (the creator tools' Upload API needs one, signed in or not).
    pub key_saved: bool,
    /// Nexus has issued the app's client ID, so Sign in with Nexus works.
    pub sign_in_available: bool,
}

/// Keep the account and its Premium state. Going Premium starts the files that were waiting for a click.
async fn set_user(app: &tauri::AppHandle, state: &State<'_, AppState>, u: NexusUser) {
    let upgraded = { let mut q = state.queue.lock().unwrap(); let was = q.premium; q.premium = u.is_premium; u.is_premium && !was };
    *state.user.write().await = Some(u);
    if upgraded { crate::runner::emit(app); state.pump.notify_one(); }
}

#[tauri::command]
pub async fn nexus_status(app: tauri::AppHandle, state: State<'_, AppState>, refresh: Option<bool>) -> Result<NexusStatus> {
    let has_key = state.nexus.has_key();
    if has_key && (refresh.unwrap_or(false) || state.user.read().await.is_none()) {
        // validate once per session; a bad key shows as no user, the error goes to the caller
        match state.nexus.validate().await {
            Ok(u) => set_user(&app, &state, u).await,
            Err(Error::SignedOut) => { *state.user.write().await = None; return Err(Error::SignedOut); }
            Err(e @ (Error::BadKey | Error::Throttled { .. })) => return Err(e),
            Err(_) => {}
        }
    }
    let signed_in_with = match state.nexus.auth() { Some(Auth::SignIn(_)) => Some("nexus"), Some(Auth::Key(_)) => Some("key"), None => None };
    Ok(NexusStatus { has_key: state.nexus.has_key(), user: state.user.read().await.clone(), rate_limit: state.nexus.rate_limit(),
        signed_in_with, key_saved: state.keys.get().ok().flatten().is_some(), sign_in_available: oauth::client_id().is_some() })
}

/// Sign in with Nexus: Nexus's approval page opens in the browser, and the app waits for the browser to come back.
/// The refresh token goes to Windows Credential Manager. A pasted key stays saved for the creator tools.
#[tauri::command]
pub async fn nexus_sign_in(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<NexusStatus> {
    let client_id = oauth::client_id().ok_or_else(|| Error::Invalid("Signing in with Nexus isn't available yet. Paste an API key instead".into()))?;
    let (tx, cancelled) = tokio::sync::oneshot::channel();
    // a second click starts over; the first attempt lets go of the port first
    let earlier = state.sign_in_cancel.lock().unwrap().replace(tx);
    if let Some(old) = earlier { let _ = old.send(()); tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
    let pkce = oauth::Pkce::new()?;
    let st = oauth::new_state()?;
    let cb = oauth::Callback::bind(oauth::callback_port()).await?;
    let redirect = oauth::redirect_uri(cb.port);
    crate::commands::extensions::open_in_browser(&oauth::authorize_url(&client_id, &redirect, &st, &pkce.challenge))?;
    let code = tokio::select! {
        r = tokio::time::timeout(oauth::GIVE_UP, cb.code(&st)) => r.map_err(|_| Error::Invalid("No answer from Nexus. Try signing in again".into()))??,
        _ = cancelled => return Err(Error::Cancelled),
    };
    let tokens = oauth::exchange(state.nexus.http(), oauth::TOKEN_URL, &client_id, &code, &pkce.verifier, &redirect).await?;
    match &tokens.refresh { Some(r) => state.sign_in.set(r)?, None => state.sign_in.clear()? }
    let previous = state.nexus.auth();
    state.nexus.set_auth(Some(Auth::SignIn(tokens.clone())));
    // the account as the API reports it, else as the token names it
    let user = match state.nexus.validate().await {
        Ok(u) => Some(u),
        Err(e @ (Error::SignedOut | Error::Throttled { .. })) => {
            state.nexus.set_auth(previous);
            let _ = state.sign_in.clear();
            return Err(e);
        }
        Err(_) => oauth::user_from_token(&tokens.access),
    };
    match user { Some(u) => set_user(&app, &state, u).await, None => *state.user.write().await = None }
    if let Some(w) = tauri::Manager::get_webview_window(&app, "main") { let _ = w.unminimize(); let _ = w.set_focus(); }
    nexus_status(app, state, None).await
}

/// Stop waiting for the browser.
#[tauri::command]
pub fn nexus_sign_in_cancel(state: State<'_, AppState>) {
    if let Some(tx) = state.sign_in_cancel.lock().unwrap().take() { let _ = tx.send(()); }
}

/// Check the key with Nexus, then store it in Windows Credential Manager. A key Nexus rejects is not stored.
/// While signed in with Nexus the sign-in stays in use, and the key is kept for the creator tools.
#[tauri::command]
pub async fn nexus_set_key(app: tauri::AppHandle, state: State<'_, AppState>, key: String) -> Result<NexusStatus> {
    let key = key.trim().to_owned();
    if key.len() < 16 || key.chars().any(char::is_whitespace) { return Err(Error::Invalid("That isn't a Nexus API key".into())); }
    let previous = state.nexus.auth();
    state.nexus.set_key(Some(Secret::new(key.clone())));
    match state.nexus.validate().await {
        Ok(user) => {
            state.keys.set(&Secret::new(key))?;
            match previous {
                Some(signed_in @ Auth::SignIn(_)) => state.nexus.set_auth(Some(signed_in)),
                _ => {
                    state.queue.lock().unwrap().premium = user.is_premium;
                    *state.user.write().await = Some(user);
                }
            }
            nexus_status(app, state, None).await
        }
        Err(e) => {
            state.nexus.set_auth(previous);
            Err(e)
        }
    }
}

/// Sign out: forget the sign-in and the key.
#[tauri::command]
pub async fn nexus_clear_key(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<NexusStatus> {
    if let (Some(Auth::SignIn(t)), Some(id)) = (state.nexus.auth(), oauth::client_id()) {
        let http = state.nexus.http().clone();
        if let Some(r) = t.refresh { tauri::async_runtime::spawn(async move { oauth::revoke(&http, &id, &r).await }); }
    }
    state.sign_in.clear()?;
    state.keys.clear()?;
    state.nexus.set_auth(None);
    *state.user.write().await = None;
    state.queue.lock().unwrap().premium = false;
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
        Error::Http { status: 403, .. } => Error::Invalid("Download the mod first, then wait a few minutes to endorse".into()),
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
            Err(Error::Http { status: 403, .. }) => { first.get_or_insert_with(|| "Download the mod from Nexus first, then wait a few minutes to endorse".to_string()); }
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
