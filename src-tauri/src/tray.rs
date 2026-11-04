//! The tray icon, while the app runs (also with its window closed). It wears the theme's mark (icon.rs). A click
//! opens the window; its menu is short: Open, an update line only when one is ready, and Quit.
use crate::background::Background;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub const ID: &str = "main-tray";

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let b = app.state::<Background>();
    let ready = b.ready.lock().unwrap().as_ref().map(|(_, v)| v.clone());
    let mods = *b.mods_ready.lock().unwrap();
    let m = Menu::new(app)?;
    m.append(&MenuItem::with_id(app, "open", "Open Pockymod", true, None::<&str>)?)?;
    if let Some(v) = ready { m.append(&MenuItem::with_id(app, "app-update", format!("Restart into Pockymod {v}"), true, None::<&str>)?)?; }
    if mods > 0 { m.append(&MenuItem::with_id(app, "mod-updates", format!("Install {mods} mod {}", if mods == 1 { "update" } else { "updates" }), true, None::<&str>)?)?; }
    m.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;
    Ok(m)
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let icon = app.state::<crate::icon::Current>().0.lock().unwrap().clone()
        .or_else(|| tauri::image::Image::from_bytes(crate::icon::builtin("redline")).ok().map(|i| i.to_owned()));
    let mut b = TrayIconBuilder::with_id(ID).tooltip("Pockymod").menu(&menu(app)?).show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => show_main(app),
            "app-update" => { crate::background::install_now_with(app, true); }
            "mod-updates" => {
                let h = app.clone();
                tauri::async_runtime::spawn(async move {
                    let list = pocket_core::updates::last(&h.state::<crate::state::AppState>().db.lock().unwrap());
                    for u in list.into_iter().filter(|u| u.file_id > 0) {
                        let _ = crate::commands::library::queue_add(h.clone(), h.state(), u.mod_id, u.file_id, true).await;
                    }
                    *h.state::<Background>().mods_ready.lock().unwrap() = 0;
                    refresh(&h);
                });
            }
            "quit" => crate::power::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|t, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev { show_main(t.app_handle()); }
        });
    if let Some(i) = icon { b = b.icon(i); }
    b.build(app)?;
    Ok(())
}

/// Rebuild the menu after an update became ready or went in.
pub fn refresh(app: &AppHandle) {
    if let (Some(t), Ok(m)) = (app.tray_by_id(ID), menu(app)) { let _ = t.set_menu(Some(m)); }
}

/// The main window, made again if it was closed to the tray. It turns on as it opens (ui/fx.ts) and the page comes
/// back where it was.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
    if let Ok(b) = tauri::WebviewWindowBuilder::from_config(app, &cfg) {
        if let Ok(w) = b.build() {
            if let Some(i) = app.state::<crate::icon::Current>().0.lock().unwrap().clone() { let _ = w.set_icon(i); }
            let _ = w.set_focus();
        }
    }
}
