//! The logs modders read to find what broke, read the same way: each tool's newest log, its errors and warnings
//! from the last start of the game, and the installed mod each one points at.
//!
//! Where the tools write (from each tool's source, and the modding wiki's troubleshooting page,
//! wiki.redmodding.org "Users: Troubleshooting", log files section):
//! - RED4ext: `red4ext/logs/red4ext-<YYYY-MM-DD-HH-MM-SS>.log`, one file per start. Plugins that log through
//!   RED4ext get `red4ext/logs/<plugin file stem>-<timestamp>.log` next to it. spdlog pattern
//!   `[%Y-%m-%d %H:%M:%S.%e] [%^%-8l%$] [%6t] [%n] %v`: `[2026-11-04 12:00:00.123] [error   ] [  1234] [RED4ext] text`.
//! - ArchiveXL, TweakXL and Codeware (one shared logging core): `red4ext/plugins/<Name>/<Name>-<timestamp>.log`,
//!   one per start (older versions: `<Name>.log`), pattern `[%Y-%m-%d %H:%M:%S.%e] [%t] [%l] %v`. TweakXL names
//!   the file it reads (`Reading "x.yaml"...`) and the errors that follow are about it; ArchiveXL does the same
//!   with `Loading "x.archive.xl"...` and starts its own errors with the file name.
//! - redscript: `r6/logs/redscript_rCURRENT.log`, appended to by every compile of the day. Lines are
//!   `[LEVEL - time] text`, the time RFC 2822 (0.5) or RFC 3339 (1.0); a diagnostic names its `.reds` file
//!   (`At <path>.reds:12:5:` in 0.5, ` --> <path>.reds:12:5` in 1.0) on the lines that follow.
//! - Cyber Engine Tweaks: `bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log`
//!   (`[%Y-%m-%d %H:%M:%S UTC%z] [%l] [%!] [%t] %v`, starting each run with "Cyber Engine Tweaks is starting..."),
//!   `scripting.log` next to it, and each Lua mod's own `mods/<mod>/<mod>.log`. Those two have no level in the
//!   line (`[%Y-%m-%d %H:%M:%S UTC%z] [%t] %v`), so errors are recognised by their words ("error",
//!   "attempt to ...", "stack traceback", "failed").
//!
//! UNVERIFIED against real logs from a Windows install: plugin logs that use neither layout, and the exact
//! wording of every tool's messages. The parser falls back to keywords and to "no mod found" when unsure.

use crate::db::Db;
use crate::error::Result;
use chrono::{NaiveDateTime, TimeZone};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level { Error, Warning }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Raw { Error, Warning, Info }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format { Spdlog, Redscript }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    pub tool: String,
    /// Game-relative, forward slashes.
    pub path: String,
    /// Last written, Unix seconds.
    pub written: Option<i64>,
    /// Written since the game last started.
    pub current: bool,
    pub errors: usize,
    pub warnings: usize,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub log: String,
    pub tool: String,
    pub level: Level,
    pub time: Option<String>,
    /// The message with the lines that belong to it, game paths shortened.
    pub text: String,
    /// The game file or folder it names, game-relative.
    pub subject: Option<String>,
    pub install_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModErrors { pub install_id: i64, pub name: String, pub enabled: bool, pub errors: usize, pub warnings: usize, pub entries: Vec<LogEntry> }

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogReport {
    /// When the game last started, as the logs (or the app's own Play key) tell it. Unix seconds.
    pub last_start: Option<i64>,
    pub logs: Vec<LogFile>,
    /// Errors and warnings from logs of the last start, by the mod they point at.
    pub mods: Vec<ModErrors>,
    /// The rest of them: no mod could be told from the line.
    pub other: Vec<LogEntry>,
    pub errors: usize,
    pub warnings: usize,
    /// redscript said the scripts did not compile on the last start.
    pub scripts_failed: bool,
}

/// A log on disk. `fallback` is what its errors are about when a line names nothing: a mod's own log is about
/// that mod (a CET mod's folder, a RED4ext plugin's DLL), a framework's log is about whatever its lines name.
struct Found { tool: String, rel: String, path: PathBuf, format: Format, per_start: bool, fallback: Option<Subject>, cet_main: bool, name_time: Option<i64> }

const FRAMEWORK_PLUGINS: &[&str] = &["archivexl", "tweakxl", "codeware", "red4ext"];

const CET: &str = "bin/x64/plugins/cyber_engine_tweaks";
/// Read at most this much of the end of a log.
const MAX_READ: u64 = 2 << 20;

fn mtime(p: &Path) -> Option<i64> {
    fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// "red4ext-2026-11-04-12-00-00" → ("red4ext", the time it names, local).
fn split_stamp(stem: &str) -> (String, Option<i64>) {
    let b = stem.as_bytes();
    if b.len() > 20 && b[b.len() - 20] == b'-' {
        let ts = &stem[stem.len() - 19..];
        if let Ok(t) = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d-%H-%M-%S") {
            return (stem[..stem.len() - 20].to_string(), chrono::Local.from_local_datetime(&t).earliest().map(|d| d.timestamp()));
        }
    }
    (stem.to_string(), None)
}

