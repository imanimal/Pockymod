//! Nexus pages: where they open, and the browser pane.
//!
//! By default a Nexus page opens in the user's own browser: their sign-in, saved passwords and extensions are there,
//! and "Mod Manager Download" comes back to the app through the nxm:// link handler (lib.rs, single instance). In
//! Settings it can open in the app instead, in the browser pane: a webview docked in the main window beside the app
//! (the page lays it out, src/ui/BrowserPane.tsx, and sends its bounds), over the whole view, hidden behind a tab,
//! or popped out as a window of its own. Pane and window share a profile folder of their own (app data/browser), so
//! a sign-in there is kept between runs and carries over when the pane pops out or back in. With unpacked Chrome
//! extensions in app data/browser-extensions, the pane loads them (WebView2 only, read when the pane first opens).
//!
//! The pane and the window never open the app's own pages (they would get the app's access), their nxm:// links never
//! reach the OS (the queue takes them), and a "Manual download" lands in the library's downloads folder.

use crate::state::{AppState, Paths};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

/// The docked pane in the main window, and the window it pops out to.
pub const PANE: &str = "nexus-pane";
pub const WINDOW: &str = "nexus";

/// Where the page last asked the pane to be, so a pane made again opens there.
/// And whether the pane was last seen signed in to Nexus, so the app hears only of a change.
#[derive(Default)]
pub struct Browser { bounds: Mutex<Option<[f64; 4]>>, signed: Mutex<Option<bool>> }

/// Pages open in the user's own browser unless Settings says the pane ("nexus.pages" = "pane").
pub fn in_app(app: &AppHandle) -> bool {
    app.state::<AppState>().db.lock().unwrap().setting("nexus.pages").ok().flatten().as_deref() == Some("pane")
}

/// The pane's and window's profile folder, and the extensions folder.
pub fn profile_dir(app: &AppHandle) -> Option<PathBuf> { app.path().app_data_dir().ok().map(|d| d.join("browser")) }
pub fn extensions_dir(app: &AppHandle) -> Option<PathBuf> { app.path().app_data_dir().ok().map(|d| d.join("browser-extensions")) }

/// The pane and the window are for the web (Nexus, its sign-in providers, sites mods link to). They never open the
/// app's own pages (the app, extension panels, files): those would get the pane's access to the app.
pub fn may_open(u: &Url) -> bool {
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let local = host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" || host == "[::1]" || host == "0.0.0.0";
    (matches!(u.scheme(), "https" | "http") && !local) || u.as_str() == "about:blank"
}

/// Pages that ask for a stylesheet but got none (a first load can come up unstyled, with no scripts either) load
/// again, once per address. Nothing else on the page is touched.
const STYLE_GUARD: &str = r#"(function () {
  if (location.protocol !== "https:") return;
  addEventListener("load", function () {
    setTimeout(function () {
      var links = [].slice.call(document.querySelectorAll('link[rel~="stylesheet"]'));
      if (!links.length || links.some(function (l) { return !!l.sheet; })) return;
      var k = "pm-restyled:" + location.pathname;
      try { if (sessionStorage.getItem(k)) return; sessionStorage.setItem(k, "1"); } catch (e) { return; }
      location.reload();
    }, 1500);
  });
})();"#;

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserState {
    pub open: bool, pub popped: bool, pub url: Option<String>, pub title: Option<String>, pub loading: bool,
    /// The app opened this page (a page waiting for a click, a link the user pressed): a hidden pane shows itself.
    #[serde(default)]
    pub reveal: bool,
}
impl BrowserState {
    fn closed() -> Self { BrowserState { open: false, popped: false, url: None, title: None, loading: false, reveal: false } }
    fn at(url: &Url, popped: bool, loading: bool, reveal: bool) -> Self { BrowserState { open: true, popped, url: Some(url.to_string()), title: None, loading, reveal } }
}

fn say(app: &AppHandle, s: BrowserState) { let _ = app.emit("browser", s); }

pub fn state(app: &AppHandle) -> BrowserState {
    if let Some(w) = app.get_webview_window(WINDOW) {
        return BrowserState { open: true, popped: true, url: w.url().ok().map(|u| u.to_string()), title: w.title().ok(), loading: false, reveal: false };
    }
    match app.get_webview(PANE) {
        Some(p) => BrowserState { open: true, popped: false, url: p.url().ok().map(|u| u.to_string()), title: None, loading: false, reveal: false },
        None => BrowserState::closed(),
    }
}

