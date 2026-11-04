//! Install and uninstall: archive → staging folder → install plan → deploy. Also the library list and paths.

use crate::archive;
use crate::cyberpunk::layout::{plan_install, InstallPlan, ModType};
use crate::db::{now, Db};
use crate::deploy::{self, DeployOptions, DeployOutcome, UndeployReport};
use crate::error::{Error, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// The mod library next to the game: `<drive>\PocketMods` on Windows, `<game parent>/PocketMods` elsewhere.
/// Same drive as the game, so hard links work, and outside Program Files.
pub fn default_library(game: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(std::path::Component::Prefix(p)) = game.components().next() {
            return PathBuf::from(format!("{}\\PocketMods", p.as_os_str().to_string_lossy()));
        }
    }
    game.parent().map(|p| p.join("PocketMods")).unwrap_or_else(|| PathBuf::from("PocketMods"))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub mod_id: u32,
    pub file_id: u32,
    pub name: String,
    pub version: String,
    pub archive: PathBuf,
    /// Go on although some files have no known place (they are left out).
    #[serde(default)]
    pub accept_unknown: bool,
    /// Remove other installed files of the same mod (an update replaces the old version).
    #[serde(default = "yes")]
    pub replace_older: bool,
    #[serde(default)]
    pub options: DeployOptions,
    /// The user's picks for a FOMOD installer: `[step, group, plugin]`.
    #[serde(default)]
    pub fomod_choices: Option<Vec<[usize; 3]>>,
    /// What it was installed for (the mod that requires it, or the collection), when the user did not ask for it.
    #[serde(default)]
    pub origin: Option<String>,
    /// The user may already have this mod: if its files are already in the game folder (put there by hand, by
    /// Vortex or tracked from either), keep the user's copy and install nothing.
    #[serde(default)]
    pub keep_existing: bool,
    /// The Nexus file's name ("Main file 4K"), kept so updates follow the same file.
    #[serde(default)]
    pub file_name: Option<String>,
    /// With `replace_older`: take out only these installs of the mod (the ones this file updates), not every one.
    #[serde(default)]
    pub replace_only: Option<Vec<i64>>,
}
fn yes() -> bool { true }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum InstallOutcome {
    Installed { install_id: i64, mod_type: ModType, files: usize, needs_redmod_deploy: bool, deploy: DeployOutcome, replaced: Vec<i64>, warnings: Vec<String> },
    /// Some files have no known place. Nothing was installed.
    NeedsReview { plan: InstallPlan },
    /// Staged but not deployed: conflicts or the copy fallback need the user's word. Call `deploy_install`.
    Staged { install_id: i64, deploy: DeployOutcome },
    /// The mod has an installer with options (FOMOD). Ask, then install again with `fomod_choices`.
    NeedsChoices { fomod: crate::fomod::Fomod, defaults: Vec<[usize; 3]> },
    /// The user already has this mod; their copy is kept (and tracked when it was not). Nothing was installed.
    AlreadyHave { install_id: Option<i64>, files: usize },
}

const PLAN_FILE: &str = ".pocket-plan.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRow {
    pub id: i64,
    pub mod_id: u32,
    pub file_id: u32,
    pub name: String,
    pub version: String,
    pub mod_type: ModType,
    pub frameworks: Vec<String>,
    pub staging: String,
    pub installed_at: i64,
    pub updated_at: Option<String>,
    pub deployed_files: i64,
    pub enabled: bool,
    pub load_order: i64,
    /// pocket, vortex (tracked, kept in Vortex's format) or found (was in the game folder already)
    pub source: String,
    pub category: Option<String>,
    /// The user's own group, else the preset one from the category and type.
    pub group: String,
    pub custom_group: bool,
    /// A picture of the mod, from its cached Nexus record.
    pub picture: Option<String>,
    /// The mod or collection it was installed for, when the user did not pick it themselves.
    pub origin: Option<String>,
    /// Files that were already in the game folder when this was installed, set aside (restorable).
    pub replaced: i64,
}

/// The app's own groups, from the Nexus category and the mod type. Users can make their own on top.
pub fn preset_group(category: Option<&str>, kind: ModType, name: &str) -> &'static str {
    let c = category.unwrap_or("").to_lowercase();
    let n = name.to_lowercase();
    if c.contains("preset") || n.contains("preset") { return "Character presets"; }
    if kind == ModType::Framework || c == "modders resources" || c == "utilities" { return "Core"; }
    match c.as_str() {
        "armour and clothing" | "atelier shop" => "Clothing",
        "appearance" | "characters" => "Appearance",
        "gameplay" | "crafting" | "scripts" => "Gameplay",
        "visuals and graphics" | "animations" | "locations" | "props and decorations" | "apartment" | "add-on apartment" | "adverts" => "Visuals and places",
        "weapons" | "vehicles" => "Weapons and vehicles",
        "audio" | "audio replacer" | "ai voices" => "Audio",
        "user interface" => "Interface",
        _ => "Other",
    }
}

pub const PRESET_GROUPS: &[&str] = &["Core", "Clothing", "Character presets", "Appearance", "Gameplay", "Visuals and places", "Weapons and vehicles", "Audio", "Interface", "Other"];

pub fn staging_dir(library: &Path, mod_id: u32, file_id: u32) -> PathBuf { library.join("staging").join(format!("{mod_id}-{file_id}")) }

/// The plan for files already in a staging folder.
pub fn staged_plan(staging: &Path, name: &str) -> InstallPlan {
    // a FOMOD install remembers what was picked; the folder holds every variant
    if let Some(files) = fs::read_to_string(staging.join(PLAN_FILE)).ok().and_then(|t| serde_json::from_str::<Vec<crate::cyberpunk::layout::Placement>>(&t).ok()) {
        return crate::cyberpunk::layout::summarize(files);
    }
    let files: Vec<String> = walkdir::WalkDir::new(staging).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(staging).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .filter(|p| !p.starts_with(".pocket-"))
        .collect();
    let mut files = files;
    files.sort();
    plan_install(&files, name)
}