/// The newest log of each name in a folder (`name-<timestamp>.log` or `name.log`). A link to the newest one
/// (ArchiveXL leaves `ArchiveXL.log` pointing at it) is skipped.
fn newest_per_name(dir: &Path) -> Vec<(String, PathBuf, Option<i64>)> {
    let mut by: BTreeMap<String, (String, PathBuf, Option<i64>, i64)> = BTreeMap::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        let is_log = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("log"));
        if !is_log || fs::symlink_metadata(&p).map(|m| !m.is_file()).unwrap_or(true) { continue; }
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (name, t) = split_stamp(&stem);
        let order = t.or_else(|| mtime(&p)).unwrap_or(0);
        let k = name.to_lowercase();
        if by.get(&k).is_none_or(|old| order > old.3) { by.insert(k, (name, p, t, order)); }
    }
    by.into_values().map(|(n, p, t, _)| (n, p, t)).collect()
}

fn find(game: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    let rel = |p: &Path| p.strip_prefix(game).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    for (name, p, t) in newest_per_name(&game.join("red4ext/logs")) {
        let fallback = (!FRAMEWORK_PLUGINS.contains(&name.to_lowercase().as_str())).then(|| Subject::Name(format!("{}.dll", name.to_lowercase())));
        let tool = if name.eq_ignore_ascii_case("red4ext") { "RED4ext".to_string() } else { name };
        out.push(Found { tool, rel: rel(&p), path: p, format: Format::Spdlog, per_start: true, fallback, cet_main: false, name_time: t });
    }
    let mut plugins: Vec<PathBuf> = fs::read_dir(game.join("red4ext/plugins")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    plugins.sort();
    for d in plugins {
        let folder = d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let fallback = (!FRAMEWORK_PLUGINS.contains(&folder.to_lowercase().as_str())).then(|| Subject::Rel(format!("red4ext/plugins/{}/", folder.to_lowercase())));
        for (_, p, t) in newest_per_name(&d) {
            out.push(Found { tool: folder.clone(), rel: rel(&p), path: p, format: Format::Spdlog, per_start: true, fallback: fallback.clone(), cet_main: false, name_time: t });
        }
    }
    let rs = game.join("r6/logs/redscript_rCURRENT.log");
    if rs.is_file() { out.push(Found { tool: "redscript".into(), rel: rel(&rs), path: rs, format: Format::Redscript, per_start: false, fallback: None, cet_main: false, name_time: None }); }
    let cet = game.join(CET);
    for (file, tool, main) in [("cyber_engine_tweaks.log", "Cyber Engine Tweaks", true), ("scripting.log", "CET scripts", false)] {
        let p = cet.join(file);
        if p.is_file() { out.push(Found { tool: tool.into(), rel: rel(&p), path: p, format: Format::Spdlog, per_start: false, fallback: None, cet_main: main, name_time: None }); }
    }
    let mut mods: Vec<PathBuf> = fs::read_dir(cet.join("mods")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    mods.sort();
    for d in mods {
        let folder = d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("log")) {
                out.push(Found { tool: format!("CET mod {folder}"), rel: rel(&p), path: p, format: Format::Spdlog, per_start: false, fallback: Some(Subject::Rel(format!("{CET}/mods/{}/", folder.to_lowercase()))), cet_main: false, name_time: None });
            }
        }
    }
    out
}

fn read_tail(p: &Path) -> String {
    let Ok(mut f) = fs::File::open(p) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len > MAX_READ { let _ = f.seek(SeekFrom::Start(len - MAX_READ)); }
    let mut b = Vec::new();
    let _ = f.read_to_end(&mut b);
    let mut s = String::from_utf8_lossy(&b).into_owned();
    if len > MAX_READ { if let Some(i) = s.find('\n') { s.drain(..=i); } }
    s
}

// ── lines ──

struct Line<'a> { time: &'a str, level: Option<Raw>, msg: &'a str }

fn level_word(w: &str) -> Option<Raw> {
    match w.trim().to_ascii_lowercase().as_str() {
        "error" | "err" | "critical" | "fatal" => Some(Raw::Error),
        "warning" | "warn" => Some(Raw::Warning),
        "info" | "debug" | "trace" => Some(Raw::Info),
        _ => None,
    }
}