/// Open a Nexus page where the user wants pages: the popped-out window when it is open, the pane, or their browser.
pub fn open(app: &AppHandle, url: &str) -> pocket_core::Result<()> {
    let bad = || pocket_core::Error::Invalid("The app doesn't open that page".into());
    let u: Url = url.parse().map_err(|_| bad())?;
    if !may_open(&u) || u.scheme() != "https" { return Err(bad()); }
    if let Some(w) = app.get_webview_window(WINDOW) {
        w.navigate(u).map_err(err)?;
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    if in_app(app) { return open_pane(app, u); }
    crate::commands::extensions::open_in_browser(u.as_str())
}

fn err(e: tauri::Error) -> pocket_core::Error { pocket_core::Error::Invalid(e.to_string()) }

/// The profile and extensions. Every webview on the profile needs the same ones, or WebView2 refuses to make it.
macro_rules! profile {
    ($b:expr, $app:expr) => {{
        let app: &AppHandle = $app;
        let mut b = $b;
        if let Some(d) = profile_dir(app) { let _ = std::fs::create_dir_all(&d); b = b.data_directory(d); }
        // extensions: any unpacked Chrome extension folder in app data/browser-extensions (Windows, WebView2)
        if let Some(x) = extensions_dir(app).filter(|x| std::fs::read_dir(x).map(|mut r| r.any(|e| e.is_ok_and(|e| e.path().join("manifest.json").exists()))).unwrap_or(false)) {
            b = b.browser_extensions_enabled(true).extensions_path(x);
        }
        b
    }};
}

/// Settings every pane and window shares: the profile, extensions, downloads into the library, nxm:// links to the
/// queue, no app pages, and the page state for the pane's bar.
macro_rules! common {
    ($b:expr, $app:expr) => {{
        let app: &AppHandle = $app;
        let b = profile!($b, app);
        let nav = app.clone();
        let dl = app.clone();
        let load = app.clone();
        b.initialization_script(STYLE_GUARD)
            .on_download(move |_, ev| {
                match ev {
                    // "Manual download" on a Nexus page: into the library's downloads folder, and the app offers Install
                    tauri::webview::DownloadEvent::Requested { destination, .. } => {
                        let paths = { let st = dl.state::<AppState>(); let p = Paths::load(&st.db.lock().unwrap()); p };
                        if let Ok((_, lib)) = paths.require() {
                            let name = destination.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "download.zip".into());
                            let dir = lib.join("downloads");
                            let _ = std::fs::create_dir_all(&dir);
                            *destination = dir.join(name);
                        }
                    }
                    tauri::webview::DownloadEvent::Finished { path, success, .. } => {
                        let _ = dl.emit("manual-download", serde_json::json!({ "path": path.map(|p| p.to_string_lossy().into_owned()), "success": success }));
                    }
                    _ => {}
                }
                true
            })
            .on_navigation(move |u| {
                if u.scheme() == "nxm" {
                    let s = u.to_string();
                    let h = nav.clone();
                    tauri::async_runtime::spawn(async move { crate::runner::handle_nxm(&h, &s).await });
                    return false;
                }
                may_open(u)
            })
            // sign-in popups (window.open) are WebView2's own; never one for the app's pages
            .on_new_window(|u, _| if may_open(&u) { tauri::webview::NewWindowResponse::Allow } else { tauri::webview::NewWindowResponse::Deny })
            .on_page_load(move |w, p| {
                let loading = !matches!(p.event(), tauri::webview::PageLoadEvent::Finished);
                let popped = w.label() == WINDOW;
                say(&load, BrowserState::at(p.url(), popped, loading, false));
                // signing in or out on the page itself: read again once it has loaded (off this thread: reading
                // cookies from a webview's own event handler deadlocks)
                if !loading { let a = load.clone(); tauri::async_runtime::spawn(async move { watch_sign_in(&a) }); }
            })
    }};
}

