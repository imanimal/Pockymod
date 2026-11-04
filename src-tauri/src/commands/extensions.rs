//! Extensions: their on/off and approval state, the programs, launch options and links they list, opening pages in
//! the default browser, and the `pmext` protocol that serves a panel's page to a sandboxed frame.
//!
//! An extension is a JSON manifest in `addons/extension/<id>.json` (see docs/extensions.md). Its state (turned on,
//! which release the user approved, programs the user picked) is kept beside it in `addons/extension-state.json`.
//! Every command here re-reads the manifest from disk and refuses to act unless the extension is turned on and the
//! user approved the release that is installed, so a tool, launch option or link can only be one the user saw.
use crate::state::{AppState, Paths};
use pocket_core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, State};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtState {
    pub enabled: bool,
    /// The release the user approved. Another release does nothing until it is approved too.
    pub approved_release: Option<String>,
    /// What the user approved (permissions, tools, launch options, links), as the app lists them.
    pub approved: Vec<String>,
    /// Programs the user picked for the extension's tools, by tool id.
    pub tool_paths: BTreeMap<String, String>,
    /// Installed from the marketplace (so updates are offered).
    pub from_market: bool,
    pub installed_at: Option<i64>,
}

fn addons_dir(app: &AppHandle) -> Result<PathBuf> {
    let base = app.path().app_data_dir().map_err(|e| Error::Invalid(e.to_string()))?;
    let dir = base.join("addons");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn load_states(app: &AppHandle) -> Result<BTreeMap<String, ExtState>> {
    let f = addons_dir(app)?.join("extension-state.json");
    if !f.exists() { return Ok(BTreeMap::new()); }
    let text = std::fs::read_to_string(&f)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn save_states(app: &AppHandle, all: &BTreeMap<String, ExtState>) -> Result<()> {
    let dir = addons_dir(app)?;
    let tmp = dir.join("extension-state.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(all).map_err(|e| Error::Invalid(e.to_string()))?)?;
    std::fs::rename(&tmp, dir.join("extension-state.json"))?;
    Ok(())
}

/// Every extension's state, by id.
#[tauri::command]
pub async fn ext_states(app: AppHandle) -> Result<BTreeMap<String, ExtState>> { load_states(&app) }

/// Save one extension's state, or forget it (`state` left out) when the extension is removed.
#[tauri::command]
pub async fn ext_state_set(app: AppHandle, id: String, state: Option<ExtState>) -> Result<()> {
    super::addons::check_id(&id)?;
    let mut all = load_states(&app)?;
    match state {
        Some(s) => { all.insert(id, s); }
        None => { all.remove(&id); }
    }
    save_states(&app, &all)
}

/// The manifest and state of an extension that is turned on and approved for the release on disk.
fn approved(app: &AppHandle, id: &str) -> Result<(Value, ExtState)> {
    super::addons::check_id(id)?;
    let f = addons_dir(app)?.join("extension").join(format!("{id}.json"));
    let text = std::fs::read_to_string(&f).map_err(|_| Error::Invalid("That extension is not installed.".into()))?;
    let m: Value = serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("The extension file is broken: {e}")))?;
    if m["format"] != "pocket-mods.extension" { return Err(Error::Invalid("That is not a Pocket Mods extension.".into())); }
    let st = load_states(app)?.remove(id).unwrap_or_default();
    let release = m["release"].as_str().unwrap_or_default();
    if !st.enabled { return Err(Error::Invalid("That extension is turned off. Turn it on in Add-ons.".into())); }
    if st.approved_release.as_deref() != Some(release) {
        return Err(Error::Invalid("That extension changed since you approved it. Review it in Add-ons first.".into()));
    }
    Ok((m, st))
}

/// An entry of `contributes.<list>` by id.
fn find<'a>(m: &'a Value, list: &str, id: &str) -> Result<&'a Value> {
    m["contributes"][list].as_array().and_then(|a| a.iter().find(|x| x["id"].as_str() == Some(id)))
        .ok_or_else(|| Error::Invalid("The extension does not list that.".into()))
}

