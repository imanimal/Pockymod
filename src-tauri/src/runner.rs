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

pub fn emit(app: &AppHandle) { let _ = app.emit("queue", snapshot(app)); }

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
    WebviewWindowBuilder::new(app, "nexus", WebviewUrl::External(url))
        .title("Nexus Mods · Pocket Mods")
        .inner_size(1180.0, 820.0)
        .min_inner_size(720.0, 520.0)
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
    tauri::async_runtime::spawn(async move {
        let pump = app.state::<AppState>().pump.clone();
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
                    if let Some(i) = q.get_mut(id) { i.nxm = None; }
                    q.set(id, ItemState::Failed { message: e.to_string() });
                }
            }
            drop(q);
            emit(app);
            true
        }
    }
}

async fn fetch_and_install(app: &AppHandle, item: &QueueItem) -> pocket_core::Result<ItemState> {
    let st = app.state::<AppState>();
    let paths = Paths::load(&st.db.lock().unwrap());
    let (game, library) = paths.require()?;
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
            let dl = download::download(st.nexus.http(), &url, &library.join("downloads"), &pocket_core::queue::download_name(item.mod_id, item.file_id, &item.uri), move |rec, total| {
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
            }).await?;
            install::record_download(&st.db.lock().unwrap(), item.mod_id, item.file_id, &item.file_name, &item.version, &dl.path, dl.size, &dl.md5)?;
            dl.path
        }
    };

    st.queue.lock().unwrap().set(item.id, ItemState::Installing);
    emit(app);
    let req = InstallRequest { mod_id: item.mod_id, file_id: item.file_id, name: item.mod_name.clone(), version: item.version.clone(), archive, accept_unknown: false, replace_older: true, options: paths.deploy_options() };
    let db = st.db.clone();
    let out = tauri::async_runtime::spawn_blocking(move || install::install(&db.lock().unwrap(), &game, &library, &req)).await
        .map_err(|e| Error::Invalid(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    Ok(match out {
        InstallOutcome::Installed { install_id, needs_redmod_deploy, warnings, .. } => ItemState::Done {
            install_id: Some(install_id),
            note: if needs_redmod_deploy { Some("REDmod deploy needed: run it from Health or Load order.".into()) } else { warnings.first().cloned() },
        },
        InstallOutcome::Staged { install_id, .. } => ItemState::Done { install_id: Some(install_id), note: Some("Downloaded, not deployed yet: it clashes with installed mods. Decide in Library.".into()) },
        InstallOutcome::NeedsReview { plan } => ItemState::Failed { message: format!("{} files have no known place in the game folder. Open Library, Downloads to install it anyway.", plan.unknown.len()) },
    })
}