/// Show `url` in the pane, making the pane when there is none. The page lays the pane out when it hears "browser".
fn open_pane(app: &AppHandle, url: Url) -> pocket_core::Result<()> {
    if let Some(p) = app.get_webview(PANE) {
        p.navigate(url.clone()).map_err(err)?;
        say(app, BrowserState::at(&url, false, true, true));
        return Ok(());
    }
    let win = app.get_window("main").ok_or_else(|| pocket_core::Error::Invalid("The main window is gone".into()))?;
    // where the page last had it, else the right half until the page says where
    let [x, y, w, h] = app.state::<Browser>().bounds.lock().unwrap().unwrap_or_else(|| {
        let s = win.inner_size().ok().and_then(|s| win.scale_factor().ok().map(|f| s.to_logical::<f64>(f))).unwrap_or(LogicalSize::new(1360.0, 860.0));
        [s.width / 2.0, 48.0, s.width / 2.0, (s.height - 48.0).max(200.0)]
    });
    let title = app.clone();
    let b = common!(tauri::webview::WebviewBuilder::new(PANE, WebviewUrl::External("about:blank".parse().unwrap())), app)
        .on_document_title_changed(move |w, t| { let _ = title.emit("browser-title", serde_json::json!({ "label": w.label(), "title": t })); })
        .background_color(tauri::window::Color(0x1d, 0x1d, 0x21, 0xff));
    let pane = win.add_child(b, LogicalPosition::new(x, y), LogicalSize::new(w.max(1.0), h.max(1.0))).map_err(err)?;
    save_passwords(&pane);
    // made blank, then sent to the page: a page given to a new webview can come up without its stylesheets
    pane.navigate(url.clone()).map_err(err)?;
    say(app, BrowserState::at(&url, false, true, true));
    Ok(())
}

/// The pane's place in the main window, in CSS pixels, and whether it shows (hidden behind a tab, or under one of
/// the app's own windows).
pub fn set_bounds(app: &AppHandle, x: f64, y: f64, w: f64, h: f64, visible: bool) -> pocket_core::Result<()> {
    *app.state::<Browser>().bounds.lock().unwrap() = Some([x, y, w, h]);
    let Some(p) = app.get_webview(PANE) else { return Ok(()) };
    if visible && w >= 1.0 && h >= 1.0 {
        p.set_bounds(tauri::Rect { position: LogicalPosition::new(x, y).into(), size: LogicalSize::new(w, h).into() }).map_err(err)?;
        p.show().map_err(err)?;
    } else {
        p.hide().map_err(err)?;
    }
    Ok(())
}

/// Back, forward, reload, or stop the pane or window.
pub fn nav(app: &AppHandle, action: &str) -> pocket_core::Result<()> {
    let js = match action { "back" => "history.back()", "forward" => "history.forward()", "stop" => "window.stop()", "reload" => "", _ => return Err(pocket_core::Error::Invalid("Unknown browser action".into())) };
    if let Some(w) = app.get_webview_window(WINDOW) { return if js.is_empty() { w.reload() } else { w.eval(js) }.map_err(err); }
    if let Some(p) = app.get_webview(PANE) { return if js.is_empty() { p.reload() } else { p.eval(js) }.map_err(err); }
    Ok(())
}

/// The page the pane or window shows now.
pub fn current(app: &AppHandle) -> Option<Url> {
    app.get_webview_window(WINDOW).and_then(|w| w.url().ok()).or_else(|| app.get_webview(PANE).and_then(|p| p.url().ok())).filter(|u| u.scheme() == "https")
}

pub fn close(app: &AppHandle) -> pocket_core::Result<()> {
    if let Some(p) = app.get_webview(PANE) { p.close().map_err(err)?; }
    if let Some(w) = app.get_webview_window(WINDOW) { w.close().map_err(err)?; }
    say(app, BrowserState::closed());
    Ok(())
}

/// Pop the pane out into a window of its own (same profile, so still signed in), or back into the main window.
pub fn pop(app: &AppHandle, out: bool) -> pocket_core::Result<()> {
    let url = current(app).unwrap_or_else(|| "https://www.nexusmods.com/cyberpunk2077".parse().unwrap());
    if out {
        if let Some(p) = app.get_webview(PANE) { p.close().map_err(err)?; }
        open_window(app, url)
    } else {
        if let Some(w) = app.get_webview_window(WINDOW) { w.close().map_err(err)?; }
        open_pane(app, url)
    }
}

