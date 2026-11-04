//! Plain text files the user picks (themes, extensions, collections): read and write by path.
use pocket_core::{Error, Result};

const MAX: u64 = 8 * 1024 * 1024;

#[tauri::command]
pub async fn text_file_read(path: String) -> Result<String> {
    let meta = std::fs::metadata(&path)?;
    if meta.len() > MAX { return Err(Error::Invalid("That file is too large.".into())); }
    std::fs::read_to_string(&path).map_err(|e| Error::Invalid(format!("Could not read {path}: {e}")))
}

#[tauri::command]
pub async fn text_file_save(path: String, text: String) -> Result<()> {
    if text.len() as u64 > MAX { return Err(Error::Invalid("That is too much to save.".into())); }
    std::fs::write(&path, text).map_err(|e| Error::Invalid(format!("Could not save {path}: {e}")))
}
