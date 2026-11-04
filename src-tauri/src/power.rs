//! The main window turns off like a tube TV before it goes (after Collegiapp's power.rs). Closing it (the title
//! bar's close key, Alt+F4, the taskbar) emits "power-off" `{ seq }` to the page, which plays the picture collapsing
//! (ui/fx.ts powerOff) and calls `power_done(seq)`; then the window is destroyed and the app carries on in the tray
//! (background.rs), or the app quits when running in the background is off or Quit was chosen. If the page does not
//! answer in time it goes anyway. A hidden or minimised window just goes. A new version that is downloaded goes in
//! at this moment, when updates are automatic: the user is looking away.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Longer than the slowest power-off, short enough that a stuck page never keeps the window up.
const FALLBACK: Duration = Duration::from_millis(2600);

#[derive(Default)]
pub struct Power {
    seq: AtomicU64,
    /// the turn-off waiting for the page
    pending: Mutex<Option<u64>>,
}

#[derive(Serialize, Clone)]
struct Off {
    seq: u64,
}

fn quitting(app: &AppHandle) -> bool {
    app.state::<crate::background::Background>().quitting.load(Ordering::Relaxed) || !crate::background::enabled(app)
}

/// After the picture is off: quit, or close to the tray. Either way a downloaded update goes in now when updates
/// are automatic.
fn finish(app: &AppHandle) {
    let auto = app.state::<crate::state::AppState>().db.lock().unwrap().setting("app.updates").ok().flatten().unwrap_or_else(|| "auto".into()) == "auto";
    if quitting(app) {
        if !(auto && crate::background::install_now_with(app, false)) { app.exit(0); }
        return;
    }
    if let Some(w) = app.get_webview_window("main") { let _ = w.destroy(); }
    if auto { crate::background::install_now(app); }
}

/// Quit for real (the tray's Quit): the picture turns off first when the window is open.
pub fn quit(app: &AppHandle) {
    app.state::<crate::background::Background>().quitting.store(true, Ordering::Relaxed);
    match app.get_webview_window("main") {
        Some(w) => { let _ = w.close(); }
        None => finish(app),
    }
}

/// Turn the main window off: animate when it is on screen, else straight away.
pub fn off(app: &AppHandle) {
    let shown = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
    if !shown {
        return finish(app);
    }
    let p = app.state::<Power>();
    let seq = p.seq.fetch_add(1, Ordering::Relaxed) + 1;
    *p.pending.lock().unwrap() = Some(seq);
    if app.emit_to("main", "power-off", Off { seq }).is_err() {
        return done(app, seq);
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FALLBACK);
        done(&app, seq);
    });
}

/// The turn-off `seq` finished (or ran out of time): quit, once.
fn done(app: &AppHandle, seq: u64) {
    let p = app.state::<Power>();
    let take = {
        let mut pend = p.pending.lock().unwrap();
        if *pend == Some(seq) {
            *pend = None;
            true
        } else {
            false
        }
    };
    if take {
        finish(app);
    }
}

#[tauri::command]
pub fn power_done(app: AppHandle, seq: u64) {
    done(&app, seq);
}