fn looks_like_time(s: &str) -> bool { let b = s.as_bytes(); b.len() >= 10 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-' }

/// A line that starts an entry, or None for a line that continues the previous one.
fn parse_line(format: Format, line: &str) -> Option<Line<'_>> {
    let line = line.trim_end_matches('\r');
    if !line.starts_with('[') { return None; }
    if format == Format::Redscript {
        let end = line.find(']')?;
        let (lvl, time) = line[1..end].split_once(" - ")?;
        let level = level_word(lvl)?;
        return Some(Line { time, level: Some(level), msg: line[end + 1..].trim_start() });
    }
    // up to five leading [groups]: the time, then the level, thread, logger or function in the tool's order
    let mut groups: Vec<(&str, usize)> = Vec::new();
    let mut at = 0;
    while groups.len() < 5 && line[at..].starts_with('[') {
        let Some(close) = line[at..].find(']') else { break };
        groups.push((&line[at + 1..at + close], at + close + 1));
        at += close + 1;
        while line[at..].starts_with(' ') { at += 1; }
    }
    let (time, _) = *groups.first()?;
    if !looks_like_time(time) { return None; }
    let digits = |i: usize| groups.get(i).is_some_and(|(g, _)| { let g = g.trim(); !g.is_empty() && g.bytes().all(|c| c.is_ascii_digit()) });
    let li = (1..groups.len().min(3)).find(|&i| level_word(groups[i].0).is_some());
    let used = match li {
        // RED4ext: level, thread, logger name; CET: level, function, thread; the plugins: thread, level
        Some(i) if digits(i + 1) && groups.len() > i + 2 => i + 3,
        Some(i) if digits(i + 2) => i + 3,
        Some(i) => i + 1,
        None => if digits(1) { 2 } else { 1 },
    };
    let end = groups.get(used - 1).map(|g| g.1).unwrap_or(0);
    Some(Line { time, level: li.and_then(|i| level_word(groups[i].0)), msg: line[end..].trim_start() })
}

/// Unix seconds from a log line's time: spdlog's local time (with CET's `UTC+01:00` when given), or redscript's
/// RFC 3339 and RFC 2822 times.
fn unix_of(time: &str) -> Option<i64> {
    let t = time.trim();
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(t) { return Some(d.timestamp()); }
    if let Ok(d) = chrono::DateTime::parse_from_rfc2822(t) { return Some(d.timestamp()); }
    let naive = NaiveDateTime::parse_from_str(t.get(..19)?, "%Y-%m-%d %H:%M:%S").ok()?;
    if let Some(i) = t.find("UTC") {
        let off = t[i + 3..].trim().replace(':', "");
        if off.len() == 5 {
            let sign = if off.starts_with('-') { -1 } else { 1 };
            let (h, m): (i32, i32) = (off[1..3].parse().ok()?, off[3..5].parse().ok()?);
            let o = chrono::FixedOffset::east_opt(sign * (h * 3600 + m * 60))?;
            return o.from_local_datetime(&naive).earliest().map(|d| d.timestamp());
        }
    }
    chrono::Local.from_local_datetime(&naive).earliest().map(|d| d.timestamp())
}

/// For lines with no level (CET's script and mod logs): by their words.
fn guess(msg: &str) -> Raw {
    let l = msg.to_ascii_lowercase();
    if ["error", "attempt to ", "stack traceback", "exception", "failed"].iter().any(|w| l.contains(w)) { Raw::Error }
    else if l.contains("warn") { Raw::Warning } else { Raw::Info }
}

struct Parsed { time: String, unix: Option<i64>, level: Raw, msg: String, block: String }

fn parse(format: Format, text: &str) -> Vec<Parsed> {
    let mut out: Vec<Parsed> = Vec::new();
    for raw in text.lines() {
        match parse_line(format, raw) {
            Some(l) => {
                let level = l.level.unwrap_or_else(|| guess(l.msg));
                out.push(Parsed { time: l.time.to_string(), unix: unix_of(l.time), level, msg: l.msg.to_string(), block: l.msg.to_string() });
            }
            None => if let Some(last) = out.last_mut() {
                // a diagnostic's own lines (redscript's file and code, a Lua stack traceback), up to a point
                if last.block.len() < 1200 && last.block.lines().count() < 10 && !raw.trim().is_empty() { last.block.push('\n'); last.block.push_str(raw.trim_end()); }
            },
        }
    }
    out
}

/// When the last run in an appended log began: after the last pause of more than `gap` seconds.
fn session_start_by_gap(entries: &[Parsed], gap: i64) -> Option<i64> {
    let mut start = None;
    let mut prev: Option<i64> = None;
    for t in entries.iter().filter_map(|e| e.unix) {
        if prev.is_none_or(|p| t - p > gap) { start = Some(t); }
        prev = Some(t);
    }
    start
}

// ── which mod a line points at ──

const ROOTS: &[&str] = &["bin/x64/plugins/cyber_engine_tweaks/mods/", "r6/scripts/", "r6/tweaks/", "archive/pc/mod/", "red4ext/plugins/", "mods/"];
const EXTS: &[&str] = &[".reds", ".archive.xl", ".xl", ".archive", ".yaml", ".yml", ".tweak", ".lua", ".dll", ".asi"];
const STOPS: &[u8] = b"\"'`()<>|\n\t,;=[]";

#[derive(Debug, Clone, PartialEq)]
enum Subject { Rel(String), Name(String) }

