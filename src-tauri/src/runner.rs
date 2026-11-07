//! The download queue runner and the in-app Nexus window.
//!
//! Free accounts: the file's Nexus page opens where the user wants pages (browser.rs): in their own browser, whose
//! "Mod Manager Download" link comes back through the registered nxm:// handler and the single-instance hook, or in
//! the app's browser pane, whose navigation to the link is caught (it never reaches the OS). Then the queue goes on.
//! Premium accounts: links are fetched directly and nothing waits on the user.

use crate::state::{AppState, Paths};
use pocket_core::install::{self, InstallOutcome, InstallRequest};
use pocket_core::nexus::nxm::NxmLink;
use pocket_core::queue::{Action, ItemState, QueueItem, Summary};
use pocket_core::{browse, download, Error};
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot { pub items: Vec<QueueItem>, pub summary: Summary }

pub fn snapshot(app: &AppHandle) -> QueueSnapshot {
    let st = app.state::<AppState>();
    let q = st.queue.lock().unwrap();
    QueueSnapshot { items: q.items.clone(), summary: q.summary() }
}

pub fn emit(app: &AppHandle) {
    // every change is saved, so a restart picks the queue up where it was
    let st = app.state::<AppState>();
    { let q = st.queue.lock().unwrap(); let d = st.db.lock().unwrap(); let _ = pocket_core::queue::save(&d, &q); }
    let _ = app.emit("queue", snapshot(app));
}

/// UNVERIFIED: `nmm=1` asks the Files tab to open the "Mod Manager Download" flow for that file (as Vortex does).
pub fn file_page(mod_id: u32, file_id: Option<u32>) -> String {
    match file_id {
        Some(f) => format!("https://www.nexusmods.com/{}/mods/{mod_id}?tab=files&file_id={f}&nmm=1", pocket_core::GAME_DOMAIN),
        None => format!("https://www.nexusmods.com/{}/mods/{mod_id}", pocket_core::GAME_DOMAIN),
    }
}

/// Show a Nexus page where the user wants pages (browser.rs): their own browser, the pane or its window.
pub fn open_nexus(app: &AppHandle, url: &str) -> pocket_core::Result<()> { crate::browser::open(app, url) }

/// The page a free account clicks Mod Manager Download on. In the user's own browser that link comes back through
/// the nxm:// handler, so say when another manager has it.
pub(crate) fn open_click_page(app: &AppHandle, url: &str) {
    if !crate::browser::in_app(app) {
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(h) = pocket_core::platform::nxm_handler_status(&exe) {
                if h.supported && !h.ours {
                    let who = h.owner.unwrap_or_else(|| "another program".into());
                    let _ = app.emit("toast", format!("Mod Manager Download links go to {who}. Take them back in Settings, Nexus pages"));
                }
            }
        }
    }
    if let Err(e) = open_nexus(app, url) { let _ = app.emit("toast", e.to_string()); }
}