/// The page in a window of its own. Commands that open a window are async: on Windows a window made from a
/// synchronous command never finishes loading (WebView2 needs the main thread the command would be blocking).
fn open_window(app: &AppHandle, url: Url) -> pocket_core::Result<()> {
    let closed = app.clone();
    let w = common!(tauri::WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External("about:blank".parse().unwrap())), app)
        .title("Nexus Mods · Pockymod")
        .inner_size(1180.0, 820.0)
        .min_inner_size(720.0, 520.0)
        // Nexus's own dark grey while the page loads, not a white flash
        .background_color(tauri::window::Color(0x1d, 0x1d, 0x21, 0xff))
        .on_document_title_changed(|w, t| { let _ = w.set_title(&format!("{t} · Pockymod")); })
        .build().map_err(err)?;
    if let Some(v) = app.get_webview(WINDOW) { save_passwords(&v); }
    // the window closed by its own close key: the pane's tab goes too (unless it went back into the main window)
    w.on_window_event(move |e| { if let tauri::WindowEvent::Destroyed = e { if closed.get_webview(PANE).is_none() { say(&closed, BrowserState::closed()); } } });
    w.navigate(url.clone()).map_err(err)?;
    say(app, BrowserState::at(&url, true, true, true));
    Ok(())
}

/// The pane offers to save a password typed into a page and fills it in next time (WebView2 has this off unless
/// asked, and asks the user before saving each one). Whoever's browser can't hand over its Nexus sign-in then types
/// the password once.
fn save_passwords(v: &tauri::Webview) {
    #[cfg(windows)]
    let _ = v.with_webview(|w| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
        use windows_core::Interface;
        if let Ok(s) = w.controller().CoreWebView2().and_then(|c| c.Settings()).and_then(|s| s.cast::<ICoreWebView2Settings4>()) {
            let _ = s.SetIsPasswordAutosaveEnabled(true);
            let _ = s.SetIsGeneralAutofillEnabled(true);
        }
    });
    #[cfg(not(windows))]
    let _ = v;
}

// ── the pane's Nexus sign-in ──

/// The cookies that make up a sign-in on nexusmods.com. Nexus hands every visitor a `nexusmods_session`, signed in or
/// not; only a sign-in also gets the refresh cookie.
const SIGN_IN_COOKIES: [&str; 2] = ["nexusmods_session", "nexusmods_session_refresh"];

/// Whether the pane is signed in to Nexus.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignIn { pub signed_in: bool }

/// A webview on the pane's profile: the pane, its window, or a hidden one made for the call and closed after.
struct OnProfile { view: tauri::Webview, temp: Option<tauri::WebviewWindow> }
impl OnProfile {
    fn get(app: &AppHandle) -> pocket_core::Result<Self> {
        if let Some(view) = app.get_webview(WINDOW).or_else(|| app.get_webview(PANE)) { return Ok(OnProfile { view, temp: None }); }
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let label = format!("nexus-profile-{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let w = profile!(tauri::WebviewWindowBuilder::new(app, &label, WebviewUrl::External("about:blank".parse().unwrap())), app)
            .visible(false).skip_taskbar(true).build().map_err(err)?;
        let view = app.get_webview(&label).ok_or_else(|| pocket_core::Error::Invalid("The app's browser didn't start".into()))?;
        Ok(OnProfile { view, temp: Some(w) })
    }
    fn done(self) { if let Some(w) = self.temp { let _ = w.destroy(); } }
}

fn signed_in(v: &tauri::Webview) -> bool {
    let now = tauri::webview::cookie::time::OffsetDateTime::now_utc();
    v.cookies_for_url("https://www.nexusmods.com/".parse().unwrap()).is_ok_and(|c| c.iter().any(|c|
        c.name() == SIGN_IN_COOKIES[1] && c.expires_datetime().is_none_or(|t| t > now)))
}

/// Remove the profile's Nexus sign-in, under whatever domain each cookie was saved. This goes to WebView2's cookie
/// manager itself: tauri's cookie calls drop the leading dot of `.nexusmods.com`, so they miss the real cookies.
#[cfg(windows)]
async fn forget_sign_in(v: &tauri::Webview) -> pocket_core::Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    v.with_webview(move |w| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows_core::{Interface, HSTRING};
        let r = (|| unsafe {
            let m = w.controller().CoreWebView2()?.cast::<ICoreWebView2_2>()?.CookieManager()?;
            for name in SIGN_IN_COOKIES {
                for uri in ["https://nexusmods.com/", "https://www.nexusmods.com/", "https://users.nexusmods.com/"] {
                    m.DeleteCookies(&HSTRING::from(name), &HSTRING::from(uri))?;
                }
            }
            windows_core::Result::Ok(())
        })();
        let _ = tx.send(r.map_err(|e| e.message()));
    }).map_err(err)?;
    rx.await.map_err(|_| pocket_core::Error::Invalid("The app's browser didn't answer".into()))?.map_err(pocket_core::Error::Invalid)
}