/// A game-relative path (lower case, `/`) from a path as a log prints it, absolute or relative.
fn game_relative(p: &str) -> Option<String> {
    let l = p.to_ascii_lowercase().replace('\\', "/");
    for r in ROOTS {
        let mut from = 0;
        while let Some(i) = l[from..].find(r) {
            let i = from + i;
            // `mods/` only as a whole folder name (a REDmod), never the tail of another
            if *r != "mods/" || i == 0 || l.as_bytes()[i - 1] == b'/' { return Some(l[i..].trim_end_matches(['.', ':']).to_string()); }
            from = i + 1;
        }
    }
    None
}

/// Files and mod folders a message names, most specific first.
fn subjects(text: &str) -> Vec<Subject> {
    let lower = text.to_ascii_lowercase();
    let b = lower.as_bytes();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for ext in EXTS {
        let mut from = 0;
        while let Some(i) = lower[from..].find(ext) {
            let i = from + i;
            let end = i + ext.len();
            from = end;
            let next = b.get(end).copied();
            let after = b.get(end + 1).copied();
            let ends = match next { None => true, Some(c) if c.is_ascii_alphanumeric() || c == b'_' => false, Some(b'.') => !after.is_some_and(|c| c.is_ascii_alphanumeric()), _ => true };
            if !ends { continue; }
            let mut start = i;
            while start > 0 && !STOPS.contains(&b[start - 1]) { start -= 1; }
            let cand = text[start..end].trim();
            if !seen.insert(cand.to_ascii_lowercase()) { continue; }
            match game_relative(cand) {
                Some(r) => out.push(Subject::Rel(r)),
                None => out.push(Subject::Name(cand.replace('\\', "/").rsplit('/').next().unwrap_or(cand).to_ascii_lowercase())),
            }
        }
    }
    // a mod folder named without a file (CET: "Mod X failed to load! ('...\mods\X')")
    let norm = lower.replace('\\', "/");
    for r in ROOTS {
        let mut from = 0;
        while let Some(i) = norm[from..].find(r) {
            let i = from + i;
            from = i + r.len();
            if *r == "mods/" && ((i > 0 && norm.as_bytes()[i - 1] != b'/') || norm[..i].ends_with("cyber_engine_tweaks/")) { continue; }
            let rest = &norm[i + r.len()..];
            let comp: String = rest.chars().take_while(|c| *c != '/' && !STOPS.contains(&(*c as u8)) && *c != ':').collect();
            let comp = comp.trim().trim_end_matches('.');
            if !comp.is_empty() && !comp.contains('.') { out.push(Subject::Rel(format!("{}{comp}/", &norm[i..i + r.len()]))); }
        }
    }
    out
}

struct Owners { targets: BTreeMap<String, i64>, by_name: HashMap<String, BTreeSet<i64>> }

impl Owners {
    fn load(db: &Db) -> Result<Owners> {
        let mut st = db.conn.prepare("SELECT target, install_id FROM deployed_files")?;
        let mut targets = BTreeMap::new();
        let mut by_name: HashMap<String, BTreeSet<i64>> = HashMap::new();
        for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (t, i) = r?;
            by_name.entry(t.rsplit('/').next().unwrap_or(&t).to_string()).or_default().insert(i);
            targets.insert(t, i);
        }
        Ok(Owners { targets, by_name })
    }

    /// The install that owns a path, or most of the files in the folder it names (below a game root only).
    fn owner(&self, s: &Subject) -> Option<i64> {
        match s {
            Subject::Name(n) => {
                // a bare name; when the message had words before it, try shorter and shorter tails
                let mut cand = n.as_str();
                loop {
                    if let Some(set) = self.by_name.get(cand) { if set.len() == 1 { return set.first().copied(); } }
                    match cand.split_once(' ') { Some((_, rest)) => cand = rest, None => return None }
                }
            }
            Subject::Rel(p) => {
                if let Some(i) = self.targets.get(p.trim_end_matches('/')) { return Some(*i); }
                let root = ROOTS.iter().find(|r| p.starts_with(*r)).map(|r| r.len()).unwrap_or(0);
                let mut dir = p.trim_end_matches('/').to_string();
                loop {
                    if dir.len() <= root.saturating_sub(1) { return None; }
                    let pre = format!("{dir}/");
                    let mut count: BTreeMap<i64, usize> = BTreeMap::new();
                    for (_, i) in self.targets.range(pre.clone()..).take_while(|(t, _)| t.starts_with(&pre)) { *count.entry(*i).or_default() += 1; }
                    if let Some((i, _)) = count.into_iter().max_by_key(|(_, n)| *n) { return Some(i); }
                    match dir.rfind('/') { Some(k) if k + 1 > root => dir.truncate(k), _ => return None }
                }
            }
        }
    }
}

/// Paths in a message, shortened to game-relative ones.
fn tidy(text: &str, game: &Path) -> String {
    let g = game.to_string_lossy().to_string();
    let mut out = text.to_string();
    for form in [g.clone(), g.replace('\\', "/"), g.replace('/', "\\")] {
        if form.len() < 3 { continue; }
        for sep in ["\\", "/"] {
            let needle = format!("{form}{sep}");
            out = replace_ci(&out, &needle, "");
        }
    }
    out
}