/// Arguments are passed straight to the program (no shell). Quotes, control characters and very long values are
/// refused so an argument means what it shows.
fn check_arg(a: &str) -> Result<()> {
    if a.is_empty() || a.len() > 260 || a.chars().any(|c| c.is_control() || c == '"') {
        return Err(Error::Invalid(format!("The extension has an argument the app will not pass: {a}")));
    }
    Ok(())
}

fn args_of(v: &Value, game: Option<&Path>) -> Result<Vec<String>> {
    let list = v["args"].as_array().cloned().unwrap_or_default();
    if list.len() > 24 { return Err(Error::Invalid("The extension lists too many arguments.".into())); }
    let mut out = Vec::new();
    for a in list {
        let a = a.as_str().ok_or_else(|| Error::Invalid("An argument is not text.".into()))?;
        check_arg(a)?;
        let a = if a.contains("{game}") {
            let g = game.ok_or_else(|| Error::Invalid("Set the Cyberpunk 2077 folder in Settings first.".into()))?;
            a.replace("{game}", &g.to_string_lossy())
        } else { a.to_string() };
        out.push(a);
    }
    Ok(out)
}

/// A path inside the game folder: relative, no `..`, no drive or root.
fn inside(game: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if rel.is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::Invalid("A tool's path must be inside the game folder, like tools/x/x.exe.".into()));
    }
    Ok(game.join(p))
}

fn tool_program(tool: &Value, st: &ExtState, tool_id: &str, game: Option<&Path>) -> Result<PathBuf> {
    let exe = if let Some(rel) = tool["path"].as_str() {
        let g = game.ok_or_else(|| Error::Invalid("Set the Cyberpunk 2077 folder in Settings first.".into()))?;
        inside(g, rel)?
    } else if tool.get("pick").is_some() {
        let p = st.tool_paths.get(tool_id).ok_or_else(|| Error::Invalid("Choose where this program is first (Add-ons, Installed).".into()))?;
        let p = PathBuf::from(p);
        if !p.is_absolute() { return Err(Error::Invalid("The program's path is not a full path.".into())); }
        if let Some(want) = tool["pick"]["file"].as_str() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if !name.eq_ignore_ascii_case(want) { return Err(Error::Invalid(format!("This tool starts {want}; choose that file."))); }
        }
        p
    } else {
        return Err(Error::Invalid("The tool has no program.".into()));
    };
    if !exe.is_file() { return Err(Error::Invalid(format!("{} was not found.", exe.display()))); }
    if cfg!(windows) && !exe.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
        return Err(Error::Invalid("Only .exe programs can be started.".into()));
    }
    Ok(exe)
}

fn spawn(exe: &Path, args: &[String]) -> Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    if let Some(dir) = exe.parent() { cmd.current_dir(dir); }
    cmd.spawn().map_err(|e| Error::Invalid(format!("Could not start {}: {e}", exe.display())))?;
    Ok(())
}

/// Start a program an extension lists (`contributes.tools`), if the extension is on and approved.
#[tauri::command]
pub async fn ext_tool_start(app: AppHandle, state: State<'_, AppState>, ext_id: String, tool_id: String) -> Result<()> {
    let (m, st) = approved(&app, &ext_id)?;
    let tool = find(&m, "tools", &tool_id)?;
    let game = Paths::load(&state.db.lock().unwrap()).game_dir;
    let exe = tool_program(tool, &st, &tool_id, game.as_deref())?;
    let args = args_of(tool, game.as_deref())?;
    spawn(&exe, &args)
}

