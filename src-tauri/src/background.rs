//! Running in the background. With it on (the default), closing the window turns the picture off and then destroys
//! the window, webview and all, so nothing is drawn and almost no memory is held; the app stays in the tray, where
//! the Rust side keeps the download queue, nxm:// links and the checks below going. Opening it again makes a new
//! window, which turns on and comes back where it was (the page keeps its place in local storage).
//!
//! Every six hours (first a minute after start), quietly:
//! - only when the user turned it on ("mods.background", off by default, because Nexus's API policy has a user's
//!   key used only for what the user starts): mod updates are checked; for a Premium account their files are
//!   downloaded ahead ("mods.prefetch"), so installing them is one click and instant; with "mods.autoInstall" on
//!   they also go in, but only while the window is closed and the game is not running. Check now always does this.
//! - a new Pockymod is looked for ("app.updates": "auto", the default, downloads it and installs it when you are
//!   away: when the window is closed, or when you quit; "ask" downloads it and waits for a click; "off" does nothing).
//!   It installs silently and starts again in the background, so the next time you open it, it is just newer.
//!
//! Nothing here shows a window or a prompt. The settings are in Settings, Background.

use crate::state::{AppState, Paths};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Default)]
pub struct Background {
    /// A downloaded installer for a newer version, and that version.
    pub ready: Mutex<Option<(PathBuf, String)>>,
    /// Mod updates whose files are downloaded and waiting for Install.
    pub mods_ready: Mutex<usize>,
    /// Quitting for real (not closing to the tray).
    pub quitting: std::sync::atomic::AtomicBool,
}

fn setting(app: &AppHandle, key: &str) -> Option<String> { app.state::<AppState>().db.lock().unwrap().setting(key).ok().flatten() }
/// Closing sends the app to the tray unless turned off.
pub fn enabled(app: &AppHandle) -> bool { setting(app, "app.background").as_deref() != Some("0") }
fn app_updates(app: &AppHandle) -> String { setting(app, "app.updates").unwrap_or_else(|| "auto".into()) }
pub fn window_open(app: &AppHandle) -> bool { app.get_webview_window("main").is_some() }

/// Started with `--background` (after an update, or at sign-in): no window until the tray is used.
pub fn started_hidden() -> bool { std::env::args().any(|a| a == "--background") }

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            round(&app, false).await;
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// Mod updates are looked for on the timer only when the user turned that on (Settings, Background).
pub fn mod_checks_on(app: &AppHandle) -> bool { setting(app, "mods.background").as_deref() == Some("1") }

/// One quiet round of checks; `asked` when the user pressed Check now.
pub async fn round(app: &AppHandle, asked: bool) {
    let st = app.state::<AppState>();
    // mod updates
    let list = if asked || mod_checks_on(app) { pocket_core::updates::check(&st.nexus, &st.db).await.ok() } else { None };
    if let Some(list) = list {
        let _ = app.emit("updates", &list);
        let premium = st.user.read().await.as_ref().is_some_and(|u| u.is_premium);
        if premium && setting(app, "mods.prefetch").as_deref() != Some("0") {
            let mut ready = 0;
            for u in list.iter().filter(|u| u.file_id > 0) {
                if prefetch(app, u.mod_id, u.file_id).await.is_ok() { ready += 1; }
            }
            *app.state::<Background>().mods_ready.lock().unwrap() = ready;
            crate::tray::refresh(app);
            if ready > 0 && setting(app, "mods.autoInstall").as_deref() == Some("1") && !window_open(app) && !pocket_core::launch::game_running() {
                for u in list.iter().filter(|u| u.file_id > 0) {
                    let _ = crate::commands::library::queue_add(app.clone(), app.state::<AppState>(), u.mod_id, u.file_id, true).await;
                }
            }
        }
    }
    // a new version of the app
    let mode = app_updates(app);
    if mode == "off" { return; }
    let Ok(u) = pocket_core::appupdate::check(st.nexus.http()).await else { return };
    if !u.newer { return; }
    let Some(dir) = app.path().app_data_dir().ok().map(|d| d.join("updates")) else { return };
    if let Ok(path) = pocket_core::appupdate::download(st.nexus.http(), &u, &dir).await {
        let v = u.latest.clone().unwrap_or_default();
        *app.state::<Background>().ready.lock().unwrap() = Some((path, v.clone()));
        let _ = app.emit("app-update", serde_json::json!({ "version": v, "ready": true }));
        crate::tray::refresh(app);
        // away: put it in now
        if mode == "auto" && !window_open(app) { install_now(app); }
    }
}

