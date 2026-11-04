//! Themes and extensions the user installs, and the marketplace they come from.
//!
//! Installed add-ons are JSON files in the app's data folder: `addons/theme/<id>.json`, `addons/extension/<id>.json`.
//! The marketplace is a folder of JSON files in this project's repository (`marketplace/` on the main branch), read
//! over HTTPS and cached for an hour so it works offline once seen.
use crate::state::AppState;
use pocket_core::{Error, Result};
use std::path::PathBuf;
use tauri::{Manager, State};

pub const MARKET_BASE: &str = "https://raw.githubusercontent.com/imanimal/pocket-mods/main/marketplace/";
const MAX: usize = 4 * 1024 * 1024;

fn kind_dir(app: &tauri::AppHandle, kind: &str) -> Result<PathBuf> {
    if !matches!(kind, "theme" | "extension") { return Err(Error::Invalid(format!("Unknown add-on kind {kind}."))); }
    let base = app.path().app_data_dir().map_err(|e| Error::Invalid(e.to_string()))?;
    let dir = base.join("addons").join(kind);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Ids are lowercase letters, digits, dots and dashes, so they are safe file names.
pub(crate) fn check_id(id: &str) -> Result<()> {
    let ok = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && !id.starts_with('.') && !id.contains("..");
    if ok { Ok(()) } else { Err(Error::Invalid("An add-on id uses lowercase letters, digits, dots and dashes (for example my-theme).".into())) }
}

/// Every installed add-on of a kind, as the JSON text it was saved with.
#[tauri::command]
pub async fn addon_list(app: tauri::AppHandle, kind: String) -> Result<Vec<String>> {
    let dir = kind_dir(&app, &kind)?;
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir)?.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("json") {
            if let Ok(t) = std::fs::read_to_string(&p) { out.push(t); }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn addon_save(app: tauri::AppHandle, kind: String, id: String, json: String) -> Result<()> {
    check_id(&id)?;
    if json.len() > MAX { return Err(Error::Invalid("That add-on is too large.".into())); }
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| Error::Invalid(format!("That is not valid JSON: {e}")))?;
    let dir = kind_dir(&app, &kind)?;
    let tmp = dir.join(format!("{id}.json.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join(format!("{id}.json")))?;
    Ok(())
}

#[tauri::command]
pub async fn addon_remove(app: tauri::AppHandle, kind: String, id: String) -> Result<()> {
    check_id(&id)?;
    let f = kind_dir(&app, &kind)?.join(format!("{id}.json"));
    if f.exists() { std::fs::remove_file(f)?; }
    Ok(())
}

/// A file from the marketplace folder, for example `index.json` or `themes/neon.json`. Cached for an hour; an old
/// copy is used when offline.
#[tauri::command]
pub async fn market_get(state: State<'_, AppState>, path: String, fresh: Option<bool>) -> Result<String> {
    Ok(fetch(&state, &path, fresh.unwrap_or(false)).await?.0)
}

/// A marketplace file with where it came from: `network`, `cache` (fetched in the last hour) or `offline` (an older
/// copy, used because the marketplace could not be reached).
#[derive(serde::Serialize)]
pub struct MarketFile { text: String, source: &'static str }

#[tauri::command]
pub async fn market_fetch(state: State<'_, AppState>, path: String, fresh: Option<bool>) -> Result<MarketFile> {
    let (text, source) = fetch(&state, &path, fresh.unwrap_or(false)).await?;
    Ok(MarketFile { text, source })
}

async fn fetch(state: &State<'_, AppState>, path: &str, fresh: bool) -> Result<(String, &'static str)> {
    let safe = !path.is_empty() && !path.contains("..") && !path.starts_with('/') && !path.contains('\\')
        && path.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c)) && path.ends_with(".json");
    if !safe { return Err(Error::Invalid("That is not a marketplace file.".into())); }
    let key = format!("market:{path}");
    let now = pocket_core::db::now();
    if !fresh {
        if let Some(hit) = state.db.lock().unwrap().cache_get(&key, 3600, now)? { return Ok((hit, "cache")); }
    }
    let res = state.nexus.http().get(format!("{MARKET_BASE}{path}")).send().await;
    match res {
        Ok(r) if r.status().is_success() => {
            let body = r.text().await?;
            if body.len() > MAX { return Err(Error::Invalid("That marketplace file is too large.".into())); }
            state.db.lock().unwrap().cache_put(&key, &body, now)?;
            Ok((body, "network"))
        }
        other => {
            // offline or not published yet: any old copy beats nothing
            if let Some(old) = state.db.lock().unwrap().cache_get(&key, i64::MAX / 4, now)? { return Ok((old, "offline")); }
            match other {
                Ok(r) => Err(Error::Http { status: r.status().as_u16(), message: format!("The marketplace has no {path} yet.") }),
                Err(e) => Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids() {
        assert!(super::check_id("my-theme.v2").is_ok());
        for bad in ["", "../x", "A", "a/b", ".x", "a..b"] { assert!(super::check_id(bad).is_err(), "{bad}"); }
    }
}