/// An nxm:// link from the Nexus window or the OS.
pub async fn handle_nxm(app: &AppHandle, raw: &str) {
    let st = app.state::<AppState>();
    let link = match NxmLink::parse(raw) {
        Ok(l) if l.is_for_this_game() && l.mod_id == 0 && l.file_id == 0 => { let _ = app.emit("toast", "Test link received. Browser downloads come here"); return; }
        Ok(l) if l.is_for_this_game() => l,
        Ok(l) => { let _ = app.emit("toast", format!("That link is for {}, not Cyberpunk 2077", l.game)); return; }
        Err(e) => { let _ = app.emit("toast", e.to_string()); return; }
    };
    // a link for something not queued: look the file up so the item has names and a size
    let known = st.queue.lock().unwrap().items.iter().any(|i| i.is_open() && i.mod_id == link.mod_id);
    let fallback = if known { None } else {
        browse::detail(&st.nexus, &st.db, link.mod_id, None).await.ok().and_then(|d| {
            d.files.iter().find(|f| f.file_id == link.file_id).map(|f| QueueItem {
                id: 0, mod_id: link.mod_id, file_id: f.file_id, mod_name: d.detail.name.clone(), file_name: f.name.clone(), version: f.version.clone(),
                uri: f.uri.clone(), size: f.bytes(), required_by: None, manual_only: !pocket_core::queue::is_archive_name(&f.uri), state: ItemState::Waiting, nxm: None, from_collection: false,
            })
        })
    };
    {
        let mut q = st.queue.lock().unwrap();
        q.accept_nxm(link.clone(), |l| fallback.unwrap_or_else(|| QueueItem {
            id: 0, mod_id: l.mod_id, file_id: l.file_id, mod_name: format!("Mod {}", l.mod_id), file_name: format!("File {}", l.file_id), version: String::new(),
            uri: format!("{}-{}.zip", l.mod_id, l.file_id), size: 0, required_by: None, manual_only: false, state: ItemState::Waiting, nxm: None, from_collection: false,
        }));
    }
    emit(app);
    st.pump.notify_one();
}

/// Start the runner. It sleeps until woken, then works through the queue one item at a time.
pub fn start(app: AppHandle) {
    {
        let st = app.state::<AppState>();
        let saved = { let d = st.db.lock().unwrap(); pocket_core::queue::load(&d) };
        if let Ok(saved) = saved {
            if !saved.items.is_empty() { *st.queue.lock().unwrap() = saved; }
        }
    }
    tauri::async_runtime::spawn(async move {
        let pump = app.state::<AppState>().pump.clone();
        // a saved queue starts again by itself (Premium) or shows its next page (free)
        pump.notify_one();
        loop {
            pump.notified().await;
            loop {
                let action = app.state::<AppState>().queue.lock().unwrap().next_action();
                let Some(action) = action else { break };
                if !step(&app, action).await { break; }
            }
            emit(&app);
        }
    });
}

/// One step. Returns false when the queue now waits on the user.
async fn step(app: &AppHandle, action: Action) -> bool {
    let st = app.state::<AppState>();
    let id = match &action { Action::FetchLink(i) | Action::DownloadWith(i) | Action::OpenPage(i) => *i };
    let Some(item) = st.queue.lock().unwrap().get(id).cloned() else { return true };
    match action {
        Action::OpenPage(_) => {
            if item.state != ItemState::NeedsClick {
                st.queue.lock().unwrap().set(id, ItemState::NeedsClick);
                open_click_page(app, &file_page(item.mod_id, (!item.manual_only).then_some(item.file_id)));
                emit(app);
            }
            false
        }
        Action::FetchLink(_) | Action::DownloadWith(_) => {
            let result = fetch_and_install(app, &item).await;
            st.cancel.lock().unwrap().remove(&id);
            let mut q = st.queue.lock().unwrap();
            match result {
                Ok(state) => q.set(id, state),
                Err(Error::NeedsPremium) => {
                    // the account is not Premium after all: fall back to clicks
                    q.premium = false;
                    q.set(id, ItemState::Waiting);
                }
                Err(e) => {
                    // a refused or expired link is useless; one that broke mid-download still works for a retry
                    let link_bad = matches!(e, Error::Http { .. }) || e.to_string().contains("expired");
                    if let Some(i) = q.get_mut(id) { if link_bad { i.nxm = None; } }
                    q.set(id, ItemState::Failed { message: e.to_string() });
                }
            }
            drop(q);
            emit(app);
            true
        }
    }
}

/// Installs of the item's mod that the item's file updates, from the cached file list.
pub fn replaced_by(db: &pocket_core::db::Db, item: &QueueItem) -> Vec<i64> { replaced_by_file(db, item.mod_id, item.file_id, Some(&item.file_name)) }