/// Start the game with an extension's launch option, after the REDmod deploy when REDmods changed (as the Play key).
#[tauri::command]
pub async fn ext_launch(app: AppHandle, state: State<'_, AppState>, ext_id: String, option_id: String) -> Result<Value> {
    use pocket_core::cyberpunk::redmod;
    use pocket_core::profiles;
    let (m, _) = approved(&app, &ext_id)?;
    let opt = find(&m, "launchOptions", &option_id)?;
    let args = args_of(opt, None)?;
    let (game, _) = Paths::load(&state.db.lock().unwrap()).require()?;
    let (redmods, needs_deploy) = { let d = state.db.lock().unwrap(); (pocket_core::launch::has_redmods(&d)?, profiles::redmod_needs_deploy(&d)?) };
    let mut deployed = None;
    if needs_deploy {
        let order = profiles::redmod_order(&state.db.lock().unwrap())?;
        let mods: Vec<String> = order.into_iter().filter(|e| e.enabled).flat_map(|e| e.folders).collect();
        let g = game.clone();
        let r = tauri::async_runtime::spawn_blocking(move || redmod::run_deploy(&g, &mods)).await.map_err(|e| Error::Invalid(e.to_string()))??;
        if !r.ok { return Ok(serde_json::json!({ "started": false, "redmod": r })); }
        state.db.lock().unwrap().set_setting("redmod.lastDeploy", &pocket_core::db::now().to_string())?;
        deployed = Some(r);
    }
    let exe = game.join(pocket_core::cyberpunk::game::EXE);
    if !exe.exists() { return Err(Error::Invalid("Cyberpunk2077.exe was not found. Check the game folder in Settings.".into())); }
    let mut all = Vec::new();
    if redmods && !args.iter().any(|a| a == "-modded") { all.push("-modded".to_string()); }
    all.extend(args);
    spawn(&exe, &all)?;
    Ok(serde_json::json!({ "started": true, "modded": redmods, "redmod": deployed, "args": all }))
}

/// Open a page an extension lists in the default browser, if the extension is on and approved: a link
/// (`contributes.links`) by id, or a tool's download page as `tool:<id>`.
#[tauri::command]
pub async fn ext_open_link(app: AppHandle, ext_id: String, link_id: String) -> Result<()> {
    let (m, _) = approved(&app, &ext_id)?;
    let raw = match link_id.strip_prefix("tool:") {
        Some(tool) => find(&m, "tools", tool)?["download"].as_str().map(str::to_owned),
        None => find(&m, "links", &link_id)?["url"].as_str().map(str::to_owned),
    };
    let url = url::Url::parse(&raw.unwrap_or_default()).map_err(|_| Error::Invalid("That link is not a web address.".into()))?;
    if url.scheme() != "https" { return Err(Error::Invalid("Only https links open.".into())); }
    open_in_browser(url.as_str())
}

/// Pages the app itself opens in the default browser: this project on GitHub (the marketplace, docs, submissions)
/// and Nexus Mods.
pub const BROWSER_PREFIXES: [&str; 2] = ["https://github.com/imanimal/pocket-mods/", "https://www.nexusmods.com/"];

pub fn browser_url_allowed(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let s = u.as_str().to_string();
    (u.scheme() == "https" && u.username().is_empty() && u.password().is_none() && BROWSER_PREFIXES.iter().any(|p| s.starts_with(p))).then_some(s)
}

/// Open a page in the default browser. Only this project's GitHub pages and Nexus Mods pages.
#[tauri::command]
pub async fn open_external(url: String) -> Result<()> {
    let ok = browser_url_allowed(&url).ok_or_else(|| Error::Invalid("Only this project's GitHub pages and Nexus Mods pages open from here.".into()))?;
    open_in_browser(&ok)
}

/// Hand an https URL to the system. On Windows this is the shell's URL handler (no shell command line is built);
/// elsewhere `open` or `xdg-open`.
fn open_in_browser(url: &str) -> Result<()> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"') {
        return Err(Error::Invalid("That is not a web address the app opens.".into()));
    }
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).creation_flags(CREATE_NO_WINDOW).spawn()
    };
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(not(any(windows, unix)))]
    let r: std::io::Result<()> = Err(std::io::Error::other("no browser"));
    r.map(|_| ()).map_err(|e| Error::Invalid(format!("Could not open the browser: {e}")))
}

// ── panels ──

/// The page around a panel: a strict CSP, the theme, and the `pocket` object that talks to the app by postMessage.
/// The same file is used by the browser demo (src/screens/addons/Panel.tsx), so it is kept in one place.
pub const PANEL_SHELL: &str = include_str!("panel-shell.html");

