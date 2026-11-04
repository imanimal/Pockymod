//! Vortex compatibility: read the mods Vortex installed, track them as ordinary mods, and update them so Vortex
//! still sees them as its own.
//!
//! From Vortex's source (github.com/Nexus-Mods/vortex, read 2026-11-04):
//! - Each mod lives in a folder of the staging folder (default `%APPDATA%\Vortex\cyberpunk2077\mods`), named after
//!   its archive without the extension (`deriveModInstallName`). Vortex rescans that folder on start: a folder it
//!   does not know raises a "mods changed on disk" dialog, a folder that disappears loses its meta data for good.
//! - Each deployment target holds a manifest `vortex.deployment.json` (or `vortex.deployment.<modtype>.json`):
//!   `{ version: 1, instance, deploymentMethod?, deploymentTime?, stagingPath?, gameId?, targetPath?,
//!      files: [{ relPath, source, target?, merged?, time }] }`, `source` being the mod folder name, `time` the
//!   file's modification time in ms. Vortex reads that file first; its backups in the staging folder
//!   (`vortex.deployment.json`, `vortex.deployment.msgpack`) are used only when it is damaged.
//! So, to keep a mod Vortex's: change files inside its existing staging folder, and keep the manifest in step.
//! The mod's version in Vortex's own database (LevelDB, not touched) stays the old one until Vortex rechecks.

use crate::archive;
use crate::cyberpunk::layout::{plan_install, ModType};
use crate::db::{now, Db};
use crate::deploy::{self, DeployOptions, DeployOutcome};
use crate::error::{Error, Result};
use crate::install;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// A Nexus archive name: `<name>-<modId>-<version>-<unix time>`, e.g. `ArchiveXL-4198-1-28-6-1761742782`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveName { pub name: String, pub mod_id: u32, pub version: String, pub timestamp: i64 }

