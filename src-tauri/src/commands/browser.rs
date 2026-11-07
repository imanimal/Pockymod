//! The browser pane's commands: where it sits, its keys (back, forward, reload, open in your browser, pop out or
//! back in, close), its Nexus sign-in, and its extensions folder. See browser.rs.
use crate::browser::{self, BrowserState};
use pocket_core::{Error, Result};
use tauri::AppHandle;

/// The pane's place in CSS pixels from the page's layout, and whether it shows now.
#[tauri::command]
pub async fn browser_bounds(app: AppHandle, x: f64, y: f64, w: f64, h: f64, visible: bool) -> Result<()> { browser::set_bounds(&app, x, y, w, h, visible) }

#[tauri::command]
pub async fn browser_nav(app: AppHandle, action: String) -> Result<()> { browser::nav(&app, &action) }

/// Open an address in the pane (or its window); only web pages.
#[tauri::command]
pub async fn browser_go(app: AppHandle, url: String) -> Result<()> { browser::open(&app, &url) }

#[tauri::command]
pub async fn browser_state(app: AppHandle) -> Result<BrowserState> { Ok(browser::state(&app)) }

#[tauri::command]
pub async fn browser_close(app: AppHandle) -> Result<()> { browser::close(&app) }

/// Pop the pane out into its own window (out) or back into the main window.
#[tauri::command]
pub async fn browser_pop(app: AppHandle, out: bool) -> Result<()> { browser::pop(&app, out) }

/// The page the pane shows, in the user's own browser (their sign-in, passwords and extensions).
#[tauri::command]
pub async fn browser_external(app: AppHandle) -> Result<()> {
    let u = browser::current(&app).ok_or_else(|| Error::Invalid("No page to open".into()))?;
    crate::commands::extensions::open_in_browser(u.as_str())
}

/// Whether the pane is signed in to Nexus and whether Firefox has a sign-in to copy.
#[tauri::command]
pub async fn browser_sign_in(app: AppHandle) -> Result<browser::SignIn> { browser::sign_in_state(&app).await }

/// Copy the Nexus sign-in from Firefox into the pane (only its two Nexus sign-in cookies).
#[tauri::command]
pub async fn browser_use_firefox(app: AppHandle) -> Result<browser::SignIn> { browser::use_firefox(&app).await }

/// Sign the pane out of Nexus and clear what its profile saved.
#[tauri::command]
pub async fn browser_sign_out(app: AppHandle) -> Result<browser::SignIn> { browser::sign_out(&app).await }

/// The folder the pane loads unpacked Chrome extensions from, made when missing and, with `open`, shown in Explorer.
/// Returns its path and how many extensions are in it.
#[tauri::command]
pub async fn browser_extensions_folder(app: AppHandle, open: bool) -> Result<(String, usize)> {
    let dir = browser::extensions_dir(&app).ok_or_else(|| Error::Invalid("The app's data folder is unknown".into()))?;
    std::fs::create_dir_all(&dir)?;
    let n = std::fs::read_dir(&dir)?.flatten().filter(|e| e.path().join("manifest.json").exists()).count();
    if open {
        #[cfg(windows)]
        { std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| Error::Invalid(format!("Could not open the folder: {e}")))?; }
    }
    Ok((dir.to_string_lossy().into_owned(), n))
}
