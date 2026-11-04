//! The app's icon follows the theme: the taskbar and the tray show the Redline, Standard or Pocket mark, or the
//! icon an installed theme brings (a PNG of at most 512 by 512 and 256 KB). The installer and the .exe keep the
//! Redline mark (icons/), which is the default theme's.
use tauri::image::Image;
use tauri::{AppHandle, Manager};

const REDLINE: &[u8] = include_bytes!("../icons/themes/redline.png");
const STANDARD: &[u8] = include_bytes!("../icons/themes/standard.png");
const POCKET: &[u8] = include_bytes!("../icons/themes/pocket.png");

/// The built-in mark for a look (an installed theme without an icon uses its base look's).
pub fn builtin(look: &str) -> &'static [u8] {
    match look { "standard" => STANDARD, "pocket" => POCKET, _ => REDLINE }
}

/// A theme's own icon, checked: a PNG, small, square-ish and not huge.
pub fn checked(png: &[u8]) -> pocket_core::Result<Image<'static>> {
    let bad = |m: &str| pocket_core::Error::Invalid(format!("That theme icon cannot be used: {m}."));
    if png.len() > 256 * 1024 { return Err(bad("it is over 256 KB")); }
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") { return Err(bad("it is not a PNG")); }
    let img = Image::from_bytes(png).map_err(|e| bad(&e.to_string()))?;
    if img.width() > 512 || img.height() > 512 || img.width() < 16 || img.height() < 16 { return Err(bad("it must be 16 to 512 pixels on a side")); }
    Ok(img.to_owned())
}

/// Show `look`'s mark, or `png` when the theme has its own, on the main window (the taskbar) and the tray.
pub fn apply(app: &AppHandle, look: &str, png: Option<&[u8]>) -> pocket_core::Result<()> {
    let img = match png { Some(p) => checked(p)?, None => Image::from_bytes(builtin(look)).map_err(|e| pocket_core::Error::Invalid(e.to_string()))?.to_owned() };
    if let Some(w) = app.get_webview_window("main") { let _ = w.set_icon(img.clone()); }
    if let Some(t) = app.tray_by_id(crate::tray::ID) { let _ = t.set_icon(Some(img.clone())); }
    *app.state::<Current>().0.lock().unwrap() = Some(img);
    Ok(())
}

/// The icon showing now, for a window or tray made later.
#[derive(Default)]
pub struct Current(pub std::sync::Mutex<Option<Image<'static>>>);

#[tauri::command]
pub async fn app_icon(app: AppHandle, look: String, png: Option<Vec<u8>>) -> pocket_core::Result<()> { apply(&app, &look, png.as_deref()) }

#[cfg(test)]
mod tests {
    #[test]
    fn the_built_in_marks_decode_and_bad_icons_are_refused() {
        for l in ["redline", "standard", "pocket"] { assert!(super::checked(super::builtin(l)).is_ok(), "{l}"); }
        assert!(super::checked(b"GIF89a").is_err());
        assert!(super::checked(&vec![0u8; 300 * 1024]).is_err());
    }
}
