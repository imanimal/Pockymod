//! Text files the user picks (themes, extensions): the app opens the file dialog itself and touches only the file
//! chosen there. There is no command that reads or writes a path the page names.
use pocket_core::{Error, Result};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

const MAX: u64 = 8 * 1024 * 1024;

#[derive(Serialize)]
pub struct Picked { pub path: String, pub text: String }

fn exts(list: &[String]) -> Vec<String> {
    list.iter().filter(|e| !e.is_empty() && e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric())).cloned().collect()
}

/// Ask for a text file and read it. None when the user cancels.
#[tauri::command]
pub async fn text_file_open(app: tauri::AppHandle, title: String, extensions: Vec<String>) -> Result<Option<Picked>> {
    let ext = exts(&extensions);
    tauri::async_runtime::spawn_blocking(move || {
        let refs: Vec<&str> = ext.iter().map(String::as_str).collect();
        let mut d = app.dialog().file().set_title(&title);
        if !refs.is_empty() { d = d.add_filter(&title, &refs); }
        let Some(fp) = d.blocking_pick_file() else { return Ok(None) };
        let path = fp.into_path().map_err(|e| Error::Invalid(e.to_string()))?;
        if std::fs::metadata(&path)?.len() > MAX { return Err(Error::Invalid("That file is too large.".into())); }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::Invalid(format!("Could not read that file: {e}")))?;
        Ok(Some(Picked { path: path.to_string_lossy().into_owned(), text }))
    }).await.map_err(|e| Error::Invalid(e.to_string()))?
}

/// Ask where to save text and write it there. False when the user cancels.
#[tauri::command]
pub async fn text_file_save_as(app: tauri::AppHandle, default_name: String, text: String, extensions: Vec<String>) -> Result<bool> {
    if text.len() as u64 > MAX { return Err(Error::Invalid("That is too much to save.".into())); }
    let ext = exts(&extensions);
    let name: String = default_name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).take(120).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let refs: Vec<&str> = ext.iter().map(String::as_str).collect();
        let mut d = app.dialog().file().set_file_name(&name);
        if !refs.is_empty() { d = d.add_filter(&name, &refs); }
        let Some(fp) = d.blocking_save_file() else { return Ok(false) };
        let path = fp.into_path().map_err(|e| Error::Invalid(e.to_string()))?;
        std::fs::write(&path, text).map_err(|e| Error::Invalid(format!("Could not save it: {e}")))?;
        Ok(true)
    }).await.map_err(|e| Error::Invalid(e.to_string()))?
}
