//! The health panel: what is broken or about to break, most serious first, each with the one action that fixes it.

use crate::cyberpunk::frameworks::{self, Framework, FRAMEWORKS};
use crate::cyberpunk::game::{FrameworkState, GameInfo};
use crate::cyberpunk::patches;
use crate::db::Db;
use crate::deploy;
use crate::error::Result;
use crate::install;
use crate::nexus::types::ModDetail;
use crate::profiles;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Problem, Warning, Info }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Fix {
    InstallMod { mod_id: u32, name: String },
    UpdateMod { mod_id: u32, name: String },
    RunRedmodDeploy,
    Redeploy { install_id: i64 },
    OpenLibrary,
    OpenSettings,
    ResetScripts,
    /// Turn a mod off (the screen does it through its undoable operations).
    TurnOff { install_id: i64, name: String },
    /// Move `archive/pc/mod/modlist.txt` aside.
    DisableModlist,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub severity: Severity,
    /// Which module it belongs to: game, frameworks, deploy, redmod, conflicts, logs.
    pub area: &'static str,
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
}

/// The newest version on Nexus of a framework, from `legacyModsByDomain`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Latest { pub mod_id: u32, pub version: String, pub updated_at: String }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkRow {
    #[serde(flatten)]
    pub state: FrameworkState,
    pub latest: Option<String>,
    pub update_available: bool,
    /// Installed mods in the active profile that need it.
    pub needed_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub game: Option<GameInfo>,
    /// The patch used for checks: detected, else the Settings choice, else the newest known.
    pub patch: Option<String>,
    pub patch_source: &'static str,
    pub frameworks: Vec<FrameworkRow>,
    pub issues: Vec<Issue>,
    pub problems: usize,
    pub warnings: usize,
}

/// "v1.31.0", "1.31", "1.31.0.0" all compare as [1, 31].
pub fn version_key(v: &str) -> Vec<u32> {
    let mut parts: Vec<u32> = v.trim().trim_start_matches(['v', 'V']).split(['.', '-', ' ']).map_while(|p| p.parse().ok()).collect();
    while parts.last() == Some(&0) && parts.len() > 1 { parts.pop(); }
    parts
}