/// Download a mod update's file without installing it; installing it later finds it and skips the download.
pub async fn prefetch(app: &AppHandle, mod_id: u32, file_id: u32) -> pocket_core::Result<()> {
    let st = app.state::<AppState>();
    if pocket_core::install::downloaded_archive(&st.db.lock().unwrap(), mod_id, file_id)?.is_some() { return Ok(()); }
    let (_, library) = Paths::load(&st.db.lock().unwrap()).require()?;
    let d = pocket_core::browse::detail(&st.nexus, &st.db, mod_id, None).await?;
    let f = d.files.iter().find(|f| f.file_id == file_id).ok_or_else(|| pocket_core::Error::Invalid("That file is gone from Nexus".into()))?;
    let links = st.nexus.download_links(mod_id, file_id, None).await?;
    let url = links.first().map(|l| l.uri.clone()).ok_or_else(|| pocket_core::Error::Invalid("Nexus gave no download link".into()))?;
    let name = pocket_core::queue::download_name(mod_id, file_id, &f.uri);
    let dl = pocket_core::download::download(st.nexus.download_http(), &url, &library.join("downloads"), &name, |_, _| true).await?;
    pocket_core::install::record_download(&st.db.lock().unwrap(), mod_id, file_id, &f.name, &f.version, &dl.path, dl.size, &dl.md5)?;
    Ok(())
}

/// Install the downloaded version now: the installer runs silently once this app has quit, then starts the new
/// version in the background (or with its window, when `show` is set). Windows only.
pub fn install_now_with(app: &AppHandle, show: bool) -> bool {
    let Some((path, _)) = app.state::<Background>().ready.lock().unwrap().clone() else { return false };
    let Ok(exe) = std::env::current_exe() else { return false };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let then = if show { String::new() } else { " --background".into() };
        // wait for this process to go, install, start again; no console window, no prompt
        let line = format!("/C ping -n 3 127.0.0.1 >nul & \"{}\" /S & start \"\" \"{}\"{}", path.display(), exe.display(), then);
        if std::process::Command::new("cmd").raw_arg(line).creation_flags(CREATE_NO_WINDOW).spawn().is_ok() {
            app.exit(0);
            return true;
        }
        false
    }
    #[cfg(not(windows))]
    { let _ = (path, exe, show); false }
}
/// The same, while away: only when no download is running or waiting, so an update never cuts one off.
pub fn install_now(app: &AppHandle) -> bool {
    let busy = app.state::<AppState>().queue.lock().unwrap().items.iter().any(|i| i.is_open());
    !busy && install_now_with(app, false)
}

/// The state for Settings and the update notice.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundState { pub ready_version: Option<String>, pub mods_ready: usize }

#[tauri::command]
pub async fn background_state(app: AppHandle) -> pocket_core::Result<BackgroundState> {
    let b = app.state::<Background>();
    let ready_version = b.ready.lock().unwrap().as_ref().map(|(_, v)| v.clone());
    let mods_ready = *b.mods_ready.lock().unwrap();
    Ok(BackgroundState { ready_version, mods_ready })
}

/// Restart into the downloaded version now (the user pressed it).
#[tauri::command]
pub async fn app_update_install(app: AppHandle) -> pocket_core::Result<()> {
    if install_now_with(&app, true) { Ok(()) } else { Err(pocket_core::Error::Invalid("No new version downloaded yet".into())) }
}

/// Run the checks now (Settings, Background, Check now).
#[tauri::command]
pub async fn background_check(app: AppHandle) -> pocket_core::Result<BackgroundState> {
    round(&app, true).await;
    background_state(app).await
}

/// Start with Windows, in the background (off by default).
#[tauri::command]
pub async fn autostart_set(on: bool) -> pocket_core::Result<bool> { pocket_core::platform::set_autostart(&std::env::current_exe()?, on) }

#[tauri::command]
pub async fn autostart_get() -> pocket_core::Result<bool> { pocket_core::platform::autostart(&std::env::current_exe()?) }