fn replace_ci(hay: &str, needle: &str, with: &str) -> String {
    let lh = hay.to_ascii_lowercase();
    let ln = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(hay.len());
    let mut at = 0;
    while let Some(i) = lh[at..].find(&ln) { out.push_str(&hay[at..at + i]); out.push_str(with); at += i + ln.len(); }
    out.push_str(&hay[at..]);
    out
}

// ── the scan ──

/// Read every tool's newest log. `launched` is when the app last started the game, if it did.
pub fn scan(db: &Db, game: &Path, launched: Option<i64>) -> Result<LogReport> {
    let found = find(game);
    let owners = Owners::load(db)?;
    let mut parsed: Vec<(Found, Vec<Parsed>, Option<i64>)> = Vec::new();
    for f in found {
        let entries = parse(f.format, &read_tail(&f.path));
        parsed.push((f, entries, None));
    }
    // when each appended log's last run began; CET's own log says when CET (so the game) started
    let cet_start = parsed.iter().find(|(f, ..)| f.cet_main)
        .and_then(|(_, e, _)| e.iter().rev().find(|x| x.msg.contains("Cyber Engine Tweaks is starting")).and_then(|x| x.unix));
    for (f, e, start) in parsed.iter_mut() {
        if f.per_start { continue; }
        *start = if f.cet_main { cet_start } else if f.format == Format::Redscript { session_start_by_gap(e, 120) }
            else { cet_start.filter(|s| e.iter().any(|x| x.unix.is_some_and(|t| t >= *s))).or_else(|| session_start_by_gap(e, 1800)) };
    }
    let red4ext_start = parsed.iter().filter(|(f, ..)| f.tool == "RED4ext").filter_map(|(f, ..)| f.name_time).max();
    let redscript_start = parsed.iter().find(|(f, ..)| f.format == Format::Redscript).and_then(|(_, _, s)| *s);
    let now = crate::db::now();
    let last_start = [red4ext_start, cet_start, redscript_start, launched].into_iter().flatten().filter(|t| *t <= now + 300).max();

    // the installs, for names and on/off
    let active = crate::install::active_profile(db)?;
    let mut st = db.conn.prepare("SELECT i.id, i.name, COALESCE(pm.enabled, 0) FROM installs i LEFT JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1")?;
    let names: HashMap<i64, (String, bool)> = st.query_map(params![active], |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, String>(1)?, r.get::<_, bool>(2)?))))?
        .collect::<std::result::Result<_, _>>()?;

    let mut rep = LogReport { last_start, ..Default::default() };
    let mut mods: BTreeMap<i64, ModErrors> = BTreeMap::new();
    for (f, entries, start) in parsed {
        let written = mtime(&f.path);
        let current = match (last_start, written) { (Some(s), Some(w)) => w + 300 >= s, _ => true };
        let mut file = LogFile { tool: f.tool.clone(), path: f.rel.clone(), written, current, errors: 0, warnings: 0, size: fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0) };
        // the file a plugin said it is reading or loading; its next errors are about that file
        let mut context: Vec<Subject> = Vec::new();
        for e in entries.iter().filter(|e| start.is_none_or(|s| e.unix.is_none_or(|t| t >= s))) {
            if e.level == Raw::Info {
                let m = e.msg.trim_start();
                if m.starts_with("Loading") || m.starts_with("Reading") {
                    let mut s = subjects(m);
                    // TweakXL names files relative to r6/tweaks
                    if f.tool.eq_ignore_ascii_case("TweakXL") {
                        s = s.into_iter().flat_map(|x| match &x { Subject::Name(_) => {
                            let inner = m.split('"').nth(1).unwrap_or("").replace('\\', "/").to_ascii_lowercase();
                            vec![Subject::Rel(format!("r6/tweaks/{inner}")), x]
                        } _ => vec![x] }).collect();
                    }
                    context = s;
                } else if m.contains("has been loaded") || m.starts_with("All ") || m.starts_with("Import completed") || m.contains("loaded.") {
                    context.clear();
                }
                if f.format == Format::Redscript && (m.starts_with("Compilation failed") || m.contains("REDScript compilation has failed")) && file.current { rep.scripts_failed = true; }
                continue;
            }
            if f.format == Format::Redscript && e.msg.starts_with("Compilation failed") && file.current { rep.scripts_failed = true; }
            let level = if e.level == Raw::Error { Level::Error } else { Level::Warning };
            if level == Level::Error { file.errors += 1 } else { file.warnings += 1 }
            let mut subs = subjects(&e.block);
            if subs.is_empty() { subs = context.clone(); }
            if let Some(fb) = &f.fallback { subs.push(fb.clone()); }
            let hit = subs.iter().find_map(|s| owners.owner(s).map(|i| (s.clone(), i)));
            let subject = hit.as_ref().map(|(s, _)| s.clone()).or_else(|| subs.first().cloned()).map(|s| match s { Subject::Rel(p) | Subject::Name(p) => p });
            let entry = LogEntry { log: f.rel.clone(), tool: f.tool.clone(), level, time: Some(e.time.clone()), text: tidy(&e.block, game), subject, install_id: hit.map(|(_, i)| i) };
            if !file.current { continue; }
            match entry.install_id {
                Some(id) => {
                    let (name, enabled) = names.get(&id).cloned().unwrap_or_else(|| (format!("Install {id}"), false));
                    let m = mods.entry(id).or_insert_with(|| ModErrors { install_id: id, name, enabled, errors: 0, warnings: 0, entries: Vec::new() });
                    if level == Level::Error { m.errors += 1 } else { m.warnings += 1 }
                    if m.entries.len() < 30 { m.entries.push(entry); }
                }
                None => if rep.other.len() < 60 { rep.other.push(entry); },
            }
        }
        if file.current { rep.errors += file.errors; rep.warnings += file.warnings; }
        rep.logs.push(file);
    }
    let mut mods: Vec<ModErrors> = mods.into_values().collect();
    mods.sort_by(|a, b| b.errors.cmp(&a.errors).then(b.warnings.cmp(&a.warnings)).then(a.name.cmp(&b.name)));
    rep.mods = mods;
    // errors first among the rest
    rep.other.sort_by_key(|e| e.level);
    rep.logs.sort_by(|a, b| b.current.cmp(&a.current).then(b.errors.cmp(&a.errors)).then(a.tool.to_lowercase().cmp(&b.tool.to_lowercase())));
    Ok(rep)
}

