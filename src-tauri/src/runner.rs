//! The download queue runner and the in-app Nexus window.
//!
//! Free accounts: the file's Nexus page opens in an app window ("nexus"); the user presses "Mod Manager Download"
//! there, the window's navigation to the nxm:// link is caught (it never reaches the OS), and the queue goes on.
//! Links from outside (the browser, via the registered handler) arrive through the single-instance hook.
//! Premium accounts: links are fetched directly and nothing waits on the user.

use crate::state::{AppState, Paths};
use pocket_core::install::{self, InstallOutcome, InstallRequest};
use pocket_core::nexus::nxm::NxmLink;
use pocket_core::queue::{Action, ItemState, QueueItem, Summary};
use pocket_core::{browse, download, Error};
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

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

/// Show a Nexus page in the app's own Nexus window (WebView2). Never the system browser.
pub fn open_nexus(app: &AppHandle, url: &str) -> tauri::Result<()> {
    let url: url::Url = url.parse().map_err(|_| tauri::Error::InvalidWebviewUrl("bad Nexus URL"))?;
    if let Some(w) = app.get_webview_window("nexus") {
        w.navigate(url)?;
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let handle = app.clone();
    let loading = app.clone();
    let _ = app.emit("nexus-window", "loading");
    WebviewWindowBuilder::new(app, "nexus", WebviewUrl::External(url))
        .title("Nexus Mods · Pocket Mods")
        .inner_size(1180.0, 820.0)
        .min_inner_size(720.0, 520.0)
        // Nexus's own dark grey while the page loads, not a white flash
        .background_color(tauri::window::Color(0x1d, 0x1d, 0x21, 0xff))
        // "Manual download" on a Nexus page: the file goes to the library's downloads folder, and the app offers to
        // install it when it is done (instead of it landing in Windows' Downloads, forgotten)
        .on_download({
            let h = app.clone();
            move |_, ev| {
                match ev {
                    tauri::webview::DownloadEvent::Requested { destination, .. } => {
                        let paths = { let st = h.state::<AppState>(); let p = Paths::load(&st.db.lock().unwrap()); p };
                        if let Ok((_, lib)) = paths.require() {
                            let name = destination.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "download.zip".into());
                            let dir = lib.join("downloads");
                            let _ = std::fs::create_dir_all(&dir);
                            *destination = dir.join(name);
                        }
                    }
                    tauri::webview::DownloadEvent::Finished { path, success, .. } => {
                        let _ = h.emit("manual-download", serde_json::json!({ "path": path.map(|p| p.to_string_lossy().into_owned()), "success": success }));
                    }
                    _ => {}
                }
                true
            }
        })
        .on_page_load(move |_, p| {
            let state = if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) { "ready" } else { "loading" };
            let _ = loading.emit("nexus-window", state);
        })
        .on_navigation(move |u| {
            if u.scheme() == "nxm" {
                let s = u.to_string();
                let h = handle.clone();
                tauri::async_runtime::spawn(async move { handle_nxm(&h, &s).await });
                return false;
            }
            true
        })
        .build()?;
    Ok(())
}

