//! Collection commands: browse Nexus, check before install, install, and local create, export and import.
use crate::runner::{self, QueueSnapshot};
use crate::state::AppState;
use pocket_core::collections::{self, Check, CollectionQuery, CollectionsPage, LocalCollection, Revision};
use pocket_core::{Error, Result};
use tauri::{AppHandle, State};

async fn patch_and_premium(state: &State<'_, AppState>) -> (Option<String>, bool) {
    let premium = state.user.read().await.as_ref().is_some_and(|u| u.is_premium);
    let (g, setting) = { let d = state.db.lock().unwrap(); (crate::state::Paths::load(&d).game_dir, d.setting("game.patch").ok().flatten().filter(|s| !s.is_empty())) };
    let detected = g.and_then(|g| pocket_core::cyberpunk::game::inspect(&g).detected_patch);
    (detected.or(setting).or_else(|| Some(pocket_core::cyberpunk::patches::latest().version.to_string())), premium)
}

#[tauri::command]
pub async fn collections_browse(state: State<'_, AppState>, query: CollectionQuery) -> Result<CollectionsPage> {
    collections::browse(&state.nexus, &state.db, &query).await
}

#[tauri::command]
pub async fn collection_revision(state: State<'_, AppState>, slug: String, revision: Option<u32>) -> Result<Revision> {
    collections::revision(&state.nexus, &slug, revision).await
}

/// A collection's description and header picture, for its page.
#[tauri::command]
pub async fn collection_detail(state: State<'_, AppState>, slug: String) -> Result<collections::Detail> {
    Ok(collections::detail(&state.nexus, &slug).await)
}

/// Pre-install checks for a Nexus collection: game version, what is missing, unavailable files and their
/// replacements (checked for up to 40 mods), clicks needed and download size.
#[tauri::command]
pub async fn collection_check(app: AppHandle, state: State<'_, AppState>, slug: String, revision: Option<u32>) -> Result<Check> {
    let rev = collections::revision(&state.nexus, &slug, revision).await?;
    let (patch, premium) = patch_and_premium(&state).await;
    let mut c = collections::check_revision(&state.db.lock().unwrap(), &rev, patch, premium)?;
    // archived and removed files: look for the mod's current file
    let gone: Vec<usize> = c.entries.iter().enumerate().filter(|(_, e)| e.status == "unavailable").map(|(i, _)| i).take(40).collect();
    for i in gone {
        if let Ok(d) = pocket_core::browse::detail(&state.nexus, &state.db, c.entries[i].mod_id, None).await {
            if let Some(f) = pocket_core::browse::main_file(&d.files) {
                let e = &mut c.entries[i];
                e.replacement = Some((f.file_id, f.version.clone()));
                e.uri = f.uri.clone();
                e.size = f.bytes();
            }
        }
    }
    let entries = std::mem::take(&mut c.entries);
    let mut fresh = collections::check_revision(&state.db.lock().unwrap(), &rev, c.current_patch.clone(), premium)?;
    fresh.entries = entries;
    fresh.manifest = manifest(&app, &state, &slug, Some(rev.revision_number)).await;
    let n = fresh.entries.iter().filter(|e| e.status == "unavailable" && e.replacement.is_none()).count();
    fresh.unavailable = n;
    Ok(fresh)
}

#[tauri::command]
pub async fn collection_install(app: AppHandle, state: State<'_, AppState>, check: Check, include_optional: bool) -> Result<QueueSnapshot> {
    let items = collections::queue_items(&check, include_optional, &check.name);
    if items.is_empty() { return Err(Error::Invalid("Everything in this collection is already installed.".into())); }
    // the curator's installer picks and rules go in with the mods (best effort: without them each mod gets its defaults)
    if let Some(slug) = &check.slug {
        if let Some(m) = manifest(&app, &state, slug, check.revision).await {
            let names = check.entries.iter().map(|e| (e.mod_id, e.name.clone())).collect();
            let _ = pocket_core::collection_manifest::store(&state.db.lock().unwrap(), &m, &names);
        }
    }
    {
        let premium = state.user.read().await.as_ref().is_some_and(|u| u.is_premium);
        let mut q = state.queue.lock().unwrap();
        q.premium = premium;
        for i in items { q.add(i); }
    }
    state.pump.notify_one();
    runner::emit(&app);
    Ok(runner::snapshot(&app))
}

#[tauri::command]
pub async fn local_collection_create(state: State<'_, AppState>, name: String, description: String) -> Result<LocalCollection> {
    let (patch, _) = patch_and_premium(&state).await;
    collections::create_local(&state.db.lock().unwrap(), &name, &description, patch, None)
}

#[tauri::command]
pub fn local_collection_export(collection: LocalCollection, path: String) -> Result<()> {
    let text = serde_json::to_string_pretty(&collection).map_err(|e| Error::Invalid(e.to_string()))?;
    std::fs::write(&path, text)?;
    Ok(())
}

#[tauri::command]
pub async fn local_collection_import(state: State<'_, AppState>, path: String) -> Result<(LocalCollection, Check)> {
    let text = std::fs::read_to_string(&path)?;
    let c = collections::parse_local(&text)?;
    let (patch, premium) = patch_and_premium(&state).await;
    let mut check = collections::check_local(&state.db.lock().unwrap(), &c, patch, premium)?;
    collections::resolve_files(&state.nexus, &state.db, &mut check, 60, premium).await?;
    Ok((c, check))
}

/// A collection's manifest: from the cache (a day), else downloaded once. None when it cannot be had.
async fn manifest(app: &AppHandle, state: &State<'_, AppState>, slug: &str, revision: Option<u32>) -> Option<pocket_core::collection_manifest::Manifest> {
    use tauri::Manager;
    let key = format!("collection.manifest:{slug}:{}", revision.map(|r| r.to_string()).unwrap_or_default());
    let now = pocket_core::db::now();
    if let Some(t) = state.db.lock().unwrap().cache_get(&key, 86400, now).ok().flatten() { return pocket_core::collection_manifest::parse(&t).ok(); }
    let tmp = app.path().app_cache_dir().ok()?.join("collections");
    let m = pocket_core::collection_manifest::fetch(&state.nexus, slug, revision, &tmp).await.ok().flatten()?;
    let _ = state.db.lock().unwrap().cache_put(&key, &m.text, now);
    Some(m)
}