pub fn replaced_by_file(db: &pocket_core::db::Db, mod_id: u32, file_id: u32, file_name: Option<&str>) -> Vec<i64> {
    let files: Vec<pocket_core::nexus::types::ModFile> = db.mod_get(mod_id, None).ok().flatten()
        .and_then(|(_, f)| serde_json::from_str(&f).ok()).unwrap_or_default();
    let others: Vec<(i64, u32, Option<String>)> = db.conn.prepare("SELECT id, file_id, file_name FROM installs WHERE mod_id = ?1").and_then(|mut q| {
        let v = q.query_map(rusqlite::params![mod_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<Vec<_>, _>>();
        v
    }).unwrap_or_default();
    let Some(new) = files.iter().find(|f| f.file_id == file_id) else {
        // a file the list does not know: replace only the same file, or one of the same line by name
        let line = file_name.map(pocket_core::browse::file_line).filter(|l| !l.is_empty());
        return others.into_iter().filter(|(_, fid, name)| *fid == file_id
            || matches!((&line, name.as_deref().map(pocket_core::browse::file_line)), (Some(a), Some(b)) if *a == b)).map(|o| o.0).collect();
    };
    others.into_iter().filter(|(_, fid, name)| pocket_core::browse::replaces(&files, new, *fid, name.as_deref())).map(|o| o.0).collect()
}

async fn fetch_and_install(app: &AppHandle, item: &QueueItem) -> pocket_core::Result<ItemState> {
    let st = app.state::<AppState>();
    let paths = Paths::load(&st.db.lock().unwrap());
    let (game, library) = paths.require()?;
    // added for another mod, and the user has it by now (from Vortex, by hand, or an earlier install): keep theirs
    let for_another = item.required_by.is_some();
    // a requirement is met by any copy of the mod the user has (turned on if it is off); a collection asks for this
    // file, so only a copy of the same file counts
    if for_another && !item.from_collection && crate::commands::library::present_mod_ids(&st.db)?.contains(&item.mod_id) {
        let off: Option<i64> = {
            let d = st.db.lock().unwrap();
            let active = install::active_profile(&d)?;
            let any_on: bool = d.conn.query_row("SELECT EXISTS(SELECT 1 FROM installs i JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?2 WHERE i.mod_id = ?1 AND pm.enabled = 1)",
                rusqlite::params![item.mod_id, active], |r| r.get(0))?;
            if any_on { None } else { d.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 ORDER BY installed_at DESC LIMIT 1", [item.mod_id], |r| r.get(0)).ok() }
        };
        if let Some(id) = off {
            let db = st.db.clone();
            let (g, l, o) = (game.clone(), library.clone(), paths.deploy_options());
            let _ = tauri::async_runtime::spawn_blocking(move || pocket_core::profiles::set_enabled(&db.lock().unwrap(), &g, &l, id, true, &o)).await;
            let _ = app.emit("library-changed", ());
            return Ok(ItemState::Done { install_id: Some(id), note: Some(format!("You already have {}. Turned it on", item.mod_name)) });
        }
        return Ok(ItemState::Done { install_id: None, note: Some(format!("You already have {}", item.mod_name)) });
    }
    st.queue.lock().unwrap().set(item.id, ItemState::Downloading { received: 0, total: Some(item.size) });
    emit(app);

    // already downloaded and intact: skip the download
    let have = install::downloaded_archive(&st.db.lock().unwrap(), item.mod_id, item.file_id)?;
    let archive = match have {
        Some((p, _)) => p,
        None => {
            let nxm = item.nxm.as_ref().and_then(|l| l.key.as_deref().map(|k| (k.to_string(), l.expires.unwrap_or(0))));
            if let Some(l) = &item.nxm { if l.expired(pocket_core::db::now()) { return Err(Error::Invalid("The Nexus link expired. Press Mod Manager Download again".into())); } }
            let links = st.nexus.download_links(item.mod_id, item.file_id, nxm.as_ref().map(|(k, e)| (k.as_str(), *e))).await?;
            let url = links.first().map(|l| l.uri.clone()).ok_or_else(|| Error::Invalid("Nexus gave no download link for this file".into()))?;
            let started = Instant::now();
            let mut last = Instant::now();
            let id = item.id;
            let app2 = app.clone();
            let dl = download::download(st.nexus.download_http(), &url, &library.join("downloads"), &pocket_core::queue::download_name(item.mod_id, item.file_id, &item.uri), move |rec, total| {
                if last.elapsed().as_millis() >= 250 {
                    last = Instant::now();
                    let s = app2.state::<AppState>();
                    let mut q = s.queue.lock().unwrap();
                    let secs = started.elapsed().as_secs_f64().max(0.001);
                    q.bytes_per_sec = Some((rec as f64 / secs) as u64);
                    q.set(id, ItemState::Downloading { received: rec, total });
                    drop(q);
                    emit(&app2);
                }
                !app2.state::<AppState>().cancel.lock().unwrap().remove(&id)
            }).await?;
            install::record_download(&st.db.lock().unwrap(), item.mod_id, item.file_id, &item.file_name, &item.version, &dl.path, dl.size, &dl.md5)?;
            dl.path
        }
    };

    // stopped while the archive was already here or being checked
    if st.cancel.lock().unwrap().remove(&item.id) { return Err(Error::Invalid(pocket_core::download::CANCELLED.into())); }
    // the game locks its files: an install waits until it is closed
    if pocket_core::launch::game_running() {
        return Err(Error::Invalid("Downloaded. Close Cyberpunk 2077 and press Retry to install".into()));
    }
    st.queue.lock().unwrap().set(item.id, ItemState::Installing);
    emit(app);

    // an update to a mod tracked from Vortex: replace its files inside Vortex's folder so Vortex keeps it
    let keep_vortex = st.db.lock().unwrap().setting("vortex.keepFormat")?.as_deref() != Some("0");
    // the installs of this mod that this file updates (same file line); the others (an optional file, another
    // variant) stay
    // a new upload is often missing from an old cached file list: fetch it again before deciding what this replaces
    let known = st.db.lock().unwrap().mod_get(item.mod_id, None).ok().flatten()
        .and_then(|(_, f)| serde_json::from_str::<Vec<pocket_core::nexus::types::ModFile>>(&f).ok()).is_some_and(|f| f.iter().any(|x| x.file_id == item.file_id));
    if !known {
        if let Ok((d, f)) = st.nexus.mod_detail(item.mod_id).await {
            let _ = st.db.lock().unwrap().mod_put(item.mod_id, &d.updated_at, &serde_json::to_string(&d).unwrap_or_default(), &serde_json::to_string(&f).unwrap_or_default(), pocket_core::db::now());
        }
    }
    let replace_only = replaced_by(&st.db.lock().unwrap(), item);
    let vortex_id: Option<i64> = if keep_vortex {
        let ids: Vec<i64> = {
            let d = st.db.lock().unwrap();
            let mut q = d.conn.prepare("SELECT id FROM installs WHERE mod_id = ?1 AND source = 'vortex' ORDER BY installed_at DESC")?;
            let v = q.query_map(rusqlite::params![item.mod_id], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
            v
        };
        ids.into_iter().find(|i| replace_only.contains(i))
    } else { None };
    // installer options are asked through the normal path, so the Vortex in-place update is for plain archives
    let has_options = pocket_core::archive::list(&archive).map(|e| pocket_core::fomod::find_config(&e).is_some() || pocket_core::fomod::variants(&e, &item.mod_name).is_some()).unwrap_or(false);
    if let (Some(vid), Some(setup), false) = (vortex_id, pocket_core::vortex::find(&game), has_options) {
        let db = st.db.clone();
        let (g, l, a, uri, fid, ver) = (game.clone(), library.clone(), archive.clone(), pocket_core::queue::download_name(item.mod_id, item.file_id, &item.uri), item.file_id, item.version.clone());
        let out = tauri::async_runtime::spawn_blocking(move || pocket_core::vortex::update_in_place(&db.lock().unwrap(), &g, &l, &setup, vid, &a, &uri, fid, &ver)).await
            .map_err(|e| Error::Invalid(e.to_string()))??;
        let _ = app.emit("library-changed", ());
        return Ok(match out {
            pocket_core::deploy::DeployOutcome::Deployed { .. } => ItemState::Done { install_id: Some(vid), note: Some("Updated in Vortex's folder, so Vortex keeps it".into()) },
            _ => ItemState::Done { install_id: Some(vid), note: Some("Not updated. The new files clash with other mods, so the old version is back".into()) },
        });
    }
    let archive_for_choices = archive.to_string_lossy().into_owned();
    let req = InstallRequest { mod_id: item.mod_id, file_id: item.file_id, name: item.mod_name.clone(), version: item.version.clone(), archive, accept_unknown: false, replace_older: true, options: paths.deploy_options(), fomod_choices: None,
        origin: item.required_by.clone(), keep_existing: for_another, file_name: Some(item.file_name.clone()), replace_only: Some(replace_only),
        same_file_only: item.from_collection };
    let db = st.db.clone();
    let (g2, l2, req2) = (game.clone(), library.clone(), req.clone());
    let mut out = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &g2, &l2, &req2)).await
        .map_err(|e| Error::Invalid(e.to_string()))??;
    // a collection's curator picked this mod's installer options: use them instead of asking
    if let InstallOutcome::NeedsChoices { fomod, .. } = &out {
        let picks = pocket_core::collection_manifest::choices_for(&st.db.lock().unwrap(), item.mod_id, item.file_id);
        let mapped = picks.map(|p| pocket_core::fomod::choices_by_name(fomod, &p)).filter(|c| !c.is_empty());
        if let Some(choices) = mapped {
            let mut again = req.clone();
            again.fomod_choices = Some(choices);
            let db = st.db.clone();
            out = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &game, &library, &again)).await
                .map_err(|e| Error::Invalid(e.to_string()))??;
        }
    }
    let _ = app.emit("library-changed", ());
    if matches!(out, InstallOutcome::Installed { .. }) && st.db.lock().unwrap().setting("downloads.keep")?.as_deref() == Some("0") {
        let _ = std::fs::remove_file(&archive_for_choices);
    }
    Ok(match out {
        InstallOutcome::Installed { install_id, needs_redmod_deploy, warnings, .. } => ItemState::Done {
            install_id: Some(install_id),
            note: if needs_redmod_deploy { Some("REDmod deploy needed. Run it from Health or Load order".into()) } else { warnings.first().cloned() },
        },
        InstallOutcome::AlreadyHave { install_id, .. } => ItemState::Done { install_id, note: Some(format!("You already have {}. Nothing was replaced", item.mod_name)) },
        InstallOutcome::Staged { install_id, .. } => ItemState::Done { install_id: Some(install_id), note: Some("Downloaded but not deployed. It clashes with installed mods, decide in Library".into()) },
        InstallOutcome::NeedsReview { plan } => ItemState::Failed { message: format!("{} files have no known place in the game folder. Install it anyway from Library, Downloads", plan.unknown.len()) },
        InstallOutcome::NeedsChoices { fomod, defaults } => {
            // ask now: the app opens the installer's options
            let _ = app.emit("install-choices", serde_json::json!({
                "request": { "modId": item.mod_id, "fileId": item.file_id, "name": item.mod_name, "version": item.version, "archive": archive_for_choices,
                    "origin": item.required_by, "keepExisting": for_another, "fileName": item.file_name, "sameFileOnly": item.from_collection },
                "fomod": fomod, "defaults": defaults }));
            ItemState::Done { install_id: None, note: Some("This mod has installer options. Choose them in the window that opened".into()) }
        }
    })
}
