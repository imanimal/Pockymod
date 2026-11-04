//! The main window turns off like a tube TV before the app quits (after Collegiapp's power.rs). Closing it (the
//! title bar's close key, Alt+F4, the taskbar) emits "power-off" `{ seq }` to the page, which plays the picture
//! collapsing (ui/fx.ts powerOff) and calls `power_done(seq)`; then the app quits. If the page does not answer in
//! time it quits anyway. A hidden or minimised window just goes.

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

/// Turn the main window off: animate when it is on screen, else quit straight away.
pub fn off(app: &AppHandle) {
    let shown = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
    if !shown {
        return app.exit(0);
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
        app.exit(0);
    }
}

#[tauri::command]
pub fn power_done(app: AppHandle, seq: u64) {
    done(&app, seq);
}