/// No network at all (fetch, images, fonts, frames, forms); inline script and style only. `sandbox` keeps the page in
/// an opaque origin even if it is opened outside its frame.
pub const PANEL_CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src data:; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'";

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) { out.push(h * 16 + l); i += 3; continue; }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The full page for `<extension id>/<panel id>`, if the extension is on and approved.
pub fn panel_page(app: &AppHandle, path: &str) -> Result<String> {
    let path = decode(path);
    let mut it = path.trim_start_matches('/').splitn(2, '/');
    let (ext, panel) = (it.next().unwrap_or_default(), it.next().unwrap_or_default());
    let (m, _) = approved(app, ext)?;
    let p = find(&m, "panels", panel)?;
    let html = p["html"].as_str().unwrap_or_default();
    Ok(PANEL_SHELL.replacen("<!--PANEL-->", html, 1))
}

/// The `pmext` protocol: a panel's page with its own CSP.
pub fn panel_protocol(app: &AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    // a panel loads only as the Add-ons screen's frame; opened as a page of its own (a navigation in some window)
    // it would get that window's access to the app
    let dest = request.headers().get("sec-fetch-dest").and_then(|v| v.to_str().ok()).unwrap_or("iframe").to_ascii_lowercase();
    if dest != "iframe" && dest != "frame" && dest != "empty" {
        return Response::builder().status(StatusCode::FORBIDDEN).header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(b"A panel opens only inside Pocket Mods' Add-ons screen.".to_vec()).unwrap_or_else(|_| Response::new(Vec::new()));
    }
    let page = panel_page(app, request.uri().path());
    let (status, kind, body) = match page {
        Ok(html) => (StatusCode::OK, "text/html; charset=utf-8", html.into_bytes()),
        Err(e) => (StatusCode::NOT_FOUND, "text/plain; charset=utf-8", e.to_string().into_bytes()),
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, kind)
        .header("Content-Security-Policy", PANEL_CSP)
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .header(header::CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_prefixes() {
        assert!(browser_url_allowed("https://github.com/imanimal/pocket-mods/issues/new?template=marketplace-submission.yml&title=a b").is_some());
        assert!(browser_url_allowed("https://www.nexusmods.com/cyberpunk2077/mods/107").is_some());
        for bad in ["http://www.nexusmods.com/x", "https://github.com/imanimal/other", "https://github.com/imanimal/pocket-mods/../x/",
            "https://www.nexusmods.com.evil.example/", "https://user@www.nexusmods.com/", "file:///c:/x", "javascript:alert(1)"] {
            assert!(browser_url_allowed(bad).is_none(), "{bad}");
        }
        // spaces and quotes come back percent-encoded, so nothing can split the command line
        let s = browser_url_allowed("https://github.com/imanimal/pocket-mods/issues/new?title=\"a b\"").unwrap();
        assert!(!s.contains(' ') && !s.contains('"'));
    }

    #[test]
    fn paths_stay_in_the_game() {
        let g = Path::new("/games/cp");
        assert!(inside(g, "tools/redmod/bin/redMod.exe").is_ok());
        for bad in ["", "../x.exe", "/abs/x.exe", "tools/../../x.exe", "./x.exe"] { assert!(inside(g, bad).is_err(), "{bad}"); }
    }

    #[test]
    fn args() {
        let v = serde_json::json!({ "args": ["-skipStartScreen", "--path={game}/x"] });
        assert_eq!(args_of(&v, Some(Path::new("/g"))).unwrap(), vec!["-skipStartScreen", "--path=/g/x"]);
        assert!(args_of(&serde_json::json!({ "args": ["a\"b"] }), None).is_err());
        assert!(args_of(&serde_json::json!({ "args": ["a\nb"] }), None).is_err());
        assert!(args_of(&serde_json::json!({ "args": ["{game}"] }), None).is_err());
    }

    #[test]
    fn decoding() {
        assert_eq!(decode("/my-ext%2Fpanel"), "/my-ext/panel");
        assert_eq!(decode("/a/b"), "/a/b");
        assert_eq!(decode("/a%2"), "/a%2");
    }

    #[test]
    fn shell_has_a_slot_and_a_csp() {
        assert!(PANEL_SHELL.contains("<!--PANEL-->"));
        assert!(PANEL_SHELL.contains("Content-Security-Policy"));
        assert!(PANEL_CSP.contains("default-src 'none'"));
    }
}
