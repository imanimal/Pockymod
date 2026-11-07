//! The Nexus sign-in from the user's Firefox, for the app's browser pane (src-tauri/src/browser.rs).
//!
//! Firefox keeps cookies unencrypted in each profile's cookies.sqlite. Only the two Nexus sign-in cookies are read;
//! nothing else in Firefox is looked at, and the values never leave this process except into the pane. Chrome and
//! Edge lock their cookies to their own program (app-bound encryption), so they are not read at all.

use crate::credentials::Secret;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// The cookies that make up a sign-in on nexusmods.com.
pub const SIGN_IN_COOKIES: [&str; 2] = ["nexusmods_session", "nexusmods_session_refresh"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite { None, Lax, Strict }

#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: Secret,
    pub host: String,
    pub path: String,
    /// Unix seconds.
    pub expires: i64,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
}

/// Firefox's folder in the user's roaming app data.
pub fn root() -> Option<PathBuf> { std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Mozilla").join("Firefox")) }

/// The Nexus sign-in from the first Firefox profile that has one that hasn't expired, or None.
pub fn nexus_sign_in() -> Result<Option<Vec<Cookie>>> {
    match root() { Some(r) => sign_in_in(&r, chrono::Utc::now().timestamp()), None => Ok(None) }
}

/// Profile folders, the one Firefox opens by default first. `profiles.ini` lists them; `installs.ini` (or an
/// `[Install…]` section) names the default.
pub fn profiles(root: &Path) -> Vec<PathBuf> {
    let read = |n: &str| std::fs::read_to_string(root.join(n)).unwrap_or_default();
    let (profiles, installs) = (ini(&read("profiles.ini")), ini(&read("installs.ini")));
    let at = |p: &str, relative: bool| if relative { p.split('/').fold(root.to_path_buf(), |a, s| a.join(s)) } else { PathBuf::from(p) };
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| if !out.contains(&p) { out.push(p) };
    // an install's Default is a path; a [ProfileN]'s Default=1 is only a flag
    for (name, kv) in installs.iter().chain(profiles.iter()) {
        if !name.starts_with("Profile") { if let Some(d) = value(kv, "Default") { push(at(d, true)); } }
    }
    for (name, kv) in &profiles {
        if !name.starts_with("Profile") { continue; }
        if let Some(p) = value(kv, "Path") { push(at(p, value(kv, "IsRelative") != Some("0"))); }
    }
    out
}

type Section = (String, Vec<(String, String)>);
fn ini(s: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for line in s.lines().map(str::trim) {
        if let Some(n) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) { out.push((n.to_owned(), Vec::new())); }
        else if let (Some((k, v)), Some(sec)) = (line.split_once('='), out.last_mut()) { sec.1.push((k.trim().to_owned(), v.trim().to_owned())); }
    }
    out
}
fn value<'a>(kv: &'a [(String, String)], k: &str) -> Option<&'a str> { kv.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str()) }

/// The sign-in under a Firefox folder at time `now` (unix seconds).
pub fn sign_in_in(root: &Path, now: i64) -> Result<Option<Vec<Cookie>>> {
    let mut failed = None;
    for p in profiles(root) {
        match read_profile(&p, now) {
            Ok(Some(c)) => return Ok(Some(c)),
            Ok(None) => {}
            Err(e) => failed = Some(e),
        }
    }
    failed.map_or(Ok(None), Err)
}