#[cfg(not(windows))]
async fn forget_sign_in(v: &tauri::Webview) -> pocket_core::Result<()> {
    for c in v.cookies().map_err(err)?.into_iter().filter(|c| SIGN_IN_COOKIES.contains(&c.name())) { let _ = v.delete_cookie(c); }
    Ok(())
}

pub async fn sign_in_state(app: &AppHandle) -> pocket_core::Result<SignIn> {
    let p = OnProfile::get(app)?;
    let signed_in = signed_in(&p.view);
    p.done();
    tell_sign_in(app, signed_in);
    Ok(SignIn { signed_in })
}

/// After a page loads in the pane or its window: whether the user signed in or out there.
fn watch_sign_in(app: &AppHandle) {
    let Some(v) = app.get_webview(WINDOW).or_else(|| app.get_webview(PANE)) else { return };
    tell_sign_in(app, signed_in(&v));
}

/// Say "browser-signin" when the pane's sign-in changed since last seen (Settings follows it).
fn tell_sign_in(app: &AppHandle, now: bool) {
    let b = app.state::<Browser>();
    let changed = { let mut s = b.signed.lock().unwrap(); let c = *s != Some(now); *s = Some(now); c };
    if changed { let _ = app.emit("browser-signin", SignIn { signed_in: now }); }
}

/// Sign the pane out of Nexus and forget the passwords it saved. Everything else stays, Cloudflare's clearance
/// included, so the next Nexus page doesn't stop to check for a bot.
pub async fn sign_out(app: &AppHandle) -> pocket_core::Result<SignIn> {
    let p = OnProfile::get(app)?;
    let out = forget_sign_in(&p.view).await;
    forget_passwords(&p.view);
    // a hidden webview stays a moment so the clearing it started finishes
    if p.temp.is_some() { tokio::time::sleep(std::time::Duration::from_secs(2)).await; }
    let signed_in = signed_in(&p.view);
    p.done();
    out?;
    tell_sign_in(app, signed_in);
    show_signed_change(app);
    Ok(SignIn { signed_in })
}

/// The passwords the pane offered to save (save_passwords). Cleared in the background.
fn forget_passwords(v: &tauri::Webview) {
    #[cfg(windows)]
    let _ = v.with_webview(|w| unsafe {
        use webview2_com::{ClearBrowsingDataCompletedHandler, Microsoft::Web::WebView2::Win32::*};
        use windows_core::Interface;
        if let Ok(p) = w.controller().CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_13>()).and_then(|c| c.Profile()).and_then(|p| p.cast::<ICoreWebView2Profile2>()) {
            let _ = p.ClearBrowsingData(COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE, &ClearBrowsingDataCompletedHandler::create(Box::new(|_| Ok(()))));
        }
    });
    #[cfg(not(windows))]
    let _ = v.clear_all_browsing_data();
}

/// The open pane loads its page again, now signed out.
fn show_signed_change(app: &AppHandle) { if current(app).is_some() { let _ = nav(app, "reload"); } }

#[cfg(test)]
mod tests {
    #[test]
    fn never_opens_app_pages() {
        let ok = |s: &str| super::may_open(&s.parse().unwrap());
        assert!(ok("https://www.nexusmods.com/cyberpunk2077/mods/107") && ok("https://users.nexusmods.com/auth") && ok("https://accounts.google.com/x") && ok("about:blank"));
        for bad in ["http://pmext.localhost/evil/panel", "pmext://localhost/evil/panel", "tauri://localhost/", "http://tauri.localhost/", "file:///c:/x", "http://127.0.0.1:1420/", "ipc://localhost/x", "about:srcdoc"] {
            assert!(!ok(bad), "{bad}");
        }
    }
}