pub fn install(db: &Db, game: &Path, library: &Path, req: &InstallRequest) -> Result<InstallOutcome> {
    if !game.join("bin/x64").exists() && !game.join("bin").exists() {
        return Err(Error::Invalid(format!("{} does not look like the Cyberpunk 2077 folder (no bin\\x64). Set the game folder in Settings.", game.display())));
    }
    let entries = archive::list(&req.archive)?;
    let staging = staging_dir(library, req.mod_id, req.file_id);
    let fresh = staging.with_extension("new");
    let fomod_cfg = crate::fomod::find_config(&entries);
    // no installer, but variants side by side: a one-choice installer of our own
    let synth = if fomod_cfg.is_none() { crate::fomod::variants(&entries, &req.name) } else { None };
    let (plan, chosen) = if fomod_cfg.is_some() || synth.is_some() {
        let _ = fs::remove_dir_all(&fresh);
        archive::extract(&req.archive, &fresh)?;
        let f = match (&fomod_cfg, synth) {
            (Some(cfg), _) => crate::fomod::parse(&crate::fomod::decode(&fs::read(fresh.join(cfg))?), cfg)?,
            (None, Some(v)) => v,
            (None, None) => unreachable!(),
        };
        match &req.fomod_choices {
            None => { let _ = fs::remove_dir_all(&fresh); let defaults = crate::fomod::defaults(&f); return Ok(InstallOutcome::NeedsChoices { fomod: f, defaults }); }
            Some(c) => { let files = crate::fomod::resolve(&f, c, &entries)?; (crate::cyberpunk::layout::summarize(files.clone()), Some(files)) }
        }
    } else {
        let plan = plan_install(&entries, &req.name);
        if plan.files.is_empty() { return Err(Error::Invalid("Nothing in this archive belongs in the game folder. It may need a manual install; see the mod's page.".into())); }
        if !plan.unknown.is_empty() && !req.accept_unknown { return Ok(InstallOutcome::NeedsReview { plan }); }
        // extract to a fresh folder, then swap it in
        let _ = fs::remove_dir_all(&fresh);
        archive::extract(&req.archive, &fresh)?;
        (plan, None)
    };
    if req.keep_existing {
        if let Some(out) = keep_users_copy(db, game, library, req, &plan)? { let _ = fs::remove_dir_all(&fresh); return Ok(out); }
    }
    if let Some(files) = &chosen { fs::write(fresh.join(PLAN_FILE), serde_json::to_string(files).unwrap_or_default())?; }

    let existing: Option<i64> = db.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 AND file_id = ?2", params![req.mod_id, req.file_id], |r| r.get(0)).optional()?;
    if let Some(id) = existing { deploy::undeploy(db, game, id)?; }
    let _ = fs::remove_dir_all(&staging);
    fs::rename(&fresh, &staging)?;

    let frameworks: Vec<&str> = plan.uses.iter().map(|f| crate::cyberpunk::frameworks::info(*f).key).collect();
    let mod_type = plan.mod_type.unwrap_or(ModType::Unknown);
    db.conn.execute(
        "INSERT INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(mod_id, file_id) DO UPDATE SET name = ?3, version = ?4, mod_type = ?5, frameworks = ?6, staging = ?7, installed_at = ?8",
        params![req.mod_id, req.file_id, req.name, req.version, mod_type.as_str(), serde_json::to_string(&frameworks).unwrap_or_default(), staging.to_string_lossy(), now()])?;
    let install_id: i64 = db.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 AND file_id = ?2", params![req.mod_id, req.file_id], |r| r.get(0))?;
    if let Some((d, _)) = db.mod_get(req.mod_id, None)? {
        if let Some(cat) = serde_json::from_str::<serde_json::Value>(&d).ok().and_then(|v| v["category"].as_str().map(str::to_owned)) {
            db.conn.execute("UPDATE installs SET category = ?2 WHERE id = ?1", params![install_id, cat])?;
        }
    }
    // an update keeps the user's group
    if let Some(g) = db.conn.query_row("SELECT group_name FROM installs WHERE mod_id = ?1 AND id != ?2 AND group_name IS NOT NULL", params![req.mod_id, install_id], |r| r.get::<_, String>(0)).optional()? {
        db.conn.execute("UPDATE installs SET group_name = ?2 WHERE id = ?1", params![install_id, g])?;
    }
    db.conn.execute("UPDATE installs SET origin = ?2, file_name = COALESCE(?3, file_name) WHERE id = ?1", params![install_id, req.origin, req.file_name])?;
    add_to_active_profile(db, install_id)?;

    // an update: take the older file of the same mod out first, put it back if the new one fails
    let older: Vec<i64> = if req.replace_older {
        let mut st = db.conn.prepare("SELECT id FROM installs WHERE mod_id = ?1 AND id != ?2")?;
        let ids = st.query_map(params![req.mod_id, install_id], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
        match &req.replace_only { Some(only) => ids.into_iter().filter(|i| only.contains(i)).collect(), None => ids }
    } else { Vec::new() };
    for id in &older { undeploy_any(db, game, *id)?; }

    let outcome = deploy::deploy(db, game, library, install_id, &staging, &plan.files, &req.options);
    match outcome {
        Ok(d @ DeployOutcome::Deployed { .. }) => {
            for id in &older { to_trash(db, *id, "updated")?; }
            Ok(InstallOutcome::Installed { install_id, mod_type, files: plan.files.len(), needs_redmod_deploy: plan.needs_redmod_deploy, deploy: d, replaced: older, warnings: plan.warnings })
        }
        other => {
            for id in &older { let _ = redeploy_any(db, game, library, *id, &DeployOptions { allow_copy: req.options.allow_copy, ..Default::default() }); }
            match other {
                Ok(d) => Ok(InstallOutcome::Staged { install_id, deploy: d }),
                Err(e) => Err(e),
            }
        }
    }
}

/// Game-relative files of a plan that are already in the game folder and that no install owns: the user's own.
/// Files Vortex deployed (listed in its manifests) count as owned by Vortex and are not returned.
pub fn present_unowned(db: &Db, game: &Path, targets: &[String]) -> Result<Vec<String>> {
    let vortex: std::collections::BTreeSet<String> = crate::vortex::find(game)
        .map(|s| crate::vortex::list_mods(&s, game).into_iter().filter(|m| m.deployed).flat_map(|m| m.files).map(|f| f.to_lowercase()).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for t in targets {
        let k = t.to_lowercase();
        if vortex.contains(&k) || !game.join(t).is_file() { continue; }
        let owned: bool = db.conn.query_row("SELECT EXISTS(SELECT 1 FROM deployed_files WHERE target = ?1)", params![k], |r| r.get(0))?;
        if !owned { out.push(t.clone()); }
    }
    Ok(out)
}

/// For an install added for something else (a requirement, a collection): when the user already has this mod,
/// keep theirs. Their copy is one of: an install of the same Nexus mod, an install from Vortex or the game folder
/// that owns these paths, Vortex's deployed files, or loose files nobody tracks (which are then tracked).
fn keep_users_copy(db: &Db, game: &Path, library: &Path, req: &InstallRequest, plan: &InstallPlan) -> Result<Option<InstallOutcome>> {
    let same: Option<i64> = db.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 ORDER BY installed_at LIMIT 1", params![req.mod_id], |r| r.get(0)).optional()?;
    if let Some(id) = same { return Ok(Some(InstallOutcome::AlreadyHave { install_id: Some(id), files: 0 })); }
    let targets: Vec<String> = plan.files.iter().map(|p| p.target.clone()).collect();
    // a tracked copy (from Vortex or the game folder) owns some of these paths
    for t in &targets {
        let owner: Option<(i64, String, u32)> = db.conn.query_row(
            "SELECT i.id, i.source, i.mod_id FROM deployed_files d JOIN installs i ON i.id = d.install_id WHERE d.target = ?1", params![t.to_lowercase()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        if let Some((id, source, mod_id)) = owner {
            if source != "pocket" {
                // found by its files only: now we know which Nexus mod it is, so updates can be checked
                if mod_id >= 900_000_000 { db.conn.execute("UPDATE installs SET mod_id = ?2, name = ?3 WHERE id = ?1", params![id, req.mod_id, req.name])?; }
                return Ok(Some(InstallOutcome::AlreadyHave { install_id: Some(id), files: 0 }));
            }
        }
    }
    // Vortex has it deployed but it is not tracked: leave it to Vortex
    if let Some(setup) = crate::vortex::find(game) {
        let vf: std::collections::BTreeSet<String> = crate::vortex::list_mods(&setup, game).into_iter().filter(|m| m.deployed).flat_map(|m| m.files).map(|f| f.to_lowercase()).collect();
        let hits = targets.iter().filter(|t| vf.contains(&t.to_lowercase())).count();
        if hits > 0 && hits * 2 >= targets.len().min(4) { return Ok(Some(InstallOutcome::AlreadyHave { install_id: None, files: hits })); }
    }
    // loose files: track them as the user's copy of this mod
    let mine = present_unowned(db, game, &targets)?;
    let key_file = |p: &String| { let l = p.to_lowercase(); l.ends_with(".dll") || l.ends_with(".reds") || l.ends_with(".archive") || l.ends_with(".lua") || l.ends_with(".yaml") || l.ends_with(".xl") || l.ends_with("info.json") };
    if mine.iter().any(key_file) {
        let kind = plan.mod_type.unwrap_or(ModType::Unknown);
        let ids = crate::found::track(db, game, library, &[crate::found::FoundMod { name: req.name.clone(), kind, files: mine.clone(), mod_id: Some(req.mod_id) }])?;
        // a framework's version can be read from its files, so update checks work for the user's copy too
        if let (Some(id), Some(fw)) = (ids.first(), crate::cyberpunk::frameworks::by_mod_id(req.mod_id)) {
            let v = crate::cyberpunk::game::inspect(game).frameworks.into_iter().find(|f| f.key == fw.key).and_then(|f| f.version);
            if let Some(v) = v { db.conn.execute("UPDATE installs SET version = ?2 WHERE id = ?1", params![id, v])?; }
        }
        return Ok(Some(InstallOutcome::AlreadyHave { install_id: ids.first().copied(), files: mine.len() }));
    }
    Ok(None)
}

/// Undo an install that set the user's own files aside: take it out (their files come back), forget it, and track
/// the files that came back as the user's copy of the mod. Returns that new install.
pub fn restore_replaced(db: &Db, game: &Path, library: &Path, install_id: i64) -> Result<Option<i64>> {
    let (mod_id, name, kind): (u32, String, String) = db.conn.query_row("SELECT mod_id, name, mod_type FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let files: Vec<String> = {
        let mut st = db.conn.prepare("SELECT target_path FROM deployed_files WHERE install_id = ?1 AND backup IS NOT NULL")?;
        let v = st.query_map(params![install_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    if files.is_empty() { return Err(Error::Invalid(format!("{name} did not replace any of your files."))); }
    let group: Option<String> = db.conn.query_row("SELECT group_name FROM installs WHERE id = ?1", params![install_id], |r| r.get(0))?;
    deploy::undeploy(db, game, install_id)?;
    to_trash(db, install_id, "replaced your files")?;
    let back: Vec<String> = files.into_iter().filter(|f| game.join(f).is_file()).collect();
    if back.is_empty() { return Ok(None); }
    let ids = crate::found::track(db, game, library, &[crate::found::FoundMod { name, kind: ModType::parse(&kind), files: back, mod_id: Some(mod_id) }])?;
    if let (Some(id), Some(g)) = (ids.first(), group) { db.conn.execute("UPDATE installs SET group_name = ?2 WHERE id = ?1", params![id, g])?; }
    Ok(ids.first().copied())
}

/// Installs that share a Nexus mod with another install: the extra copies, newest first, after the one to keep.
/// The one to keep is the one that is on, then the user's own (Vortex, found, installed by hand), then the newest.
pub fn duplicates(db: &Db) -> Result<Vec<Vec<i64>>> {
    let rows = list_installs(db)?;
    let mut by: std::collections::BTreeMap<u32, Vec<&InstallRow>> = std::collections::BTreeMap::new();
    for r in rows.iter().filter(|r| r.mod_id < 900_000_000) { by.entry(r.mod_id).or_default().push(r); }
    Ok(by.into_values().filter(|v| v.len() > 1).map(|mut v| {
        v.sort_by_key(|r| (!r.enabled, r.origin.is_some(), r.source == "pocket", -r.installed_at));
        v.into_iter().map(|r| r.id).collect()
    }).collect())
}

/// Deploy an install that is already staged (after a conflict or copy decision, or a profile switch).
pub fn redeploy(db: &Db, game: &Path, library: &Path, install_id: i64, opts: &DeployOptions) -> Result<DeployOutcome> {
    let (staging, name): (String, String) = db.conn.query_row("SELECT staging, name FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let plan = staged_plan(Path::new(&staging), &name);
    deploy::deploy(db, game, library, install_id, Path::new(&staging), &plan.files, opts)
}

/// Remove a mod. A mod tracked from Vortex leaves the game and Vortex's manifest, but its folder in Vortex's
/// staging folder is kept (it is Vortex's), so it is still there in Vortex.
pub fn uninstall(db: &Db, game: &Path, install_id: i64) -> Result<UndeployReport> {
    let is_redscript: bool = db.conn.query_row("SELECT mod_id = 1511 FROM installs WHERE id = ?1", params![install_id], |r| r.get(0)).optional()?.unwrap_or(false);
    let rep = undeploy_any(db, game, install_id)?;
    remove_install_record(db, install_id)?;
    // without redscript its compiled scripts break the game: put the game's own back
    if is_redscript { let _ = crate::cyberpunk::game::reset_scripts(game); }
    Ok(rep)
}

/// Where an install came from: "pocket", "vortex" or "found".
pub fn source_of(db: &Db, install_id: i64) -> Result<String> {
    Ok(db.conn.query_row("SELECT source FROM installs WHERE id = ?1", params![install_id], |r| r.get(0)).optional()?.unwrap_or_else(|| "pocket".into()))
}

/// Take a mod out of the game, keeping Vortex's manifest in step for mods tracked from Vortex.
pub fn undeploy_any(db: &Db, game: &Path, install_id: i64) -> Result<UndeployReport> {
    if source_of(db, install_id)? == "vortex" {
        if let Some(setup) = crate::vortex::find(game) { return crate::vortex::remove_from_game(db, game, &setup, install_id, false); }
    }
    deploy::undeploy(db, game, install_id)
}

/// Put a mod (back) in the game, from Vortex's staging folder for mods tracked from Vortex.
pub fn redeploy_any(db: &Db, game: &Path, library: &Path, install_id: i64, opts: &DeployOptions) -> Result<DeployOutcome> {
    if source_of(db, install_id)? == "vortex" {
        if let Some(setup) = crate::vortex::find(game) { return crate::vortex::redeploy(db, game, library, &setup, install_id, opts); }
    }
    redeploy(db, game, library, install_id, opts)
}

fn remove_install_record(db: &Db, install_id: i64) -> Result<()> { to_trash(db, install_id, "removed").map(|_| ()) }

// ── recently removed ──
// A removed install keeps its record and its staged files in the trash (the library's trash folder) for a while,
// so a removal, or the old version an update replaced, can be put back. Vortex's folders are never moved: a
// Vortex mod in the trash is only its record, and putting it back links Vortex's folder again.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashRow { pub id: i64, pub install_id: i64, pub name: String, pub mod_id: u32, pub version: String, pub source: String, pub reason: String, pub size: u64, pub removed_at: i64 }

/// How long removed mods are kept, and how much space they may take.
pub const TRASH_DAYS: i64 = 14;
pub const TRASH_MAX_BYTES: u64 = 4 << 30;

fn dir_size(p: &Path) -> u64 { walkdir::WalkDir::new(p).into_iter().flatten().filter(|e| e.file_type().is_file()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum() }

/// Move an install to the trash: its record (and its profile entries) as JSON, its staged files (not Vortex's)
/// into `<library>/trash`. The caller has already taken it out of the game.
pub fn to_trash(db: &Db, install_id: i64, reason: &str) -> Result<Option<i64>> {
    let row: Option<serde_json::Value> = db.conn.query_row(
        "SELECT mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, updated_at, source, external, category, group_name, origin, file_name FROM installs WHERE id = ?1",
        params![install_id], |r| Ok(serde_json::json!({
            "modId": r.get::<_, u32>(0)?, "fileId": r.get::<_, u32>(1)?, "name": r.get::<_, String>(2)?, "version": r.get::<_, String>(3)?, "modType": r.get::<_, String>(4)?,
            "frameworks": r.get::<_, String>(5)?, "staging": r.get::<_, String>(6)?, "installedAt": r.get::<_, i64>(7)?, "updatedAt": r.get::<_, Option<String>>(8)?,
            "source": r.get::<_, String>(9)?, "external": r.get::<_, Option<String>>(10)?, "category": r.get::<_, Option<String>>(11)?, "group": r.get::<_, Option<String>>(12)?,
            "origin": r.get::<_, Option<String>>(13)?, "fileName": r.get::<_, Option<String>>(14)?,
        }))).optional()?;
    let Some(mut row) = row else { return Ok(None) };
    let profiles: Vec<serde_json::Value> = {
        let mut st = db.conn.prepare("SELECT profile_id, enabled, load_order FROM profile_mods WHERE install_id = ?1")?;
        let v = st.query_map(params![install_id], |r| Ok(serde_json::json!({ "profile": r.get::<_, i64>(0)?, "enabled": r.get::<_, bool>(1)?, "order": r.get::<_, i64>(2)? })))?
            .collect::<std::result::Result<_, _>>()?;
        v
    };
    row["profiles"] = serde_json::Value::Array(profiles);
    let staging = PathBuf::from(row["staging"].as_str().unwrap_or(""));
    let source = row["source"].as_str().unwrap_or("pocket").to_string();
    // move the staged files aside (same drive, so it is a rename); Vortex's folder stays where it is
    let mut kept: Option<PathBuf> = None;
    let mut size = 0;
    if source != "vortex" && staging.is_dir() {
        if let Some(lib) = staging.parent().and_then(|p| p.parent()) {
            let dst = lib.join("trash").join(format!("{install_id}-{}", now()));
            if fs::create_dir_all(dst.parent().unwrap()).is_ok() && fs::rename(&staging, &dst).is_ok() { size = dir_size(&dst); kept = Some(dst); }
        }
        if kept.is_none() { let _ = fs::remove_dir_all(&staging); }
    }
    db.conn.execute("INSERT INTO trash (install_id, name, mod_id, version, source, reason, data, kept, size, removed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![install_id, row["name"].as_str().unwrap_or(""), row["modId"].as_u64().unwrap_or(0) as i64, row["version"].as_str().unwrap_or(""), source, reason,
            row.to_string(), kept.as_ref().map(|k| k.to_string_lossy().into_owned()), size as i64, now()])?;
    let trash_id = db.conn.last_insert_rowid();
    db.conn.execute("DELETE FROM profile_mods WHERE install_id = ?1", params![install_id])?;
    db.conn.execute("DELETE FROM installs WHERE id = ?1", params![install_id])?;
    Ok(Some(trash_id))
}

pub fn list_trash(db: &Db) -> Result<Vec<TrashRow>> {
    let mut st = db.conn.prepare("SELECT id, install_id, name, mod_id, version, source, reason, size, removed_at FROM trash ORDER BY removed_at DESC, id DESC")?;
    let v = st.query_map([], |r| Ok(TrashRow { id: r.get(0)?, install_id: r.get(1)?, name: r.get(2)?, mod_id: r.get(3)?, version: r.get(4)?, source: r.get(5)?,
        reason: r.get(6)?, size: r.get::<_, i64>(7)? as u64, removed_at: r.get(8)? }))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

/// Put a removed install back: its record under its old id (when free), its files where they were, its profile
/// entries, and into the game when it was on in the active profile. Returns the install id.
pub fn restore_trash(db: &Db, game: &Path, library: &Path, trash_id: i64, opts: &DeployOptions) -> Result<i64> {
    let (old_id, data, kept): (i64, String, Option<String>) = db.conn.query_row("SELECT install_id, data, kept FROM trash WHERE id = ?1", params![trash_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?.ok_or_else(|| Error::Invalid("That mod is no longer in Recently removed.".into()))?;
    let v: serde_json::Value = serde_json::from_str(&data).map_err(|e| Error::Decode(e.to_string()))?;
    let staging = PathBuf::from(v["staging"].as_str().unwrap_or(""));
    let (mod_id, file_id) = (v["modId"].as_u64().unwrap_or(0) as u32, v["fileId"].as_u64().unwrap_or(0) as u32);
    let taken: Option<i64> = db.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 AND file_id = ?2", params![mod_id, file_id], |r| r.get(0)).optional()?;
    if let Some(id) = taken { return Err(Error::Invalid(format!("{} is installed again already (install {id}). Remove that one first.", v["name"].as_str().unwrap_or("This mod")))); }
    if let Some(k) = &kept {
        if staging.exists() { let _ = fs::remove_dir_all(&staging); }
        if let Some(p) = staging.parent() { fs::create_dir_all(p)?; }
        fs::rename(k, &staging).map_err(|e| Error::Invalid(format!("Could not put the files back: {e}")))?;
    } else if v["source"] != "vortex" {
        return Err(Error::Invalid("Its files were not kept, so it cannot be put back. Install it again from Browse.".into()));
    } else if !staging.exists() {
        return Err(Error::Invalid("Its folder is no longer in Vortex's mods folder.".into()));
    }
    let free: bool = db.conn.query_row("SELECT NOT EXISTS(SELECT 1 FROM installs WHERE id = ?1)", params![old_id], |r| r.get(0))?;
    db.conn.execute(
        "INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, updated_at, source, external, category, group_name, origin, file_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![if free { Some(old_id) } else { None }, mod_id, file_id, v["name"].as_str(), v["version"].as_str(), v["modType"].as_str(), v["frameworks"].as_str().unwrap_or("[]"),
            v["staging"].as_str(), v["installedAt"].as_i64().unwrap_or_else(now), v["updatedAt"].as_str(), v["source"].as_str().unwrap_or("pocket"), v["external"].as_str(),
            v["category"].as_str(), v["group"].as_str(), v["origin"].as_str(), v["fileName"].as_str()])?;
    let id = db.conn.last_insert_rowid();
    for p in v["profiles"].as_array().into_iter().flatten() {
        let _ = db.conn.execute("INSERT OR IGNORE INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, ?3, ?4)",
            params![p["profile"].as_i64(), id, p["enabled"].as_bool().unwrap_or(true), p["order"].as_i64().unwrap_or(0)]);
    }
    db.conn.execute("DELETE FROM trash WHERE id = ?1", params![trash_id])?;
    let on: bool = db.conn.query_row("SELECT COALESCE((SELECT enabled FROM profile_mods WHERE install_id = ?1 AND profile_id = ?2), 0)", params![id, active_profile(db)?], |r| r.get(0))?;
    if on {
        // another copy of the same mod went in meanwhile (an update): it comes out, the old one goes back
        let newer: Vec<i64> = {
            let mut st = db.conn.prepare("SELECT id FROM installs WHERE mod_id = ?1 AND id != ?2")?;
            let v = st.query_map(params![mod_id, id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            v
        };
        for n in newer { let _ = profiles_set_off(db, game, n); }
        redeploy_any(db, game, library, id, opts)?;
    }
    Ok(id)
}

fn profiles_set_off(db: &Db, game: &Path, install_id: i64) -> Result<()> {
    db.conn.execute("UPDATE profile_mods SET enabled = 0 WHERE install_id = ?1 AND profile_id = ?2", params![install_id, active_profile(db)?])?;
    undeploy_any(db, game, install_id).map(|_| ())
}

/// Delete removed mods for good: one, or every one older than the limit (and the oldest past the size limit).
pub fn purge_trash(db: &Db, only: Option<i64>) -> Result<usize> {
    let rows = list_trash(db)?;
    let cutoff = now() - TRASH_DAYS * 86400;
    let mut total: u64 = 0;
    let mut n = 0;
    for r in &rows {
        total += r.size;
        let go = match only { Some(id) => id == r.id, None => r.removed_at < cutoff || total > TRASH_MAX_BYTES };
        if !go { continue; }
        let kept: Option<String> = db.conn.query_row("SELECT kept FROM trash WHERE id = ?1", params![r.id], |x| x.get(0))?;
        if let Some(k) = kept { let _ = fs::remove_dir_all(k); }
        db.conn.execute("DELETE FROM trash WHERE id = ?1", params![r.id])?;
        n += 1;
    }
    Ok(n)
}

pub fn active_profile(db: &Db) -> Result<i64> {
    Ok(db.conn.query_row("SELECT id FROM profiles WHERE active = 1 ORDER BY id LIMIT 1", [], |r| r.get(0))?)
}

fn add_to_active_profile(db: &Db, install_id: i64) -> Result<()> {
    let p = active_profile(db)?;
    let next: i64 = db.conn.query_row("SELECT COALESCE(MAX(load_order), -1) + 1 FROM profile_mods WHERE profile_id = ?1", params![p], |r| r.get(0))?;
    db.conn.execute("INSERT OR IGNORE INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, 1, ?3)", params![p, install_id, next])?;
    Ok(())
}

pub fn list_installs(db: &Db) -> Result<Vec<InstallRow>> {
    let p = active_profile(db)?;
    let mut st = db.conn.prepare(
        "SELECT i.id, i.mod_id, i.file_id, i.name, i.version, i.mod_type, i.frameworks, i.staging, i.installed_at, i.updated_at,
                (SELECT COUNT(*) FROM deployed_files d WHERE d.install_id = i.id), COALESCE(pm.enabled, 0), COALESCE(pm.load_order, 9999),
                i.source, i.category, i.group_name, (SELECT detail FROM mod_cache c WHERE c.mod_id = i.mod_id), i.origin,
                (SELECT COUNT(*) FROM deployed_files d WHERE d.install_id = i.id AND d.backup IS NOT NULL)
         FROM installs i LEFT JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1 ORDER BY COALESCE(pm.load_order, 9999), i.name")?;
    let rows = st.query_map(params![p], |r| {
        let name: String = r.get(3)?;
        let mod_type = ModType::parse(&r.get::<_, String>(5)?);
        // the category from the install, else from the cached Nexus record
        let category: Option<String> = r.get::<_, Option<String>>(14)?.or_else(|| r.get::<_, Option<String>>(16).ok().flatten()
            .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok()).and_then(|v| v["category"].as_str().map(str::to_owned)));
        let custom: Option<String> = r.get(15)?;
        let picture: Option<String> = r.get::<_, Option<String>>(16).ok().flatten()
            .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok()).and_then(|v| v["pictureUrl"].as_str().map(str::to_owned));
        let group = custom.clone().unwrap_or_else(|| preset_group(category.as_deref(), mod_type, &name).to_string());
        Ok(InstallRow {
            id: r.get(0)?, mod_id: r.get(1)?, file_id: r.get(2)?, version: r.get(4)?, mod_type,
            frameworks: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(), staging: r.get(7)?, installed_at: r.get(8)?, updated_at: r.get(9)?,
            deployed_files: r.get(10)?, enabled: r.get(11)?, load_order: r.get(12)?, source: r.get(13)?, category, group, custom_group: custom.is_some(), name, picture,
            origin: r.get(17)?, replaced: r.get(18)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn record_download(db: &Db, mod_id: u32, file_id: u32, name: &str, version: &str, path: &Path, size: u64, md5: &str) -> Result<()> {
    db.conn.execute(
        "INSERT INTO downloads (mod_id, file_id, name, version, path, size, md5, downloaded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(mod_id, file_id) DO UPDATE SET path = ?5, size = ?6, md5 = ?7, downloaded_at = ?8",
        params![mod_id, file_id, name, version, path.to_string_lossy(), size as i64, md5, now()])?;
    Ok(())
}

/// The archive already downloaded for a file, if it is still on disk.
pub fn downloaded_archive(db: &Db, mod_id: u32, file_id: u32) -> Result<Option<(PathBuf, String)>> {
    let row: Option<(String, String)> = db.conn.query_row("SELECT path, md5 FROM downloads WHERE mod_id = ?1 AND file_id = ?2", params![mod_id, file_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    Ok(row.map(|(p, m)| (PathBuf::from(p), m)).filter(|(p, _)| p.exists()))
}

pub fn installed_mod_ids(db: &Db) -> Result<std::collections::BTreeSet<u32>> {
    let mut st = db.conn.prepare("SELECT DISTINCT mod_id FROM installs")?;
    let ids = st.query_map([], |r| r.get::<_, u32>(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip(path: &Path, files: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, b) in files { z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap(); z.write_all(b.as_bytes()).unwrap(); }
        z.finish().unwrap();
    }

    #[test]
    fn a_requirement_the_user_already_has_keeps_theirs() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        // the user's own TweakXL, copied in by hand
        let mine = game.join("red4ext/plugins/TweakXL/TweakXL.dll");
        fs::create_dir_all(mine.parent().unwrap()).unwrap();
        fs::write(&mine, "mine").unwrap();
        let a = dir.path().join("tweakxl.zip");
        zip(&a, &[("red4ext/plugins/TweakXL/TweakXL.dll", "theirs"), ("r6/scripts/TweakXL/TweakXL.reds", "x")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 4197, file_id: 9, name: "TweakXL".into(), version: "1.10".into(), archive: a, accept_unknown: false, replace_older: true,
            options: Default::default(), fomod_choices: None, origin: Some("requirement".into()), keep_existing: true, file_name: None, replace_only: None };
        let out = install(&db, &game, &lib, &req).unwrap();
        let InstallOutcome::AlreadyHave { install_id: Some(id), .. } = out else { panic!("{out:?}") };
        assert_eq!(fs::read_to_string(&mine).unwrap(), "mine", "the user's file is untouched");
        assert!(!game.join("r6/scripts/TweakXL/TweakXL.reds").exists(), "nothing of the download was installed");
        let rows = list_installs(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].id, rows[0].mod_id, rows[0].source.as_str(), rows[0].enabled), (id, 4197, "found", true));
        // asked again: still theirs, no second copy
        assert!(matches!(install(&db, &game, &lib, &req).unwrap(), InstallOutcome::AlreadyHave { install_id: Some(x), .. } if x == id));
        assert_eq!(list_installs(&db).unwrap().len(), 1);
    }

    #[test]
    fn files_set_aside_by_an_install_can_go_back_to_the_users() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        let mine = game.join("red4ext/plugins/ArchiveXL/ArchiveXL.dll");
        fs::create_dir_all(mine.parent().unwrap()).unwrap();
        fs::write(&mine, "mine").unwrap();
        let a = dir.path().join("axl.zip");
        zip(&a, &[("red4ext/plugins/ArchiveXL/ArchiveXL.dll", "theirs")]);
        let db = Db::memory().unwrap();
        // an install from before this fix: the user's file was set aside
        let req = InstallRequest { mod_id: 4198, file_id: 3, name: "ArchiveXL".into(), version: "1.2".into(), archive: a, accept_unknown: false, replace_older: true,
            options: Default::default(), fomod_choices: None, origin: Some("requirement".into()), keep_existing: false, file_name: None, replace_only: None };
        let InstallOutcome::Installed { install_id, .. } = install(&db, &game, &lib, &req).unwrap() else { panic!() };
        assert_eq!(fs::read_to_string(&mine).unwrap(), "theirs");
        assert_eq!(list_installs(&db).unwrap()[0].replaced, 1);
        let back = restore_replaced(&db, &game, &lib, install_id).unwrap().unwrap();
        assert_eq!(fs::read_to_string(&mine).unwrap(), "mine");
        let rows = list_installs(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].id, rows[0].mod_id, rows[0].source.as_str()), (back, 4198, "found"));
        assert!(duplicates(&db).unwrap().is_empty());
    }

    #[test]
    fn a_removed_mod_can_be_put_back() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        let a = dir.path().join("m.zip");
        zip(&a, &[("archive/pc/mod/coat.archive", "coat")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 7, file_id: 70, name: "Coat".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true,
            options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        let InstallOutcome::Installed { install_id, .. } = install(&db, &game, &lib, &req).unwrap() else { panic!() };
        set_group(&db, &[install_id], Some("Night look")).unwrap();
        uninstall(&db, &game, install_id).unwrap();
        assert!(!game.join("archive/pc/mod/coat.archive").exists());
        assert!(list_installs(&db).unwrap().is_empty());
        let t = list_trash(&db).unwrap();
        assert_eq!((t.len(), t[0].name.as_str(), t[0].reason.as_str()), (1, "Coat", "removed"));
        assert!(t[0].size > 0);
        let back = restore_trash(&db, &game, &lib, t[0].id, &Default::default()).unwrap();
        assert_eq!(back, install_id, "same id, so history entries still point at it");
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/coat.archive")).unwrap(), "coat");
        let rows = list_installs(&db).unwrap();
        assert_eq!((rows[0].group.as_str(), rows[0].enabled), ("Night look", true));
        assert!(list_trash(&db).unwrap().is_empty());
        // removed for good
        uninstall(&db, &game, back).unwrap();
        assert_eq!(purge_trash(&db, None).unwrap(), 0, "recent removals are kept");
        let id = list_trash(&db).unwrap()[0].id;
        assert_eq!(purge_trash(&db, Some(id)).unwrap(), 1);
        assert!(!lib.join("trash").read_dir().unwrap().any(|_| true));
    }

    #[test]
    fn install_update_and_uninstall() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        let db = Db::memory().unwrap();
        let a1 = dir.path().join("v1.zip");
        zip(&a1, &[("Mod/archive/pc/mod/m.archive", "v1"), ("Mod/readme.txt", "hi")]);
        let req = InstallRequest { mod_id: 5, file_id: 10, name: "My Mod".into(), version: "1".into(), archive: a1, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        let out = install(&db, &game, &lib, &req).unwrap();
        let InstallOutcome::Installed { install_id: first, files, .. } = out else { panic!("{out:?}") };
        assert_eq!(files, 1);
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/m.archive")).unwrap(), "v1");
        assert_eq!(list_installs(&db).unwrap()[0].deployed_files, 1);

        // version 2 is another file of the same mod: it replaces version 1
        let a2 = dir.path().join("v2.zip");
        zip(&a2, &[("archive/pc/mod/m.archive", "v2")]);
        let out = install(&db, &game, &lib, &InstallRequest { file_id: 11, version: "2".into(), archive: a2, ..req.clone() }).unwrap();
        let InstallOutcome::Installed { install_id: second, replaced, .. } = out else { panic!("{out:?}") };
        assert_eq!(replaced, [first]);
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/m.archive")).unwrap(), "v2");
        assert_eq!(list_installs(&db).unwrap().len(), 1);
        assert!(!staging_dir(&lib, 5, 10).exists());

        uninstall(&db, &game, second).unwrap();
        assert!(!game.join("archive/pc/mod/m.archive").exists());
        assert!(list_installs(&db).unwrap().is_empty());
    }

    #[test]
    fn a_fomod_asks_then_installs_the_choice_and_remembers_it() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("lib");
        let a = dir.path().join("f.zip");
        let xml = r#"<config><moduleName>J</moduleName><installSteps><installStep name="S"><optionalFileGroups><group name="G" type="SelectExactlyOne"><plugins>
            <plugin name="A"><description/><files><file source="a\j.archive" destination="archive\pc\mod\j.archive"/></files><typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
            <plugin name="B"><description/><files><file source="b\j.archive" destination="archive\pc\mod\j.archive"/></files><typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
            </plugins></group></optionalFileGroups></installStep></installSteps></config>"#;
        zip(&a, &[("fomod/ModuleConfig.xml", xml), ("a/j.archive", "A"), ("b/j.archive", "B")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 3, file_id: 3, name: "J".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        let InstallOutcome::NeedsChoices { fomod, defaults } = install(&db, &game, &lib, &req).unwrap() else { panic!() };
        assert_eq!(fomod.steps[0].groups[0].plugins[1].name, "B");
        assert_eq!(defaults, [[0, 0, 0]]);
        let out = install(&db, &game, &lib, &InstallRequest { fomod_choices: Some(vec![[0, 0, 1]]), ..req }).unwrap();
        let InstallOutcome::Installed { install_id, .. } = out else { panic!("{out:?}") };
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/j.archive")).unwrap(), "B");
        // off and on again: still variant B, not every file in the archive
        deploy::undeploy(&db, &game, install_id).unwrap();
        redeploy(&db, &game, &lib, install_id, &Default::default()).unwrap();
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/j.archive")).unwrap(), "B");
        assert_eq!(fs::read_dir(game.join("archive/pc/mod")).unwrap().count(), 1);
    }

    #[test]
    fn groups() {
        assert_eq!(preset_group(Some("Armour and Clothing"), ModType::Archive, "Jacket"), "Clothing");
        assert_eq!(preset_group(Some("Appearance Change Unlocker Preset"), ModType::Unknown, "V"), "Character presets");
        assert_eq!(preset_group(Some("Characters"), ModType::Archive, "My V preset"), "Character presets");
        assert_eq!(preset_group(Some("Gameplay"), ModType::Framework, "Lib"), "Core");
        assert_eq!(preset_group(None, ModType::Archive, "x"), "Other");
        let db = Db::memory().unwrap();
        db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, category) VALUES (1, 1, 1, 'Hat', '1', 'archive', '[]', '', 0, 'Armour and Clothing')", []).unwrap();
        db.conn.execute("INSERT INTO profile_mods (profile_id, install_id) VALUES (1, 1)", []).unwrap();
        assert_eq!(list_installs(&db).unwrap()[0].group, "Clothing");
        set_group(&db, &[1], Some("My night look")).unwrap();
        assert_eq!(list_installs(&db).unwrap()[0].group, "My night look");
        assert_eq!(custom_groups(&db).unwrap(), ["My night look"]);
        rename_group(&db, "My night look", "Night").unwrap();
        assert_eq!(list_installs(&db).unwrap()[0].group, "Night");
        delete_group(&db, "Night").unwrap();
        assert_eq!(list_installs(&db).unwrap()[0].group, "Clothing");
    }

    #[test]
    fn unknown_files_ask_first() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let a = dir.path().join("x.zip");
        zip(&a, &[("a.archive", "x"), ("tool.exe", "x")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        assert!(matches!(install(&db, &game, &dir.path().join("lib"), &req).unwrap(), InstallOutcome::NeedsReview { .. }));
        let ok = install(&db, &game, &dir.path().join("lib"), &InstallRequest { accept_unknown: true, ..req }).unwrap();
        assert!(matches!(ok, InstallOutcome::Installed { .. }));
    }

    #[test]
    fn refuses_a_folder_that_is_not_the_game() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("x.zip");
        zip(&a, &[("a.archive", "x")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        assert!(install(&db, dir.path(), &dir.path().join("lib"), &req).is_err());
    }
}

// ── groups ──

/// Put mods in one of the user's groups (made if new), or back in their preset group with `None`.
pub fn set_group(db: &Db, install_ids: &[i64], group: Option<&str>) -> Result<()> {
    let g = group.map(str::trim).filter(|g| !g.is_empty());
    if let Some(name) = g {
        db.conn.execute("INSERT OR IGNORE INTO groups (name, sort) VALUES (?1, (SELECT COALESCE(MAX(sort), 0) + 1 FROM groups))", params![name])?;
    }
    let tx = db.conn.unchecked_transaction()?;
    for id in install_ids { tx.execute("UPDATE installs SET group_name = ?2 WHERE id = ?1", params![id, g])?; }
    tx.commit()?;
    Ok(())
}

/// The user's own groups, in their order (empty ones included).
pub fn custom_groups(db: &Db) -> Result<Vec<String>> {
    let mut st = db.conn.prepare("SELECT name FROM groups ORDER BY sort, name")?;
    let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn create_group(db: &Db, name: &str) -> Result<()> {
    let n = name.trim();
    if n.is_empty() { return Err(Error::Invalid("A group needs a name.".into())); }
    db.conn.execute("INSERT OR IGNORE INTO groups (name, sort) VALUES (?1, (SELECT COALESCE(MAX(sort), 0) + 1 FROM groups))", params![n])?;
    Ok(())
}

/// Rename a group; mods in it move with it. Deleting one sends its mods back to their preset groups.
pub fn rename_group(db: &Db, from: &str, to: &str) -> Result<()> {
    let to = to.trim();
    if to.is_empty() { return Err(Error::Invalid("A group needs a name.".into())); }
    db.conn.execute("UPDATE groups SET name = ?2 WHERE name = ?1", params![from, to])?;
    db.conn.execute("UPDATE installs SET group_name = ?2 WHERE group_name = ?1", params![from, to])?;
    Ok(())
}

pub fn delete_group(db: &Db, name: &str) -> Result<()> {
    db.conn.execute("DELETE FROM groups WHERE name = ?1", params![name])?;
    db.conn.execute("UPDATE installs SET group_name = NULL WHERE group_name = ?1", params![name])?;
    Ok(())
}