/// A plain-text summary to paste on a forum or a mod's bug page: game version, frameworks, how many mods are
/// on, and the errors of the last start. Paths are game-relative and the user's folder never appears.
pub fn report_text(db: &Db, game: &crate::cyberpunk::game::GameInfo, patch: Option<&str>, logs: &LogReport) -> Result<String> {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "Pockymod {} report", crate::APP_VERSION);
    let exe = game.version.as_ref().map(|v| v.product_string.clone().unwrap_or_else(|| v.product.clone()));
    let _ = writeln!(s, "Game: Cyberpunk 2077{}{}", patch.map(|p| format!(" patch {p}")).unwrap_or_default(), exe.map(|v| format!(" (exe {v})")).unwrap_or_default());
    let fws: Vec<String> = game.frameworks.iter().filter(|f| f.installed).map(|f| format!("{} {}", f.name, f.version.as_deref().unwrap_or("(version unknown)"))).collect();
    let _ = writeln!(s, "Frameworks: {}", if fws.is_empty() { "none".to_string() } else { fws.join(", ") });
    let active = crate::install::active_profile(db)?;
    let (on, all): (i64, i64) = db.conn.query_row("SELECT (SELECT COUNT(*) FROM profile_mods WHERE profile_id = ?1 AND enabled = 1), (SELECT COUNT(*) FROM installs)", params![active], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let _ = writeln!(s, "Mods: {on} on of {all} installed");
    let when = logs.last_start.and_then(|t| chrono::Local.timestamp_opt(t, 0).single()).map(|d| d.format("%Y-%m-%d %H:%M").to_string());
    let _ = writeln!(s, "Last game start: {}", when.as_deref().unwrap_or("unknown"));
    let _ = writeln!(s, "Errors: {}, warnings: {}{}", logs.errors, logs.warnings, if logs.scripts_failed { " (redscript: scripts did not compile)" } else { "" });
    let _ = writeln!(s, "\nLogs:");
    for l in &logs.logs {
        let _ = writeln!(s, "- {} ({}): {} errors, {} warnings{}", l.path, l.tool, l.errors, l.warnings, if l.current { "" } else { ", older than the last start" });
    }
    let line = |e: &LogEntry| { let t = e.text.lines().take(4).collect::<Vec<_>>().join(" / "); scrub_user(&t.chars().take(400).collect::<String>()) };
    for m in &logs.mods {
        let _ = writeln!(s, "\n{} ({}): {} errors, {} warnings", m.name, if m.enabled { "on" } else { "off" }, m.errors, m.warnings);
        for e in m.entries.iter().take(8) { let _ = writeln!(s, "  [{}] {}: {}", e.tool, if e.level == Level::Error { "error" } else { "warning" }, line(e)); }
    }
    if !logs.other.is_empty() {
        let _ = writeln!(s, "\nNot tied to one mod:");
        for e in logs.other.iter().take(12) { let _ = writeln!(s, "  [{}] {}: {}", e.tool, if e.level == Level::Error { "error" } else { "warning" }, line(e)); }
    }
    Ok(s)
}