/// An nxm:// link from the Nexus window or the OS.
pub async fn handle_nxm(app: &AppHandle, raw: &str) {
    let st = app.state::<AppState>();
    let link = match NxmLink::parse(raw) {
        Ok(l) if l.is_for_this_game() && l.mod_id == 0 && l.file_id == 0 => { let _ = app.emit("toast", "The test link reached Pocket Mods: browser downloads will come here."); return; }
        Ok(l) if l.is_for_this_game() => l,
        Ok(l) => { let _ = app.emit("toast", format!("That link is for {}, not Cyberpunk 2077.", l.game)); return; }
        Err(e) => { let _ = app.emit("toast", e.to_string()); return; }
    };
    // a link for something not queued: look the file up so the item has names and a size
    let known = st.queue.lock().unwrap().items.iter().any(|i| i.is_open() && i.mod_id == link.mod_id);
    let fallback = if known { None } else {
        browse::detail(&st.nexus, &st.db, link.mod_id, None).await.ok().and_then(|d| {
            d.files.iter().find(|f| f.file_id == link.file_id).map(|f| QueueItem {
                id: 0, mod_id: link.mod_id, file_id: f.file_id, mod_name: d.detail.name.clone(), file_name: f.name.clone(), version: f.version.clone(),
                uri: f.uri.clone(), size: f.bytes(), required_by: None, manual_only: !pocket_core::queue::is_archive_name(&f.uri), state: ItemState::Waiting, nxm: None,
            })
        })
    };
    {
        let mut q = st.queue.lock().unwrap();
        q.accept_nxm(link.clone(), |l| fallback.unwrap_or_else(|| QueueItem {
            id: 0, mod_id: l.mod_id, file_id: l.file_id, mod_name: format!("Mod {}", l.mod_id), file_name: format!("File {}", l.file_id), version: String::new(),
            uri: format!("{}-{}.zip", l.mod_id, l.file_id), size: 0, required_by: None, manual_only: false, state: ItemState::Waiting, nxm: None,
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
                let _ = open_nexus(app, &file_page(item.mod_id, (!item.manual_only).then_some(item.file_id)));
                emit(app);
            }
            false
        }
        Action::FetchLink(_) | Action::DownloadWith(_) => {
            let result = fetch_and_install(app, &item).await;
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
pub fn replaced_by(db: &pocket_core::db::Db, item: &QueueItem) -> Vec<i64> { replaced_by_file(db, item.mod_id, item.file_id) }

pub fn replaced_by_file(db: &pocket_core::db::Db, mod_id: u32, file_id: u32) -> Vec<i64> {
    let files: Vec<pocket_core::nexus::types::ModFile> = db.mod_get(mod_id, None).ok().flatten()
        .and_then(|(_, f)| serde_json::from_str(&f).ok()).unwrap_or_default();
    let others: Vec<(i64, u32, Option<String>)> = db.conn.prepare("SELECT id, file_id, file_name FROM installs WHERE mod_id = ?1").and_then(|mut q| {
        let v = q.query_map(rusqlite::params![mod_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<Vec<_>, _>>();
        v
    }).unwrap_or_default();
    let Some(new) = files.iter().find(|f| f.file_id == file_id) else {
        // no file list: the old behaviour, one copy of the mod
        return others.into_iter().map(|o| o.0).collect();
    };
    others.into_iter().filter(|(_, fid, name)| pocket_core::browse::replaces(&files, new, *fid, name.as_deref())).map(|o| o.0).collect()
}

async fn fetch_and_install(app: &AppHandle, item: &QueueItem) -> pocket_core::Result<ItemState> {
    let st = app.state::<AppState>();
    let paths = Paths::load(&st.db.lock().unwrap());
    let (game, library) = paths.require()?;
    // added for another mod, and the user has it by now (from Vortex, by hand, or an earlier install): keep theirs
    let for_another = item.required_by.is_some();
    if for_another && crate::commands::library::present_mod_ids(&st.db)?.contains(&item.mod_id) {
        return Ok(ItemState::Done { install_id: None, note: Some(format!("You already have {}. Kept yours.", item.mod_name)) });
    }
    st.queue.lock().unwrap().set(item.id, ItemState::Downloading { received: 0, total: Some(item.size) });
    emit(app);

    // already downloaded and intact: skip the download
    let have = install::downloaded_archive(&st.db.lock().unwrap(), item.mod_id, item.file_id)?;
    let archive = match have {
        Some((p, _)) => p,
        None => {
            let nxm = item.nxm.as_ref().and_then(|l| l.key.as_deref().map(|k| (k.to_string(), l.expires.unwrap_or(0))));
            if let Some(l) = &item.nxm { if l.expired(pocket_core::db::now()) { return Err(Error::Invalid("The Nexus link expired. Press Mod Manager Download again.".into())); } }
            let links = st.nexus.download_links(item.mod_id, item.file_id, nxm.as_ref().map(|(k, e)| (k.as_str(), *e))).await?;
            let url = links.first().map(|l| l.uri.clone()).ok_or_else(|| Error::Invalid("Nexus gave no download link for this file.".into()))?;
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

    // the game locks its files: an install waits until it is closed
    if pocket_core::launch::game_running() {
        return Err(Error::Invalid("Downloaded. Close Cyberpunk 2077, then press Retry to install it.".into()));
    }
    st.queue.lock().unwrap().set(item.id, ItemState::Installing);
    emit(app);

    // an update to a mod tracked from Vortex: replace its files inside Vortex's folder so Vortex keeps it
    let keep_vortex = st.db.lock().unwrap().setting("vortex.keepFormat")?.as_deref() != Some("0");
    // the installs of this mod that this file updates (same file line); the others (an optional file, another
    // variant) stay
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
            pocket_core::deploy::DeployOutcome::Deployed { .. } => ItemState::Done { install_id: Some(vid), note: Some("Updated inside Vortex's folder, so Vortex keeps it.".into()) },
            _ => ItemState::Done { install_id: Some(vid), note: Some("Not updated: the new files clash with other mods. The old version is back.".into()) },
        });
    }
    let archive_for_choices = archive.to_string_lossy().into_owned();
    let req = InstallRequest { mod_id: item.mod_id, file_id: item.file_id, name: item.mod_name.clone(), version: item.version.clone(), archive, accept_unknown: false, replace_older: true, options: paths.deploy_options(), fomod_choices: None,
        origin: item.required_by.clone(), keep_existing: for_another, file_name: Some(item.file_name.clone()), replace_only: Some(replace_only) };
    let db = st.db.clone();
    let out = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &game, &library, &req)).await
        .map_err(|e| Error::Invalid(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    if matches!(out, InstallOutcome::Installed { .. }) && st.db.lock().unwrap().setting("downloads.keep")?.as_deref() == Some("0") {
        let _ = std::fs::remove_file(&archive_for_choices);
    }
    Ok(match out {
        InstallOutcome::Installed { install_id, needs_redmod_deploy, warnings, .. } => ItemState::Done {
            install_id: Some(install_id),
            note: if needs_redmod_deploy { Some("REDmod deploy needed: run it from Health or Load order.".into()) } else { warnings.first().cloned() },
        },
        InstallOutcome::AlreadyHave { install_id, .. } => ItemState::Done { install_id, note: Some(format!("You already have {}. Kept yours; nothing was replaced.", item.mod_name)) },
        InstallOutcome::Staged { install_id, .. } => ItemState::Done { install_id: Some(install_id), note: Some("Downloaded, not deployed yet: it clashes with installed mods. Decide in Library.".into()) },
        InstallOutcome::NeedsReview { plan } => ItemState::Failed { message: format!("{} files have no known place in the game folder. Open Library, Downloads to install it anyway.", plan.unknown.len()) },
        InstallOutcome::NeedsChoices { fomod, defaults } => {
            // ask now: the app opens the installer's options
            let _ = app.emit("install-choices", serde_json::json!({
                "request": { "modId": item.mod_id, "fileId": item.file_id, "name": item.mod_name, "version": item.version, "archive": archive_for_choices },
                "fomod": fomod, "defaults": defaults }));
            ItemState::Done { install_id: None, note: Some("This mod has installer options. Choose them in the window that opened (or in Library, Downloaded).".into()) }
        }
    })
}