pub fn check(db: &Db, game: Option<&GameInfo>, patch_setting: Option<String>, latest: &[Latest]) -> Result<HealthReport> {
    let mut issues: Vec<Issue> = Vec::new();
    let (patch, patch_source) = match (game.and_then(|g| g.detected_patch.clone()), patch_setting) {
        (Some(p), _) => (Some(p), "detected"),
        (None, Some(p)) => (Some(p), "settings"),
        (None, None) => (Some(patches::latest().version.to_string()), "assumed"),
    };

    // ── game ──
    let Some(g) = game else {
        issues.push(Issue { id: "game.unset".into(), severity: Severity::Problem, area: "game", title: "The game folder is not set".into(),
            detail: "Choose the Cyberpunk 2077 folder".into(), fix: Some(Fix::OpenSettings) });
        return Ok(finish(None, patch, patch_source, Vec::new(), issues));
    };
    if !g.exe_found {
        issues.push(Issue { id: "game.exe".into(), severity: Severity::Problem, area: "game", title: "Cyberpunk2077.exe is missing".into(),
            detail: format!("No bin\\x64\\Cyberpunk2077.exe in {}. Check the game folder in Settings", g.path.display()), fix: Some(Fix::OpenSettings) });
    }
    if !g.writable {
        issues.push(Issue { id: "game.write".into(), severity: Severity::Problem, area: "game", title: "The game folder cannot be written".into(),
            detail: "Windows blocks writing here, often under Program Files. Move the game to another Steam library folder or give your account Modify rights".into(), fix: None });
    }
    if patch_source == "assumed" {
        issues.push(Issue { id: "game.patch".into(), severity: Severity::Info, area: "game", title: format!("Game patch assumed to be {}", patch.as_deref().unwrap_or("?")),
            detail: "Could not read it from the game. Pick it in Settings".into(), fix: Some(Fix::OpenSettings) });
    }

    // ── what the enabled mods need ──
    let active = install::active_profile(db)?;
    let mut st = db.conn.prepare("SELECT i.id, i.mod_id, i.name, i.frameworks, i.mod_type FROM installs i JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1 WHERE pm.enabled = 1")?;
    let mods: Vec<(i64, u32, String, Vec<String>, String)> = st.query_map(params![active], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(), r.get(4)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut needed: BTreeMap<Framework, Vec<String>> = BTreeMap::new();
    let mut needs_dlc: Vec<String> = Vec::new();
    for (_, mod_id, name, fws, _) in &mods {
        if !g.phantom_liberty {
            if let Some((detail, _)) = db.mod_get(*mod_id, None)? {
                if serde_json::from_str::<ModDetail>(&detail).is_ok_and(|d| !d.mod_requirements.dlc_requirements.is_empty()) { needs_dlc.push(name.clone()); }
            }
        }
        let mut set: BTreeSet<Framework> = fws.iter().filter_map(|k| frameworks::by_key(k)).map(|i| i.id).collect();
        // requirement links from the cached Nexus record
        if let Some((detail, _)) = db.mod_get(*mod_id, None)? {
            if let Ok(d) = serde_json::from_str::<ModDetail>(&detail) {
                set.extend(d.mod_requirements.nexus_requirements.nodes.iter().filter_map(|r| frameworks::by_mod_id(r.mod_id)).map(|i| i.id));
            }
        }
        let own = frameworks::by_mod_id(*mod_id).map(|i| i.id);
        let mut all = set.clone();
        for f in &set { all.extend(frameworks::info(*f).needs.iter().copied()); }
        for f in all { if Some(f) != own { needed.entry(f).or_default().push(name.clone()); } }
    }
    if !needs_dlc.is_empty() {
        issues.push(Issue { id: "dlc.missing".into(), severity: Severity::Problem, area: "game", title: "Mods need Phantom Liberty".into(),
            detail: format!("{} need Phantom Liberty, which is not installed. Turn them off or install it", list_names(&needs_dlc)),
            fix: Some(Fix::OpenLibrary) });
    }
    // frameworks need their own requirements too
    for st in &g.frameworks {
        if st.installed { for need in frameworks::info(st.framework).needs { needed.entry(*need).or_default().push(st.name.to_string()); } }
    }

    if crate::cyberpunk::game::stale_scripts(&g.path) {
        issues.push(Issue { id: "scripts.stale".into(), severity: Severity::Problem, area: "game", title: "Leftover redscript cache".into(),
            detail: "redscript is gone but its compiled scripts are still in r6/cache and can stop the game from starting. Reset restores the game's own scripts".into(),
            fix: Some(Fix::ResetScripts) });
    }
    // RED4ext 1.13 and newer do what cybercmd did; the redmodding wiki says to uninstall cybercmd then
    let red4 = g.frameworks.iter().find(|f| f.framework == Framework::Red4ext);
    let red4_covers_cybercmd = red4.is_some_and(|r| r.installed && r.version.as_deref().is_none_or(|v| version_key(v) >= version_key("1.13")));
    if red4_covers_cybercmd {
        needed.remove(&Framework::Cybercmd);
        if g.frameworks.iter().any(|f| f.framework == Framework::Cybercmd && f.installed) {
            issues.push(Issue { id: "fw.cybercmd.obsolete".into(), severity: Severity::Warning, area: "frameworks", title: "cybercmd is no longer needed".into(),
                detail: "RED4ext 1.13+ replaces it. Turn it off in Library and check the game still starts".into(),
                fix: Some(Fix::OpenLibrary) });
        }
    }

    let exe_patch_time = g.exe_modified;
    let mut rows = Vec::new();
    for st in &g.frameworks {
        let info = frameworks::info(st.framework);
        let lat = latest.iter().find(|l| l.mod_id == info.nexus_mod_id);
        let update_available = match (&st.version, lat) { (Some(v), Some(l)) => version_key(v) < version_key(&l.version), _ => false };
        let needed_by = needed.get(&st.framework).cloned().unwrap_or_default();
        if !st.installed && !needed_by.is_empty() {
            let mut who = needed_by.clone();
            who.dedup();
            issues.push(Issue { id: format!("fw.missing.{}", info.key), severity: Severity::Problem, area: "frameworks", title: format!("{} is missing", info.name),
                detail: format!("Needed by {}. Without it they fail or stop the game from starting", list_names(&who)),
                fix: Some(Fix::InstallMod { mod_id: info.nexus_mod_id, name: info.name.into() }) });
        }
        if st.installed && st.older_than_game {
            // a framework file older than the game's last patch: frameworks hook the executable and break on patches
            let newer_on_nexus = lat.and_then(|l| chrono::DateTime::parse_from_rfc3339(&l.updated_at).ok()).map(|t| t.timestamp())
                .zip(exe_patch_time).is_some_and(|(t, e)| t > e);
            issues.push(Issue { id: format!("fw.old.{}", info.key), severity: if newer_on_nexus || update_available { Severity::Problem } else { Severity::Warning }, area: "frameworks",
                title: format!("{} is older than the game patch", info.name),
                detail: if newer_on_nexus || update_available { format!("Nexus has a newer version{1}. Update {0} before you play", info.name, lat.map(|l| format!(" ({})", l.version)).unwrap_or_default()) }
                        else { format!("If the game crashes at start, check {}'s page for an update", info.name) },
                fix: Some(Fix::UpdateMod { mod_id: info.nexus_mod_id, name: info.name.into() }) });
        } else if st.installed && update_available {
            issues.push(Issue { id: format!("fw.update.{}", info.key), severity: Severity::Warning, area: "frameworks", title: format!("{} has an update", info.name),
                detail: format!("Installed {}, newest {}", st.version.as_deref().unwrap_or("?"), lat.map(|l| l.version.as_str()).unwrap_or("?")),
                fix: Some(Fix::UpdateMod { mod_id: info.nexus_mod_id, name: info.name.into() }) });
        }
        rows.push(FrameworkRow { state: st.clone(), latest: lat.map(|l| l.version.clone()), update_available, needed_by });
    }

    // ── deploy state ──
    let broken = deploy::verify(db, &g.path)?;
    let mut by_install: BTreeMap<i64, usize> = BTreeMap::new();
    for (i, _) in &broken { *by_install.entry(*i).or_default() += 1; }
    for (install_id, n) in by_install {
        let name = mods.iter().find(|m| m.0 == install_id).map(|m| m.2.clone()).unwrap_or_else(|| format!("Install {install_id}"));
        issues.push(Issue { id: format!("deploy.broken.{install_id}"), severity: Severity::Problem, area: "deploy", title: format!("{name}: {n} files missing or changed"),
            detail: "Something outside Pockymod removed or replaced them. Deploy again to restore them".into(), fix: Some(Fix::Redeploy { install_id }) });
    }
    for (install_id, _, name, _, _) in &mods {
        let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM deployed_files WHERE install_id = ?1", params![install_id], |r| r.get(0))?;
        if n == 0 {
            issues.push(Issue { id: format!("deploy.staged.{install_id}"), severity: Severity::Warning, area: "deploy", title: format!("{name} is not in the game"),
                detail: "Enabled but not deployed, usually because its files clash with another mod. Decide in Library".into(), fix: Some(Fix::OpenLibrary) });
        }
    }
    // ── conflicts: paths one mod took from another ──
    let mut st = db.conn.prepare("SELECT a.name, b.name, COUNT(*) FROM deployed_files d JOIN installs a ON a.id = d.install_id JOIN installs b ON b.id = d.prev_install GROUP BY a.id, b.id")?;
    let over: Vec<(String, String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<std::result::Result<_, _>>()?;
    for (a, b, n) in over {
        issues.push(Issue { id: format!("conflict.{a}.{b}"), severity: Severity::Info, area: "conflicts", title: format!("{a} overrides {n} files of {b}"),
            detail: format!("You picked {a}'s copies. Removing {a} gives {b} its files back"), fix: None });
    }

    // ── REDmod ──
    let redmods = mods.iter().filter(|m| m.4 == "redmod").count();
    if redmods > 0 && !g.redmod_available {
        issues.push(Issue { id: "redmod.tool".into(), severity: Severity::Problem, area: "redmod", title: "The REDmod tool is not installed".into(),
            detail: format!("{redmods} REDmods are installed without the free REDmod DLC. Add it in Steam or GOG Galaxy"), fix: None });
    } else if profiles::redmod_needs_deploy(db)? {
        issues.push(Issue { id: "redmod.deploy".into(), severity: Severity::Warning, area: "redmod", title: "REDmods need deploying".into(),
            detail: "REDmods changed since the last deploy. The game won't see it until deploy runs".into(), fix: Some(Fix::RunRedmodDeploy) });
    }

    // ── archive order: a stray modlist.txt decides it instead of file names ──
    if let Some(list) = crate::cyberpunk::rdar::modlist(&g.path) {
        issues.push(Issue { id: "archive.modlist".into(), severity: Severity::Warning, area: "conflicts", title: "A modlist.txt decides the archive order".into(),
            detail: format!("archive/pc/mod/modlist.txt lists {} archives and overrides name order. Usually left by another tool. Moving it aside keeps a copy as modlist.txt.off", list.len()),
            fix: Some(Fix::DisableModlist) });
    }

    // ── logs of the last start: mods that report errors ──
    let launched = db.setting("game.lastLaunch")?.and_then(|v| v.parse().ok());
    let logs = crate::cyberpunk::logs::scan(db, &g.path, launched)?;
    issues.extend(log_issues(&logs));

    Ok(finish(Some(g.clone()), patch, patch_source, rows, issues))
}

/// Health issues from the logs: scripts that did not compile, and each enabled mod with errors on the last start.
pub fn log_issues(logs: &crate::cyberpunk::logs::LogReport) -> Vec<Issue> {
    let mut out = Vec::new();
    if logs.scripts_failed {
        out.push(Issue { id: "logs.scripts".into(), severity: Severity::Problem, area: "logs", title: "Scripts did not compile on the last start".into(),
            detail: "No script mod works until this is fixed. Turn off the mods named below one at a time to find it".into(), fix: None });
    }
    for m in logs.mods.iter().filter(|m| m.errors > 0 && m.enabled) {
        let first = m.entries.iter().find(|e| e.level == crate::cyberpunk::logs::Level::Error).map(|e| e.text.lines().next().unwrap_or("").chars().take(160).collect::<String>()).unwrap_or_default();
        let tools: BTreeSet<&str> = m.entries.iter().map(|e| e.tool.as_str()).collect();
        let serious = tools.contains("redscript") || tools.contains("RED4ext");
        out.push(Issue { id: format!("logs.mod.{}", m.install_id), severity: if serious { Severity::Problem } else { Severity::Warning }, area: "logs",
            title: format!("{} reports errors on start", m.name),
            detail: format!("{} {} in the {} {}: {first}", m.errors, if m.errors == 1 { "error" } else { "errors" }, tools.into_iter().collect::<Vec<_>>().join(", "), "log"),
            fix: Some(Fix::TurnOff { install_id: m.install_id, name: m.name.clone() }) });
    }
    out
}

fn finish(game: Option<GameInfo>, patch: Option<String>, patch_source: &'static str, frameworks: Vec<FrameworkRow>, mut issues: Vec<Issue>) -> HealthReport {
    issues.sort_by_key(|i| i.severity);
    let problems = issues.iter().filter(|i| i.severity == Severity::Problem).count();
    let warnings = issues.iter().filter(|i| i.severity == Severity::Warning).count();
    HealthReport { game, patch, patch_source, frameworks, issues, problems, warnings }
}

fn list_names(v: &[String]) -> String {
    match v.len() { 0 => String::new(), 1..=3 => v.join(", "), n => format!("{}, {} and {} more", v[0], v[1], n - 2) }
}

/// Framework IDs to look up on Nexus for the health check.
pub fn framework_ids() -> Vec<u32> { FRAMEWORKS.iter().map(|f| f.nexus_mod_id).collect() }

pub fn game_info(game: Option<&Path>) -> Option<GameInfo> { game.map(crate::cyberpunk::game::inspect) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cyberpunk::game::inspect;

    fn add_install(db: &Db, id: i64, mod_id: u32, name: &str, fws: &[&str], kind: &str) {
        db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?2, 1, ?3, '1', ?4, ?5, '/nowhere', 100)",
            params![id, mod_id, name, kind, serde_json::to_string(fws).unwrap()]).unwrap();
        db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (1, ?1, 1, ?1)", params![id]).unwrap();
    }

    #[test]
    fn versions_compare_loosely() {
        assert_eq!(version_key("v1.31.0"), vec![1, 31]);
        assert_eq!(version_key("1.31.0.0"), version_key("1.31"));
        assert!(version_key("1.28.6") < version_key("1.29"));
    }

    #[test]
    fn missing_frameworks_and_redmod_tool() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin/x64")).unwrap();
        let db = Db::memory().unwrap();
        add_install(&db, 1, 500, "Hair pack", &["archivexl"], "archive");
        add_install(&db, 2, 501, "Better quests", &[], "redmod");
        let g = inspect(dir.path());
        let r = check(&db, Some(&g), Some("2.3".into()), &[]).unwrap();
        let ids: Vec<&str> = r.issues.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"fw.missing.archivexl") && ids.contains(&"fw.missing.red4ext"), "{ids:?}");
        assert!(ids.contains(&"redmod.tool"));
        assert!(ids.contains(&"game.exe"));
        assert_eq!(r.patch.as_deref(), Some("2.3"));
        assert_eq!(r.issues[0].severity, Severity::Problem);
        let fix = r.issues.iter().find(|i| i.id == "fw.missing.red4ext").unwrap().fix.clone();
        assert_eq!(fix, Some(Fix::InstallMod { mod_id: 2380, name: "RED4ext".into() }));
        assert!(r.issues.iter().any(|i| i.id == "deploy.staged.1"));
    }

    #[test]
    fn update_available_from_latest() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path();
        std::fs::create_dir_all(g.join("red4ext")).unwrap();
        std::fs::create_dir_all(g.join("bin/x64")).unwrap();
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-fixture.dll"), g.join("red4ext/RED4ext.dll")).unwrap();
        let db = Db::memory().unwrap();
        let info = inspect(g);
        let r = check(&db, Some(&info), None, &[Latest { mod_id: 2380, version: "1.32.0".into(), updated_at: "2026-11-01T00:00:00Z".into() }]).unwrap();
        assert!(r.issues.iter().any(|i| i.id == "fw.update.red4ext"));
        assert!(r.frameworks.iter().find(|f| f.state.key == "red4ext").unwrap().update_available);
        assert_eq!(r.patch_source, "assumed");
    }
}