/// Every reading of a Nexus archive name, most likely first (names can hold digits and dashes themselves).
pub fn parse_archive_name(folder: &str) -> Vec<ArchiveName> {
    // Vortex adds ".1", ".2" to repeated installs of the same archive
    let base = match folder.rsplit_once('.') { Some((b, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => b, _ => folder };
    let t: Vec<&str> = base.split('-').collect();
    let Some(ts) = t.last().filter(|x| (9..=11).contains(&x.len()) && x.chars().all(|c| c.is_ascii_digit())).and_then(|x| x.parse().ok()) else { return Vec::new() };
    let mut out = Vec::new();
    for i in 1..t.len().saturating_sub(1) {
        if !t[i].chars().all(|c| c.is_ascii_digit()) || t[i].is_empty() { continue; }
        let Ok(mod_id) = t[i].parse::<u32>() else { continue };
        let name = t[..i].join("-");
        let version = t[i + 1..t.len() - 1].join(".");
        out.push(ArchiveName { name, mod_id, version, timestamp: ts });
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    /// Manifests found in the game folder, with the folder each one describes.
    pub manifests: Vec<(PathBuf, PathBuf)>,
    pub staging: Option<PathBuf>,
    pub downloads: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VortexMod {
    pub folder: String,
    /// Game-relative paths, forward slashes.
    pub files: Vec<String>,
    /// In the game now (listed in a manifest). A mod in the staging folder but not deployed is disabled in Vortex.
    pub deployed: bool,
    pub candidates: Vec<ArchiveName>,
}

fn appdata() -> Option<PathBuf> { std::env::var_os("APPDATA").map(PathBuf::from) }

/// Where Vortex keeps Cyberpunk 2077's mods, if it has deployed any.
pub fn find(game: &Path) -> Option<Setup> {
    let mut manifests = Vec::new();
    for dir in ["", "mods", "archive/pc/mod", "r6/scripts", "r6/tweaks", "bin/x64/plugins/cyber_engine_tweaks/mods", "red4ext/plugins"] {
        let d = if dir.is_empty() { game.to_path_buf() } else { game.join(dir) };
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("vortex.deployment") && n.ends_with(".json") { manifests.push((e.path(), d.clone())); }
        }
    }
    let staging = manifests.iter().find_map(|(p, _)| read_json(p).ok().and_then(|v| v["stagingPath"].as_str().map(PathBuf::from)))
        .or_else(|| appdata().map(|a| a.join("Vortex").join(crate::GAME_DOMAIN).join("mods")))
        .filter(|p| p.exists());
    if manifests.is_empty() && staging.is_none() { return None; }
    let downloads = appdata().map(|a| a.join("Vortex").join("downloads").join(crate::GAME_DOMAIN)).filter(|p| p.exists());
    Some(Setup { manifests, staging, downloads })
}

fn read_json(p: &Path) -> Result<Value> {
    let text = fs::read_to_string(p)?;
    serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("{} is not valid JSON: {e}", p.display())))
}

fn rel_of(game: &Path, dir: &Path) -> String {
    dir.strip_prefix(game).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default().trim_matches('/').to_string()
}

fn join_rel(a: &str, b: &str) -> String {
    let b = b.replace('\\', "/");
    let b = b.trim_matches('/');
    if a.is_empty() { b.to_string() } else if b.is_empty() { a.to_string() } else { format!("{a}/{b}") }
}

/// The mods Vortex has: deployed ones from the manifests, disabled ones from the staging folder.
pub fn list_mods(setup: &Setup, game: &Path) -> Vec<VortexMod> {
    let mut by: BTreeMap<String, VortexMod> = BTreeMap::new();
    for (path, dir) in &setup.manifests {
        let Ok(v) = read_json(path) else { continue };
        let base = rel_of(game, dir);
        for f in v["files"].as_array().into_iter().flatten() {
            let (Some(rel), Some(src)) = (f["relPath"].as_str(), f["source"].as_str()) else { continue };
            let target = f["target"].as_str().unwrap_or("");
            let path = join_rel(&join_rel(&base, target), rel);
            let m = by.entry(src.to_string()).or_insert_with(|| VortexMod { folder: src.into(), files: Vec::new(), deployed: true, candidates: parse_archive_name(src) });
            m.files.push(path);
        }
    }
    if let Some(st) = &setup.staging {
        for e in fs::read_dir(st).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !e.path().is_dir() || name.starts_with("__") || name.ends_with(".installing") || by.contains_key(&name) { continue; }
            let files: Vec<String> = walkdir::WalkDir::new(e.path()).into_iter().flatten().filter(|x| x.file_type().is_file())
                .filter_map(|x| x.path().strip_prefix(e.path()).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
                .filter(|p| !p.starts_with("__")).collect();
            by.insert(name.clone(), VortexMod { candidates: parse_archive_name(&name), folder: name, files, deployed: false });
        }
    }
    by.into_values().collect()
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackReport { pub tracked: usize, pub matched_on_nexus: usize, pub already: usize, pub install_ids: Vec<i64> }

/// A mod Vortex installed, with its Nexus identity when it could be matched.
#[derive(Debug, Clone)]
pub struct Resolved { pub vortex: VortexMod, pub mod_id: Option<u32>, pub file_id: Option<u32>, pub name: String, pub version: String, pub category: Option<String> }

/// Record Vortex's mods as tracked installs: they show and update like any other, and stay in Vortex's format.
pub fn track(db: &Db, game: &Path, setup: &Setup, mods: &[Resolved]) -> Result<TrackReport> {
    let mut rep = TrackReport::default();
    let staging = setup.staging.clone().unwrap_or_default();
    let profile = install::active_profile(db)?;
    for r in mods {
        let exists: Option<i64> = db.conn.query_row("SELECT id FROM installs WHERE source = 'vortex' AND external = ?1", params![r.vortex.folder], |x| x.get(0)).optional()?;
        if exists.is_some() { rep.already += 1; continue; }
        let local = || 900_000_000 + (r.vortex.folder.bytes().fold(7u32, |a, c| a.wrapping_mul(31).wrapping_add(c as u32)) % 90_000_000);
        let (mod_id, file_id) = match (r.mod_id, r.file_id) { (Some(m), Some(f)) => (m, f), (Some(m), None) => (m, local()), _ => (local(), local()) };
        if r.mod_id.is_some() { rep.matched_on_nexus += 1; }
        let plan = plan_install(&r.vortex.files, &r.name);
        let fws: Vec<&str> = plan.uses.iter().map(|f| crate::cyberpunk::frameworks::info(*f).key).collect();
        let kind = plan.mod_type.unwrap_or(ModType::Unknown);
        let dir = staging.join(&r.vortex.folder);
        db.conn.execute(
            "INSERT OR IGNORE INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, source, external, category)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'vortex', ?9, ?10)",
            params![mod_id, file_id, r.name, r.version, kind.as_str(), serde_json::to_string(&fws).unwrap_or_default(), dir.to_string_lossy(), now(), r.vortex.folder, r.category])?;
        let id: i64 = db.conn.query_row("SELECT id FROM installs WHERE source = 'vortex' AND external = ?1", params![r.vortex.folder], |x| x.get(0))?;
        db.conn.execute("INSERT OR IGNORE INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(load_order), -1) + 1 FROM profile_mods WHERE profile_id = ?1))",
            params![profile, id, r.vortex.deployed as i32])?;
        if r.vortex.deployed {
            for f in &r.vortex.files {
                let target = game.join(f);
                let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                // the deployed file is Vortex's hard link (or copy) of the staging file
                db.conn.execute("INSERT OR IGNORE INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path) VALUES (?1, ?2, ?3, 'vortex', ?4, ?5, ?6)",
                    params![f.to_lowercase(), id, dir.join(f).to_string_lossy(), size as i64, now(), f])?;
            }
        }
        rep.tracked += 1;
        rep.install_ids.push(id);
    }
    Ok(rep)
}

fn mtime_ms(p: &Path) -> i64 {
    fs::metadata(p).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Rewrite the manifests so the entries for `folder` list exactly `files` (game-relative). Files go in the manifest
/// whose folder holds them (the deepest), falling back to the game folder's own manifest. Other entries are kept.
pub fn rewrite_manifests(setup: &Setup, game: &Path, folder: &str, files: &[String]) -> Result<()> {
    let staging = setup.staging.clone();
    let mut sorted = setup.manifests.clone();
    sorted.sort_by_key(|(_, d)| std::cmp::Reverse(d.components().count()));
    let mut placed = vec![false; files.len()];
    let mut root_manifest: Option<(PathBuf, PathBuf)> = None;
    for (path, dir) in &sorted {
        let mut v = read_json(path)?;
        let base = rel_of(game, dir);
        if base.is_empty() { root_manifest = Some((path.clone(), dir.clone())); }
        let mut list: Vec<Value> = v["files"].as_array().cloned().unwrap_or_default().into_iter().filter(|f| f["source"].as_str() != Some(folder)).collect();
        for (i, f) in files.iter().enumerate() {
            if placed[i] { continue; }
            let under = base.is_empty() || f.to_lowercase().starts_with(&format!("{}/", base.to_lowercase()));
            if !under { continue; }
            let rel = if base.is_empty() { f.clone() } else { f[base.len() + 1..].to_string() };
            list.push(json!({ "relPath": rel.replace('/', "\\"), "source": folder, "target": "", "time": mtime_ms(&game.join(f)) }));
            placed[i] = true;
        }
        v["files"] = Value::Array(list);
        v["deploymentTime"] = json!(chrono::Utc::now().timestamp_millis());
        write_manifest(path, &v, staging.as_deref())?;
    }
    if placed.iter().any(|p| !p) && root_manifest.is_none() {
        // nothing at the game folder yet: start one, as Vortex would
        let mut v = json!({ "version": 1, "instance": "", "gameId": crate::GAME_DOMAIN, "targetPath": game.to_string_lossy(), "files": [] });
        if let Some(s) = &staging { v["stagingPath"] = json!(s.to_string_lossy()); }
        let list: Vec<Value> = files.iter().zip(&placed).filter(|(_, p)| !**p).map(|(f, _)| json!({ "relPath": f.replace('/', "\\"), "source": folder, "target": "", "time": mtime_ms(&game.join(f)) })).collect();
        v["files"] = Value::Array(list);
        write_manifest(&game.join("vortex.deployment.json"), &v, staging.as_deref())?;
    }
    Ok(())
}

fn write_manifest(path: &Path, v: &Value, staging: Option<&Path>) -> Result<()> {
    let text = serde_json::to_string_pretty(v).unwrap_or_default();
    let tmp = path.with_extension("json.pocket-tmp");
    fs::write(&tmp, &text)?;
    fs::rename(&tmp, path)?;
    if let Some(st) = staging {
        // keep Vortex's backups in step; the binary one would otherwise win over our JSON if the main file broke
        if let Some(name) = path.file_name() {
            let _ = fs::write(st.join(name), &text);
            let bin = st.join(name.to_string_lossy().replace(".json", ".msgpack"));
            if bin.exists() { let _ = fs::remove_file(bin); }
        }
    }
    Ok(())
}

/// Update a tracked Vortex mod from a new archive, keeping it in Vortex's format: the files are replaced inside its
/// existing staging folder (same name, so Vortex keeps its record), relinked into the game, and the manifest updated.
/// The archive is also put in Vortex's downloads folder so Vortex lists it.
pub fn update_in_place(db: &Db, game: &Path, library: &Path, setup: &Setup, install_id: i64, archive_path: &Path, archive_name: &str, new_file_id: u32, new_version: &str) -> Result<DeployOutcome> {
    let (folder, name): (String, String) = db.conn.query_row("SELECT external, name FROM installs WHERE id = ?1 AND source = 'vortex'", params![install_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let staging_root = setup.staging.clone().ok_or_else(|| Error::Invalid("Vortex's staging folder was not found, so the update cannot be kept in Vortex's format.".into()))?;
    let dir = staging_root.join(&folder);
    let entries = archive::list(archive_path)?;
    let plan = plan_install(&entries, &name);
    if plan.files.is_empty() { return Err(Error::Invalid("Nothing in the new archive belongs in the game folder.".into())); }

    // build the new folder beside the old one, then swap, so a failure leaves Vortex's folder as it was
    let fresh = staging_root.join(format!("__pocket_{folder}"));
    let old = staging_root.join(format!("__pocket_old_{folder}"));
    let extracted = staging_root.join(format!("__pocket_x_{folder}"));
    for p in [&fresh, &old, &extracted] { let _ = fs::remove_dir_all(p); }
    archive::extract(archive_path, &extracted)?;
    for p in &plan.files {
        let to = fresh.join(&p.target);
        if let Some(parent) = to.parent() { fs::create_dir_all(parent)?; }
        fs::rename(extracted.join(&p.source), &to).or_else(|_| fs::copy(extracted.join(&p.source), &to).map(|_| ()))?;
    }
    let _ = fs::remove_dir_all(&extracted);

    deploy::undeploy(db, game, install_id)?;
    if dir.exists() { fs::rename(&dir, &old)?; }
    fs::rename(&fresh, &dir)?;
    let staged = install::staged_plan(&dir, &name);
    let vopts = DeployOptions { no_manifest: true, ..Default::default() };
    let outcome = deploy::deploy(db, game, library, install_id, &dir, &staged.files, &vopts);
    match &outcome {
        Ok(DeployOutcome::Deployed { .. }) => {
            let _ = fs::remove_dir_all(&old);
            db.conn.execute("UPDATE deployed_files SET method = 'vortex' WHERE install_id = ?1", params![install_id])?;
            let files: Vec<String> = staged.files.iter().map(|p| p.target.clone()).collect();
            rewrite_manifests(setup, game, &folder, &files)?;
            if let Some(dl) = &setup.downloads {
                let to = dl.join(crate::download::safe_file_name(archive_name));
                if !to.exists() { let _ = fs::hard_link(archive_path, &to).or_else(|_| fs::copy(archive_path, &to).map(|_| ())); }
            }
            let fws: Vec<&str> = staged.uses.iter().map(|f| crate::cyberpunk::frameworks::info(*f).key).collect();
            db.conn.execute("UPDATE installs SET file_id = ?2, version = ?3, installed_at = ?4, frameworks = ?5, mod_type = ?6 WHERE id = ?1",
                params![install_id, new_file_id, new_version, now(), serde_json::to_string(&fws).unwrap_or_default(), staged.mod_type.unwrap_or(ModType::Unknown).as_str()])?;
        }
        _ => {
            // put Vortex's folder back as it was and redeploy it
            let _ = fs::remove_dir_all(&dir);
            if old.exists() { fs::rename(&old, &dir)?; }
            let back = install::staged_plan(&dir, &name);
            let _ = deploy::deploy(db, game, library, install_id, &dir, &back.files, &DeployOptions { allow_copy: true, take_conflicts: false, no_manifest: true });
            db.conn.execute("UPDATE deployed_files SET method = 'vortex' WHERE install_id = ?1", params![install_id])?;
        }
    }
    outcome
}

/// Take a tracked Vortex mod out of the game (disable or uninstall), keeping Vortex's manifest in step. Its staging
/// folder stays unless `delete_staging`, so Vortex can still deploy it.
pub fn remove_from_game(db: &Db, game: &Path, setup: &Setup, install_id: i64, delete_staging: bool) -> Result<deploy::UndeployReport> {
    let (folder, staging): (String, String) = db.conn.query_row("SELECT external, staging FROM installs WHERE id = ?1 AND source = 'vortex'", params![install_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let rep = deploy::undeploy(db, game, install_id)?;
    rewrite_manifests(setup, game, &folder, &[])?;
    if delete_staging { let _ = fs::remove_dir_all(staging); }
    Ok(rep)
}

/// Put a tracked Vortex mod back in the game from its staging folder.
pub fn redeploy(db: &Db, game: &Path, library: &Path, setup: &Setup, install_id: i64, opts: &DeployOptions) -> Result<DeployOutcome> {
    let (folder, staging, name): (String, String, String) = db.conn.query_row("SELECT external, staging, name FROM installs WHERE id = ?1 AND source = 'vortex'", params![install_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let dir = PathBuf::from(staging);
    let plan = install::staged_plan(&dir, &name);
    let out = deploy::deploy(db, game, library, install_id, &dir, &plan.files, &DeployOptions { no_manifest: true, ..opts.clone() })?;
    if matches!(out, DeployOutcome::Deployed { .. }) {
        db.conn.execute("UPDATE deployed_files SET method = 'vortex' WHERE install_id = ?1", params![install_id])?;
        let files: Vec<String> = plan.files.iter().map(|p| p.target.clone()).collect();
        rewrite_manifests(setup, game, &folder, &files)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_names() {
        let c = parse_archive_name("ArchiveXL-4198-1-28-6-1761742782");
        assert_eq!(c[0], ArchiveName { name: "ArchiveXL".into(), mod_id: 4198, version: "1.28.6".into(), timestamp: 1761742782 });
        let c = parse_archive_name("Cyberpunk 2077 HD Reworked-7652-2-0-1700000000.1");
        assert_eq!((c[0].mod_id, c[0].version.as_str()), (7652, "2.0"));
        // a name with its own digits: every reading is offered, the first is the leftmost
        let c = parse_archive_name("Mod-2-Fix-555-1-0-1700000000");
        assert_eq!(c.iter().map(|x| x.mod_id).collect::<Vec<_>>(), [2, 555, 1, 0]);
        assert!(parse_archive_name("My hand-made mod").is_empty());
    }

    /// A fake Vortex setup: staging with two mods (one deployed by hard link, one disabled) and a manifest.
    fn fake(dir: &Path) -> (PathBuf, Setup) {
        let game = dir.join("game");
        let st = dir.join("Vortex/cyberpunk2077/mods");
        let a = st.join("Cool Jacket-500-1-0-1700000000");
        fs::create_dir_all(a.join("archive/pc/mod")).unwrap();
        fs::write(a.join("archive/pc/mod/jacket.archive"), b"v1").unwrap();
        fs::create_dir_all(st.join("Disabled Thing-501-2-0-1700000001/r6/scripts/X")).unwrap();
        fs::write(st.join("Disabled Thing-501-2-0-1700000001/r6/scripts/X/a.reds"), b"x").unwrap();
        fs::create_dir_all(game.join("archive/pc/mod")).unwrap();
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        fs::hard_link(a.join("archive/pc/mod/jacket.archive"), game.join("archive/pc/mod/jacket.archive")).unwrap();
        let m = json!({ "version": 1, "instance": "abc", "deploymentMethod": "hardlink_activator", "stagingPath": st.to_string_lossy(), "gameId": "cyberpunk2077",
            "files": [{ "relPath": "archive\\pc\\mod\\jacket.archive", "source": "Cool Jacket-500-1-0-1700000000", "target": "", "time": 1 },
                      { "relPath": "archive\\pc\\mod\\other.archive", "source": "Someone Else-9-1-0-1700000002", "target": "", "time": 1 }] });
        fs::write(game.join("vortex.deployment.json"), serde_json::to_string(&m).unwrap()).unwrap();
        (game.clone(), find(&game).unwrap())
    }

    #[test]
    fn reads_vortex_mods_and_tracks_them() {
        let dir = tempfile::tempdir().unwrap();
        let (game, setup) = fake(dir.path());
        assert_eq!(setup.staging.as_deref(), Some(dir.path().join("Vortex/cyberpunk2077/mods").as_path()));
        let mods = list_mods(&setup, &game);
        let jacket = mods.iter().find(|m| m.folder.starts_with("Cool Jacket")).unwrap();
        assert!(jacket.deployed);
        assert_eq!(jacket.files, ["archive/pc/mod/jacket.archive"]);
        assert!(!mods.iter().find(|m| m.folder.starts_with("Disabled")).unwrap().deployed);

        let db = Db::memory().unwrap();
        let resolved: Vec<Resolved> = mods.iter().map(|m| Resolved { vortex: m.clone(), mod_id: m.candidates.first().map(|c| c.mod_id), file_id: None,
            name: m.candidates.first().map(|c| c.name.clone()).unwrap_or_else(|| m.folder.clone()), version: m.candidates.first().map(|c| c.version.clone()).unwrap_or_default(), category: None }).collect();
        let rep = track(&db, &game, &setup, &resolved).unwrap();
        assert_eq!(rep.tracked, 3);
        assert_eq!(track(&db, &game, &setup, &resolved).unwrap().already, 3, "tracking twice changes nothing");
        assert!(deploy::verify(&db, &game).unwrap().iter().all(|(_, t)| t != "archive/pc/mod/jacket.archive"), "Vortex's hard link counts as the mod's file");
    }

    #[test]
    fn updates_in_place_and_keeps_the_manifest_in_step() {
        let dir = tempfile::tempdir().unwrap();
        let (game, setup) = fake(dir.path());
        let db = Db::memory().unwrap();
        let mods: Vec<Resolved> = list_mods(&setup, &game).into_iter().filter(|m| m.folder.starts_with("Cool")).map(|m| Resolved { mod_id: Some(500), file_id: Some(1), name: "Cool Jacket".into(), version: "1.0".into(), category: None, vortex: m }).collect();
        let id = track(&db, &game, &setup, &mods).unwrap().install_ids[0];

        let zip = dir.path().join("Cool Jacket-500-1-1-1700009999.zip");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(fs::File::create(&zip).unwrap());
            for (n, b) in [("jacket.archive", "v2"), ("jacket.archive.xl", "xl")] { z.start_file(n, zip::write::SimpleFileOptions::default()).unwrap(); z.write_all(b.as_bytes()).unwrap(); }
            z.finish().unwrap();
        }
        let lib = dir.path().join("PocketMods");
        let out = update_in_place(&db, &game, &lib, &setup, id, &zip, "Cool Jacket-500-1-1-1700009999.zip", 2, "1.1").unwrap();
        assert!(matches!(out, DeployOutcome::Deployed { .. }));
        let folder = setup.staging.as_ref().unwrap().join("Cool Jacket-500-1-0-1700000000");
        assert_eq!(fs::read_to_string(folder.join("archive/pc/mod/jacket.archive")).unwrap(), "v2", "same Vortex folder, new files");
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/jacket.archive")).unwrap(), "v2");
        assert!(same_file::is_same_file(game.join("archive/pc/mod/jacket.archive.xl"), folder.join("archive/pc/mod/jacket.archive.xl")).unwrap());

        let m = read_json(&game.join("vortex.deployment.json")).unwrap();
        assert_eq!(m["instance"], "abc", "Vortex's own fields are kept");
        let mine: Vec<&str> = m["files"].as_array().unwrap().iter().filter(|f| f["source"] == "Cool Jacket-500-1-0-1700000000").map(|f| f["relPath"].as_str().unwrap()).collect();
        assert_eq!(mine.len(), 2);
        assert!(m["files"].as_array().unwrap().iter().any(|f| f["source"] == "Someone Else-9-1-0-1700000002"), "other mods' entries stay");
        assert!(setup.staging.as_ref().unwrap().join("vortex.deployment.json").exists(), "backup kept in step");
        let v: String = db.conn.query_row("SELECT version FROM installs WHERE id = ?1", params![id], |r| r.get(0)).unwrap();
        assert_eq!(v, "1.1");

        // disable from Pocket Mods: out of the game and out of the manifest; Vortex's folder stays
        remove_from_game(&db, &game, &setup, id, false).unwrap();
        assert!(!game.join("archive/pc/mod/jacket.archive").exists());
        assert!(folder.exists());
        let m = read_json(&game.join("vortex.deployment.json")).unwrap();
        assert!(!m["files"].as_array().unwrap().iter().any(|f| f["source"] == "Cool Jacket-500-1-0-1700000000"));
        redeploy(&db, &game, &lib, &setup, id, &DeployOptions::default()).unwrap();
        assert!(game.join("archive/pc/mod/jacket.archive").exists());
        assert!(!folder.join(".pocket-manifest.json").exists(), "nothing of ours in Vortex's folder");
        assert!(!setup.staging.as_ref().unwrap().join("backup").exists());
        // uninstalling from Pocket Mods leaves the mod in Vortex
        crate::install::uninstall(&db, &game, id).unwrap();
        assert!(folder.join("archive/pc/mod/jacket.archive").exists(), "Vortex's folder is never deleted");
        assert!(!game.join("archive/pc/mod/jacket.archive").exists());
    }
}

/// Match Vortex's mods to Nexus: each reading of the folder name is tried until the mod's file list has a file
/// whose archive name is the folder. Six lookups at a time; each is cached like any mod detail.
pub async fn resolve(client: &crate::nexus::NexusClient, db: &std::sync::Mutex<Db>, mods: Vec<VortexMod>) -> Vec<Resolved> {
    use futures_util::stream::{self, StreamExt};
    stream::iter(mods.into_iter().map(|m| async move {
        for c in &m.candidates {
            let Ok(d) = crate::browse::detail(client, db, c.mod_id, None).await else { continue };
            let stem = |u: &str| u.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_else(|| u.to_string());
            let base = m.folder.rsplit_once('.').filter(|(_, n)| n.chars().all(|x| x.is_ascii_digit())).map(|(a, _)| a).unwrap_or(&m.folder);
            if let Some(f) = d.files.iter().find(|f| stem(&f.uri) == base || stem(&f.uri) == m.folder) {
                return Resolved { name: d.detail.name.clone(), version: f.version.clone(), category: Some(d.detail.category.clone()), mod_id: Some(c.mod_id), file_id: Some(f.file_id), vortex: m.clone() };
            }
            // the mod matches by name but the file was removed from Nexus: keep the mod, not the file
            if d.detail.name.to_lowercase().replace(' ', "") .starts_with(&c.name.to_lowercase().replace(' ', "").chars().take(6).collect::<String>()) {
                return Resolved { name: d.detail.name.clone(), version: c.version.clone(), category: Some(d.detail.category.clone()), mod_id: Some(c.mod_id), file_id: None, vortex: m.clone() };
            }
        }
        let first = m.candidates.first();
        Resolved { name: first.map(|c| c.name.clone()).unwrap_or_else(|| m.folder.clone()), version: first.map(|c| c.version.clone()).unwrap_or_default(), category: None, mod_id: None, file_id: None, vortex: m }
    })).buffer_unordered(6).collect().await
}