/// A Windows profile folder (`<drive>:` then `Users` and a name, either slash) becomes `%USERPROFILE%`, so a
/// pasted report names nobody.
fn scrub_user(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::new();
    let mut at = 0;
    while let Some(i) = lower[at..].find(":\\users\\").or_else(|| lower[at..].find(":/users/")) {
        let i = at + i;
        let start = i.saturating_sub(1);
        let after = i + ":\\users\\".len();
        let end = s[after..].find(['\\', '/']).map(|k| after + k).unwrap_or(s.len());
        out.push_str(&s[at..start]);
        out.push_str("%USERPROFILE%");
        at = end;
    }
    out.push_str(&s[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(game: &Path, rel: &str, text: &str) {
        let p = game.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn own(db: &Db, id: i64, name: &str, targets: &[&str]) {
        db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?1, ?1, ?2, '1', 'archive', '[]', '', 0)", params![id, name]).unwrap();
        db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (1, ?1, 1, ?1)", params![id]).unwrap();
        for t in targets {
            db.conn.execute("INSERT INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path) VALUES (?1, ?2, '', 'hardlink', 0, 0, ?1)", params![t.to_lowercase(), id]).unwrap();
        }
    }

    #[test]
    fn lines_of_each_tool() {
        let l = parse_line(Format::Spdlog, "[2026-11-04 12:00:00.123] [error   ] [  1234] [RED4ext] Could not load plugin 'x'").unwrap();
        assert_eq!((l.level, l.msg), (Some(Raw::Error), "Could not load plugin 'x'"));
        let l = parse_line(Format::Spdlog, "[2026-11-04 12:00:00.123] [8812] [warning] [GarmentOverride] Mesh not found").unwrap();
        assert_eq!((l.level, l.msg), (Some(Raw::Warning), "[GarmentOverride] Mesh not found"));
        let l = parse_line(Format::Spdlog, "[2026-11-04 12:00:00 UTC+01:00] [info] [CET::Load] [5120] CET version v1.35.0").unwrap();
        assert_eq!((l.level, l.msg), (Some(Raw::Info), "CET version v1.35.0"));
        let l = parse_line(Format::Spdlog, "[2026-11-04 12:00:00 UTC+01:00] [5120] [string \"init.lua\"]:3: attempt to index a nil value").unwrap();
        assert_eq!((l.level, guess(l.msg)), (None, Raw::Error));
        let l = parse_line(Format::Redscript, "[WARN - Thu, 13 Apr 2023 21:54:13 +0200] At C:\\Games\\Cyberpunk 2077\\r6\\scripts\\x.reds:12:1:").unwrap();
        assert_eq!(l.level, Some(Raw::Warning));
        assert_eq!(unix_of(l.time), Some(1681415653));
        assert_eq!(unix_of("2026-11-04T10:00:00Z"), Some(1793786400));
        assert_eq!(unix_of("2026-11-04 12:00:00 UTC+02:00"), Some(1793786400));
        assert!(parse_line(Format::Spdlog, "   at some continuation").is_none());
        assert!(parse_line(Format::Spdlog, "[not a time] x").is_none());
    }

    #[test]
    fn paths_named_in_messages() {
        let s = subjects("At C:\\Games\\Cyberpunk 2077\\r6\\scripts\\Better Netrunning\\main.reds:12:5:");
        assert_eq!(s[0], Subject::Rel("r6/scripts/better netrunning/main.reds".into()));
        assert_eq!(subjects("Loading \"Hair.archive.xl\"...")[0], Subject::Name("hair.archive.xl".into()));
        assert_eq!(subjects("Can't load hair.archive.")[0], Subject::Name("t load hair.archive".into()));
        let s = subjects("Mod Cool HUD failed to load! ('D:\\Cyberpunk 2077\\bin\\x64\\plugins\\cyber_engine_tweaks\\mods\\CoolHUD')");
        assert_eq!(s, [Subject::Rel("bin/x64/plugins/cyber_engine_tweaks/mods/coolhud/".into())]);
        assert!(subjects("Everything fine.").is_empty());
        assert_eq!(game_relative("D:/x/mods/BetterX/scripts/a.reds").as_deref(), Some("mods/betterx/scripts/a.reds"));
        assert_eq!(game_relative("C:/tweakmods/x.yaml"), None);
    }

    #[test]
    fn a_scan_finds_errors_and_the_mods_behind_them() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().join("Cyberpunk 2077");
        let gs = g.to_string_lossy().replace('/', "\\");
        let db = Db::memory().unwrap();
        own(&db, 1, "Better Netrunning", &["r6/scripts/BetterNetrunning/main.reds", "r6/scripts/BetterNetrunning/util.reds"]);
        own(&db, 2, "Hair pack", &["archive/pc/mod/Hair.archive", "archive/pc/mod/Hair.archive.xl"]);
        own(&db, 3, "Cool HUD", &["bin/x64/plugins/cyber_engine_tweaks/mods/CoolHUD/init.lua"]);
        own(&db, 4, "Loot tweaks", &["r6/tweaks/Loot/loot.yaml"]);
        let now = chrono::Local::now();
        let stamp = now.format("%Y-%m-%d-%H-%M-%S").to_string();
        let t = now.format("%Y-%m-%d %H:%M:%S").to_string();
        write(&g, &format!("red4ext/logs/red4ext-{stamp}.log"), &format!("[{t}.001] [info    ] [ 100] [RED4ext] RED4ext (v1.31.0) is initializing...\n[{t}.002] [warning ] [ 100] [RED4ext] Something odd\n"));
        write(&g, "red4ext/logs/red4ext-2020-01-01-00-00-00.log", "[2020-01-01 00:00:00.000] [error   ] [ 1] [RED4ext] old run\n");
        write(&g, &format!("red4ext/plugins/ArchiveXL/ArchiveXL-{stamp}.log"),
            &format!("[{t}.010] [77] [info] Loading \"Hair.archive.xl\"...\n[{t}.011] [77] [error] Hair.archive.xl: Bad format.\n[{t}.012] [77] [info] All archive extensions loaded.\n"));
        write(&g, &format!("red4ext/plugins/TweakXL/TweakXL-{stamp}.log"),
            &format!("[{t}.020] [78] [info] Reading \"Loot\\loot.yaml\"...\n[{t}.021] [78] [error] Items.X: Unknown property y.\n[{t}.022] [78] [info] Import completed.\n"));
        let rt = now.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        write(&g, "r6/logs/redscript_rCURRENT.log", &format!(
            "[INFO - 2020-01-01T00:00:00Z] Running REDscript 1.0\n[ERROR - 2020-01-01T00:00:01Z] an old compile\n\
             [INFO - {rt}] Running REDscript 1.0\n[ERROR - {rt}] error[UNRESOLVED_REF]: 'Foo' is not defined\n --> {gs}\\r6\\scripts\\BetterNetrunning\\main.reds:12:5\n   |\n12 | Foo();\n\
             [ERROR - {rt}] Compilation failed: 1 error\n"));
        let ct = now.format("%Y-%m-%d %H:%M:%S UTC%:z").to_string();
        write(&g, "bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log", &format!("[{ct}] [info] [Options] [9] Cyber Engine Tweaks is starting...\n"));
        write(&g, "bin/x64/plugins/cyber_engine_tweaks/mods/CoolHUD/CoolHUD.log",
            &format!("[2020-01-01 00:00:00 UTC+00:00] [9] old error\n[{ct}] [9] [string \"init.lua\"]:3: attempt to index a nil value\nstack traceback:\n  init.lua:3\n"));

        let r = scan(&db, &g, None).unwrap();
        assert!(r.last_start.is_some());
        assert!(r.scripts_failed);
        let by: HashMap<&str, &ModErrors> = r.mods.iter().map(|m| (m.name.as_str(), m)).collect();
        assert_eq!(by["Better Netrunning"].errors, 1, "{:?}", r.mods);
        assert!(by["Better Netrunning"].entries[0].text.contains("r6\\scripts\\BetterNetrunning\\main.reds"), "game path shortened: {}", by["Better Netrunning"].entries[0].text);
        assert!(!by["Better Netrunning"].entries[0].text.contains(&gs));
        assert_eq!(by["Hair pack"].errors, 1);
        assert_eq!(by["Loot tweaks"].errors, 1);
        assert_eq!(by["Cool HUD"].errors, 1, "only this run's error");
        assert_eq!(r.other.len(), 2, "RED4ext's warning and 'Compilation failed' name no mod: {:?}", r.other);
        assert_eq!(r.errors, 5);
        assert_eq!(r.warnings, 1);
        // the old RED4ext log is not read: only the newest one per tool
        assert_eq!(r.logs.iter().filter(|l| l.tool == "RED4ext").count(), 1);
        assert!(r.logs.iter().all(|l| l.current));

        let info = crate::cyberpunk::game::inspect(&g);
        let text = report_text(&db, &info, Some("2.31"), &r).unwrap();
        assert!(text.contains("patch 2.31") && text.contains("Mods: 4 on of 4 installed") && text.contains("Better Netrunning (on): 1 errors"), "{text}");
        assert!(!text.contains(&gs), "no absolute paths in the report");
    }

    #[test]
    fn user_folders_are_scrubbed() {
        // a profile folder as Windows writes it, made up here so no real one is in the code
        let home = ["C:", "Users", "player1"].join("\\");
        assert_eq!(scrub_user(&format!("at {home}\\AppData\\x.lua:3")), "at %USERPROFILE%\\AppData\\x.lua:3");
        assert_eq!(scrub_user(&format!("at {}/x", home.replace('\\', "/"))), "at %USERPROFILE%/x");
        assert_eq!(scrub_user("nothing here"), "nothing here");
    }

    #[test]
    fn a_log_older_than_the_last_start_is_said_to_be() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().to_path_buf();
        write(&g, "red4ext/plugins/Codeware/Codeware.log", "[2020-01-01 00:00:00.000] [1] [error] stale\n");
        let p = g.join("red4ext/plugins/Codeware/Codeware.log");
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        fs::File::options().write(true).open(&p).unwrap().set_modified(old).unwrap();
        let db = Db::memory().unwrap();
        let r = scan(&db, &g, Some(crate::db::now() - 60)).unwrap();
        assert_eq!(r.logs.len(), 1);
        assert!(!r.logs[0].current);
        assert_eq!(r.logs[0].errors, 1);
        assert_eq!(r.errors, 0, "errors of older logs are not counted for this start");
    }
}