/// One profile's Nexus sign-in. Firefox holds cookies.sqlite open, and its newest rows sit in the -wal file beside
/// it, so both are copied to a private folder, read there and deleted straight after.
fn read_profile(profile: &Path, now: i64) -> Result<Option<Vec<Cookie>>> {
    let db = profile.join("cookies.sqlite");
    if !db.is_file() { return Ok(None); }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("pockymod-ff-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let r = (|| {
        std::fs::copy(&db, tmp.join("cookies.sqlite"))?;
        let wal = profile.join("cookies.sqlite-wal");
        if wal.is_file() { std::fs::copy(&wal, tmp.join("cookies.sqlite-wal"))?; }
        query(&tmp.join("cookies.sqlite"), now)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn query(db: &Path, now: i64) -> Result<Option<Vec<Cookie>>> {
    let dberr = |e: rusqlite::Error| Error::Db(format!("Firefox cookies: {e}"));
    let c = rusqlite::Connection::open(db).map_err(dberr)?;
    // the default container only (originAttributes ''), not private windows or partitioned copies
    let mut q = c.prepare("SELECT name, value, host, path, expiry, isSecure, isHttpOnly, sameSite FROM moz_cookies
        WHERE host IN ('.nexusmods.com', 'nexusmods.com', 'www.nexusmods.com') AND name IN (?1, ?2) AND originAttributes = ''").map_err(dberr)?;
    let rows = q.query_map(rusqlite::params![SIGN_IN_COOKIES[0], SIGN_IN_COOKIES[1]], |r| {
        // newer Firefox stores expiry in milliseconds, older in seconds
        let e: i64 = r.get(4)?;
        Ok(Cookie {
            name: r.get(0)?, value: Secret::new(r.get::<_, String>(1)?), host: r.get(2)?, path: r.get(3)?,
            expires: if e > 100_000_000_000 { e / 1000 } else { e },
            secure: r.get::<_, i64>(5)? != 0, http_only: r.get::<_, i64>(6)? != 0,
            same_site: match r.get::<_, i64>(7)? { 0 => Some(SameSite::None), 1 => Some(SameSite::Lax), 2 => Some(SameSite::Strict), _ => None },
        })
    }).map_err(dberr)?;
    let cookies: Vec<Cookie> = rows.filter_map(|r| r.ok()).filter(|c| c.expires > now).collect();
    Ok(cookies.iter().any(|c| c.name == SIGN_IN_COOKIES[0]).then_some(cookies))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(root: &Path, dir: &str, rows: &[(&str, &str, &str, i64, &str)]) {
        let p = root.join("Profiles").join(dir);
        std::fs::create_dir_all(&p).unwrap();
        let c = rusqlite::Connection::open(p.join("cookies.sqlite")).unwrap();
        c.execute_batch("CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, originAttributes TEXT NOT NULL DEFAULT '', name TEXT, value TEXT,
            host TEXT, path TEXT, expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER)").unwrap();
        for (host, name, value, expiry, oa) in rows {
            c.execute("INSERT INTO moz_cookies (originAttributes, name, value, host, path, expiry, isSecure, isHttpOnly, sameSite)
                VALUES (?1, ?2, ?3, ?4, '/', ?5, 1, 1, 1)", rusqlite::params![oa, name, value, host, expiry]).unwrap();
        }
    }

    #[test]
    fn reads_only_the_nexus_sign_in() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        std::fs::write(r.join("profiles.ini"), "[Profile1]\nName=old\nIsRelative=1\nPath=Profiles/a.default\nDefault=1\n\n[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/b.default-release\n").unwrap();
        std::fs::write(r.join("installs.ini"), "[308046B0AF4A39CB]\nDefault=Profiles/b.default-release\nLocked=1\n").unwrap();
        // the old profile's sign-in has expired; the default one has a live one (expiry in milliseconds)
        profile(r, "a.default", &[(".nexusmods.com", "nexusmods_session", "old", 1_000, "")]);
        profile(r, "b.default-release", &[
            (".nexusmods.com", "nexusmods_session", "s3cret", 2_000_000_000_000, ""),
            (".nexusmods.com", "nexusmods_session_refresh", "r", 2_000_000_000, ""),
            (".nexusmods.com", "nexusmods_session", "private", 2_000_000_000, "^privateBrowsingId=1"),
            (".nexusmods.com", "cf_clearance", "x", 2_000_000_000, ""),
            (".example.com", "nexusmods_session", "y", 2_000_000_000, ""),
        ]);
        assert_eq!(profiles(r), [r.join("Profiles").join("b.default-release"), r.join("Profiles").join("a.default")]);
        let c = sign_in_in(r, 1_800_000_000).unwrap().unwrap();
        let mut names: Vec<_> = c.iter().map(|c| (c.name.as_str(), c.value.expose())).collect();
        names.sort();
        assert_eq!(names, [("nexusmods_session", "s3cret"), ("nexusmods_session_refresh", "r")]);
        assert_eq!(c[0].expires, 2_000_000_000);
        assert_eq!(c[0].same_site, Some(SameSite::Lax));
        // past the expiry nothing is offered
        assert!(sign_in_in(r, 2_100_000_000).unwrap().is_none());
        // no temp copies left behind
        assert!(!std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with(&format!("pockymod-ff-{}-", std::process::id()))));
    }

    #[test]
    fn no_firefox_is_no_sign_in() {
        let t = tempfile::tempdir().unwrap();
        assert!(sign_in_in(t.path(), 0).unwrap().is_none());
    }
}
