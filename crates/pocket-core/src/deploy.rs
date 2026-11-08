//! Deploy and undeploy: put a mod's staged files into the game folder with hard links, and take them out again.
//!
//! Rules (spec section 5):
//! - Mod storage is on the same drive as the game and files are hard-linked, never symlinked (symlinks need admin
//!   or Developer Mode). A copy is used only when the caller has asked for the copy fallback.
//! - Every deploy is reversible. `deployed_files` is the manifest: one row per path written, its owner, how it was
//!   written, and what it replaced (a backup of a file that was there, or the install that owned the path before).
//! - A file another mod owns is never deleted. Paths owned by another install are a conflict the user decides.
//! - If a deploy fails halfway, what it already wrote is undone.

use crate::cyberpunk::layout::Placement;
use crate::db::{now, Db};
use crate::error::{Error, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method { Hardlink, Copy }

impl Method {
    fn as_str(&self) -> &'static str { match self { Method::Hardlink => "hardlink", Method::Copy => "copy" } }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeployOptions {
    /// Copy files when a hard link is impossible (another drive or a file system without links).
    pub allow_copy: bool,
    /// Take paths another install owns. The other install gets them back when this one is removed.
    pub take_conflicts: bool,
    /// Do not write `.pocket-manifest.json` into the staging folder (a Vortex mod folder: Vortex would deploy it).
    #[serde(skip)]
    pub no_manifest: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict { pub target: String, pub owner_install: i64, pub owner_name: String }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "outcome")]
pub enum DeployOutcome {
    Deployed { linked: usize, copied: usize, backed_up: Vec<String>, took_from: Vec<Conflict> },
    /// Nothing was written: these paths belong to other mods.
    Conflicts { conflicts: Vec<Conflict> },
    /// Nothing was written: hard links are impossible here, and copying was not allowed.
    NeedsCopy { reason: String },
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UndeployReport {
    pub removed: usize,
    /// Files put back: backups of what was there before, or another mod's files.
    pub restored: Vec<String>,
    /// Files changed since deploy (not ours any more), left where they are.
    pub left_alone: Vec<String>,
}

/// Game folders the deployer never removes, even when empty.
const KEEP_DIRS: &[&str] = &["archive", "archive/pc", "archive/pc/mod", "archive/pc/content", "archive/pc/ep1", "bin", "bin/x64", "bin/x64/plugins",
    "bin/x64/plugins/cyber_engine_tweaks", "bin/x64/plugins/cyber_engine_tweaks/mods", "engine", "engine/config", "engine/tools", "mods", "r6",
    "r6/scripts", "r6/tweaks", "r6/config", "r6/cache", "red4ext", "red4ext/plugins", "tools", "tools/redmod"];

fn key(target: &str) -> String { target.replace('\\', "/").to_lowercase() }

fn in_program_files(p: &Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    s.contains("program files")
}

fn permission_error(target: &Path, game: &Path) -> Error {
    if in_program_files(game) {
        Error::Invalid(format!(
            "Windows did not allow writing to {}. The game is under Program Files, where normal users cannot write. \
             Fix: in Steam, move the game to another library folder (Properties, Installed files, Move install folder), \
             or give your account Modify rights on the game folder.", target.display()))
    } else {
        Error::Invalid(format!("Windows blocked writing to {}. Close the game and any tool using it, then try again", target.display()))
    }
}

/// True when both paths are on the same volume, so hard links can work.
pub fn same_volume(a: &Path, b: &Path) -> bool {
    let a = existing_ancestor(a);
    let b = existing_ancestor(b);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((fs::metadata(&a), fs::metadata(&b)), (Ok(x), Ok(y)) if x.dev() == y.dev())
    }
    #[cfg(windows)]
    {
        // drive letter or UNC share; mounted folders inside a drive are not detected (the link attempt still decides)
        let root = |p: &Path| p.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase());
        root(&a).is_some() && root(&a) == root(&b)
    }
}

fn existing_ancestor(p: &Path) -> PathBuf {
    let mut cur = p.to_path_buf();
    while !cur.exists() { if !cur.pop() { break; } }
    fs::canonicalize(&cur).unwrap_or(cur)
}

struct Row { target_key: String, target_path: String, source: String, method: String, backup: Option<String>, prev_install: Option<i64>, size: u64,
    /// The file was already this very file (a hard link left by Vortex): a rollback must leave it in place.
    in_place: bool }

fn owner_of(db: &Db, target_key: &str) -> Result<Option<(i64, String)>> {
    Ok(db.conn.query_row(
        "SELECT d.install_id, COALESCE(i.name, '') FROM deployed_files d LEFT JOIN installs i ON i.id = d.install_id WHERE d.target = ?1",
        params![target_key], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
}

/// Deploy one install's staged files. `plan` is from `layout::plan_install`; sources are relative to `staging`.
/// The user's standing choices (`rules`) apply: archive renames change targets, and a remembered "Take them over"
/// settles a clash without asking (this mod takes the paths, or stays out of them while the winner is there).
pub fn deploy(db: &Db, game: &Path, library: &Path, install_id: i64, staging: &Path, plan: &[Placement], opts: &DeployOptions) -> Result<DeployOutcome> {
    // every file stays inside the game folder and comes from inside the staging folder, whatever wrote the plan
    if let Some(bad) = plan.iter().find(|p| !crate::cyberpunk::layout::safe_target(&p.target) || !crate::cyberpunk::layout::safe_target(&p.source)) {
        return Err(Error::Invalid(format!("This mod tried to put a file outside the game folder ({}). Nothing was installed", bad.target)));
    }
    let plan = &crate::rules::apply_renames(db, install_id, plan)?[..];
    // 1. conflicts with other installs
    let mut conflicts = Vec::new();
    let mut settled = Vec::new();
    let mut shadowed: std::collections::BTreeSet<String> = Default::default();
    for p in plan {
        if let Some((owner, name)) = owner_of(db, &key(&p.target))? {
            if owner == install_id { continue; }
            let c = Conflict { target: p.target.clone(), owner_install: owner, owner_name: name };
            match crate::rules::verdict(db, install_id, owner)? {
                Some(crate::rules::Verdict::Wins) => settled.push(c),
                Some(crate::rules::Verdict::Loses) => { shadowed.insert(key(&p.target)); }
                None => conflicts.push(c),
            }
        }
    }
    if !conflicts.is_empty() && !opts.take_conflicts { return Ok(DeployOutcome::Conflicts { conflicts }); }

    // 2. hard links need one volume
    if !same_volume(staging, game) && !opts.allow_copy {
        return Ok(DeployOutcome::NeedsCopy { reason: "The library is on a different drive from the game, so files can't be hard-linked".into() });
    }

    let backup_root = library.join("backup");
    let mut done: Vec<Row> = Vec::new();
    let (mut linked, mut copied) = (0, 0);
    let mut backed_up = Vec::new();
    let result: Result<()> = (|| {
        for p in plan {
            let src = staging.join(&p.source);
            let dst = game.join(&p.target);
            let tkey = key(&p.target);
            if shadowed.contains(&tkey) {
                // the user chose the other mod's copy: stay out, and get the path back when that mod goes
                db.conn.execute("UPDATE deployed_files SET prev_install = ?2 WHERE target = ?1 AND prev_install IS NULL", params![tkey, install_id])?;
                continue;
            }
            let size = fs::metadata(&src).map_err(|e| if e.kind() == ErrorKind::NotFound {
                Error::Invalid(format!("{} is missing from the mod's folder in the library. Install the mod again", p.source))
            } else { e.into() })?.len();
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| if e.kind() == ErrorKind::PermissionDenied { permission_error(parent, game) } else { e.into() })?;
            }
            let mut backup = None;
            let mut prev_install = None;
            let mut in_place = false;
            match owner_of(db, &tkey)? {
                Some((owner, _)) if owner == install_id => {
                    // ours on record, but changed since (edited in place, or replaced): keep that copy aside, never delete it
                    let changed = dst.exists() && !same_file::is_same_file(&src, &dst).unwrap_or(false)
                        && fs::metadata(&dst).map(|m| m.len()).ok() != Some(size);
                    if changed {
                        let b = backup_root.join("changed").join(&p.target);
                        if let Some(parent) = b.parent() { fs::create_dir_all(parent)?; }
                        let _ = fs::remove_file(&b);
                        move_file(&dst, &b)?;
                        backed_up.push(p.target.clone());
                    } else { remove_file(&dst, game)?; }
                }
                // already this very file (a hard link to the staged one, as Vortex leaves it): nothing to move
                None if dst.exists() && same_file::is_same_file(&src, &dst).unwrap_or(false) => { in_place = true; }
                Some((owner, _)) => { prev_install = Some(owner); remove_file(&dst, game)?; }
                None if dst.exists() => {
                    // a file no mod owns (the game's, or put there by hand): keep it to put back later
                    let b = backup_root.join(&p.target);
                    if let Some(parent) = b.parent() { fs::create_dir_all(parent)?; }
                    // an older copy kept from before: this one is newer (a game patch rewrote it), so it is the one to put back
                    if b.exists() { let _ = fs::remove_file(&b); }
                    move_file(&dst, &b)?;
                    backup = Some(b.to_string_lossy().into_owned());
                    backed_up.push(p.target.clone());
                }
                None => {}
            }
            let method = if in_place { linked += 1; Method::Hardlink } else { match fs::hard_link(&src, &dst) {
                Ok(()) => { linked += 1; Method::Hardlink }
                Err(e) if e.kind() == ErrorKind::PermissionDenied => return Err(permission_error(&dst, game)),
                Err(e) if opts.allow_copy => {
                    let _ = e;
                    fs::copy(&src, &dst).map_err(|e| if e.kind() == ErrorKind::PermissionDenied { permission_error(&dst, game) } else { e.into() })?;
                    copied += 1;
                    Method::Copy
                }
                Err(e) => return Err(Error::Invalid(format!("Could not hard-link {}: {e}. Turn on the copy fallback in Settings", p.target))),
            } };
            let row = Row { target_key: tkey, target_path: p.target.clone(), source: src.to_string_lossy().into_owned(), method: method.as_str().into(), backup, prev_install, size, in_place };
            db.conn.execute(
                "INSERT INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path, backup, prev_install) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(target) DO UPDATE SET install_id = ?2, source = ?3, method = ?4, size = ?5, deployed_at = ?6, target_path = ?7, backup = COALESCE(?8, backup), prev_install = ?9",
                params![row.target_key, install_id, row.source, row.method, row.size as i64, now(), row.target_path, row.backup, row.prev_install])?;
            done.push(row);
        }
        Ok(())
    })();

    if let Err(e) = result {
        // undo this run: remove what was linked, put back what was moved or taken
        for r in done.iter().rev() { let _ = undo_row(db, game, r); }
        for t in &shadowed { let _ = db.conn.execute("UPDATE deployed_files SET prev_install = NULL WHERE target = ?1 AND prev_install = ?2", params![t, install_id]); }
        return Err(e);
    }
    if !opts.no_manifest { write_manifest(staging, db, install_id)?; }
    // "Take them over" is remembered, so the next deploy, profile switch or update does not ask again
    if opts.take_conflicts {
        let mut by: std::collections::BTreeMap<i64, usize> = Default::default();
        for c in &conflicts { *by.entry(c.owner_install).or_default() += 1; }
        for (owner, n) in by { crate::rules::remember_win(db, install_id, owner, n)?; }
    }
    let mut took_from = conflicts;
    took_from.extend(settled);
    Ok(DeployOutcome::Deployed { linked, copied, backed_up, took_from })
}

fn undo_row(db: &Db, game: &Path, r: &Row) -> Result<()> {
    let dst = game.join(&r.target_path);
    if !r.in_place { let _ = fs::remove_file(&dst); }
    db.conn.execute("DELETE FROM deployed_files WHERE target = ?1", params![r.target_key])?;
    if let Some(b) = &r.backup { let _ = move_file(Path::new(b), &dst); }
    if let Some(prev) = r.prev_install { relink_previous(db, game, &r.target_key, &r.target_path, prev)?; }
    Ok(())
}

/// Give a path back to the install that owned it before, from that install's staging folder.
fn relink_previous(db: &Db, game: &Path, target_key: &str, target_path: &str, prev: i64) -> Result<bool> {
    let row: Option<(String, String)> = db.conn.query_row("SELECT staging, name FROM installs WHERE id = ?1", params![prev], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let Some((staging, name)) = row else { return Ok(false) };
    // the previous install's own source path for this target is in its manifest; a mod that stayed out of the path
    // (another mod's copy was chosen) never had it there, so its plan says which staged file it is
    let source = manifest_source(Path::new(&staging), target_key).or_else(|| {
        let plan = crate::install::staged_plan(Path::new(&staging), &name);
        let plan = crate::rules::apply_renames(db, prev, &plan.files).ok()?;
        plan.into_iter().find(|p| key(&p.target) == target_key).map(|p| Path::new(&staging).join(p.source))
    });
    let Some(src) = source.filter(|s| s.exists()) else { return Ok(false) };
    let dst = game.join(target_path);
    if let Some(parent) = dst.parent() { fs::create_dir_all(parent)?; }
    let method = if fs::hard_link(&src, &dst).is_ok() { "hardlink" } else { fs::copy(&src, &dst)?; "copy" };
    let size = fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
    db.conn.execute(
        "INSERT INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path, backup, prev_install) VALUES (?1, ?2, ?3, ?7, ?4, ?5, ?6, NULL, NULL)
         ON CONFLICT(target) DO UPDATE SET install_id = ?2, source = ?3, method = ?7, size = ?4, deployed_at = ?5, prev_install = NULL",
        params![target_key, prev, src.to_string_lossy(), size as i64, now(), target_path, method])?;
    Ok(true)
}

/// Take an install's files out of the game folder.
pub fn undeploy(db: &Db, game: &Path, install_id: i64) -> Result<UndeployReport> {
    let rows: Vec<Row> = {
        let mut st = db.conn.prepare("SELECT target, target_path, source, method, backup, prev_install, size FROM deployed_files WHERE install_id = ?1")?;
        let it = st.query_map(params![install_id], |r| Ok(Row { target_key: r.get(0)?, target_path: r.get(1)?, source: r.get(2)?, method: r.get(3)?, backup: r.get(4)?, prev_install: r.get(5)?, size: r.get::<_, i64>(6)? as u64, in_place: false }))?;
        it.collect::<std::result::Result<_, _>>()?
    };
    let mut rep = UndeployReport::default();
    let mut dirs = Vec::new();
    for r in &rows {
        let dst = game.join(&r.target_path);
        let ours = match fs::metadata(&dst) {
            Err(_) => true, // already gone
            Ok(m) => still_ours(&dst, &r.source, &r.method, m.len(), r.size),
        };
        if !ours {
            rep.left_alone.push(r.target_path.clone());
            db.conn.execute("DELETE FROM deployed_files WHERE target = ?1", params![r.target_key])?;
            continue;
        }
        if dst.exists() { remove_file(&dst, game)?; rep.removed += 1; }
        db.conn.execute("DELETE FROM deployed_files WHERE target = ?1", params![r.target_key])?;
        if let Some(prev) = r.prev_install {
            if relink_previous(db, game, &r.target_key, &r.target_path, prev)? {
                // the game's own file this path replaced is now the other mod's to put back
                if let Some(b) = &r.backup { db.conn.execute("UPDATE deployed_files SET backup = ?2 WHERE target = ?1", params![r.target_key, b])?; }
                rep.restored.push(r.target_path.clone());
                continue;
            }
        }
        if let Some(b) = &r.backup {
            if Path::new(b).exists() { move_file(Path::new(b), &dst)?; rep.restored.push(r.target_path.clone()); continue; }
        }
        if let Some(parent) = dst.parent() { dirs.push(parent.to_path_buf()); }
    }
    for d in dirs { prune_dirs(game, &d); }
    // this mod is out of the game now: paths other mods hold are no longer to be given back to it
    db.conn.execute("UPDATE deployed_files SET prev_install = NULL WHERE prev_install = ?1", params![install_id])?;
    Ok(rep)
}

/// Remove empty folders from `dir` up towards the game folder, keeping the game's own folders.
fn prune_dirs(game: &Path, dir: &Path) {
    let mut cur = dir.to_path_buf();
    while cur.starts_with(game) && cur != game {
        let rel = cur.strip_prefix(game).map(|r| r.to_string_lossy().replace('\\', "/").to_lowercase()).unwrap_or_default();
        if KEEP_DIRS.contains(&rel.as_str()) { break; }
        if fs::remove_dir(&cur).is_err() { break; } // not empty
        if !cur.pop() { break; }
    }
}

fn remove_file(p: &Path, game: &Path) -> Result<()> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Err(permission_error(p, game)),
        // ERROR_SHARING_VIOLATION: open in the game or a tool
        Err(e) if e.raw_os_error() == Some(32) => Err(Error::Invalid(format!("{} is in use. Close the game and any tool using it, then try again", p.display()))),
        Err(e) => Err(e.into()),
    }
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() { fs::create_dir_all(parent)?; }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => { fs::copy(from, to)?; fs::remove_file(from)?; Ok(()) }
    }
}

pub const MANIFEST: &str = ".pocket-manifest.json";

/// Write an install's manifest again from the database (after its paths changed).
pub fn refresh_manifest(staging: &Path, db: &Db, install_id: i64) -> Result<()> { write_manifest(staging, db, install_id) }

/// A copy of the install's manifest rows next to its staged files, so a lost database can be rebuilt.
fn write_manifest(staging: &Path, db: &Db, install_id: i64) -> Result<()> {
    let mut st = db.conn.prepare("SELECT target, target_path, source, method, backup, prev_install FROM deployed_files WHERE install_id = ?1 ORDER BY target")?;
    let rows: Vec<serde_json::Value> = st.query_map(params![install_id], |r| Ok(serde_json::json!({
        "target": r.get::<_, String>(0)?, "targetPath": r.get::<_, String>(1)?, "source": r.get::<_, String>(2)?, "method": r.get::<_, String>(3)?,
        "backup": r.get::<_, Option<String>>(4)?, "prevInstall": r.get::<_, Option<i64>>(5)?,
    })))?.collect::<std::result::Result<_, _>>()?;
    fs::write(staging.join(MANIFEST), serde_json::to_string_pretty(&serde_json::json!({ "installId": install_id, "files": rows })).unwrap_or_default())?;
    Ok(())
}

fn manifest_source(staging: &Path, target_key: &str) -> Option<PathBuf> {
    let text = fs::read_to_string(staging.join(MANIFEST)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    // only a file inside this staging folder: the manifest says where a file came from, it cannot send us elsewhere
    v["files"].as_array()?.iter().find(|f| f["target"] == target_key).and_then(|f| f["source"].as_str()).map(PathBuf::from)
        .filter(|p| p.starts_with(staging) && !p.components().any(|c| matches!(c, std::path::Component::ParentDir)))
}

/// Is the file in the game still the one the mod put there? Hard links must be the same file; copies the same size;
/// Vortex's files (hard links by default, copies or moves otherwise) either.
fn still_ours(dst: &Path, source: &str, method: &str, len: u64, size: u64) -> bool {
    match method {
        "hardlink" => same_file::is_same_file(dst, source).unwrap_or(false),
        "vortex" => same_file::is_same_file(dst, source).unwrap_or(false) || len == size,
        _ => len == size,
    }
}

/// Deployed files that are missing or no longer the mod's (for the health panel).
pub fn verify(db: &Db, game: &Path) -> Result<Vec<(i64, String)>> {
    let mut st = db.conn.prepare("SELECT install_id, target_path, source, method, size FROM deployed_files")?;
    let rows: Vec<(i64, String, String, String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<std::result::Result<_, _>>()?;
    Ok(rows.into_iter().filter(|(_, t, src, method, size)| {
        let dst = game.join(t);
        match fs::metadata(&dst) {
            Err(_) => true,
            Ok(m) => !still_ours(&dst, src, method, m.len(), *size as u64),
        }
    }).map(|(i, t, ..)| (i, t)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cyberpunk::layout::plan_install;

    struct Env { _dir: tempfile::TempDir, game: PathBuf, lib: PathBuf, db: Db }
    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Cyberpunk 2077");
        let lib = dir.path().join("PocketMods");
        fs::create_dir_all(game.join("archive/pc/mod")).unwrap();
        fs::create_dir_all(&lib).unwrap();
        Env { game, lib, db: Db::memory().unwrap(), _dir: dir }
    }
    fn stage(e: &Env, id: i64, name: &str, files: &[(&str, &str)]) -> (PathBuf, Vec<Placement>) {
        let st = e.lib.join("staging").join(id.to_string());
        for (p, body) in files { let f = st.join(p); fs::create_dir_all(f.parent().unwrap()).unwrap(); fs::write(f, body).unwrap(); }
        e.db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?1, ?1, ?2, '1', 'archive', '[]', ?3, 0)",
            params![id, name, st.to_string_lossy()]).unwrap();
        let list: Vec<String> = files.iter().map(|(p, _)| p.to_string()).collect();
        (st, plan_install(&list, name).files)
    }

    #[test]
    fn deploys_with_hard_links_and_removes_cleanly() {
        let e = env();
        let (st, plan) = stage(&e, 1, "Jacket", &[("Jacket/archive/pc/mod/jacket.archive", "J"), ("Jacket/r6/scripts/Jacket/j.reds", "R")]);
        let out = deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        assert!(matches!(out, DeployOutcome::Deployed { linked: 2, copied: 0, .. }));
        let dst = e.game.join("archive/pc/mod/jacket.archive");
        assert!(same_file::is_same_file(&dst, st.join("Jacket/archive/pc/mod/jacket.archive")).unwrap());
        assert!(st.join(MANIFEST).exists());
        assert!(verify(&e.db, &e.game).unwrap().is_empty());

        let rep = undeploy(&e.db, &e.game, 1).unwrap();
        assert_eq!(rep.removed, 2);
        assert!(!dst.exists());
        assert!(e.game.join("archive/pc/mod").exists(), "game folders stay");
        assert!(!e.game.join("r6/scripts/Jacket").exists(), "the mod's own folder goes");
        assert!(e.game.join("r6/scripts").exists());
    }

    #[test]
    fn a_vanilla_file_is_backed_up_and_put_back() {
        let e = env();
        fs::create_dir_all(e.game.join("r6/config")).unwrap();
        fs::write(e.game.join("r6/config/inputUserMappings.xml"), "vanilla").unwrap();
        let (st, plan) = stage(&e, 1, "Keys", &[("r6/config/inputUserMappings.xml", "modded")]);
        let out = deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        assert!(matches!(&out, DeployOutcome::Deployed { backed_up, .. } if backed_up.len() == 1));
        assert_eq!(fs::read_to_string(e.game.join("r6/config/inputUserMappings.xml")).unwrap(), "modded");
        let rep = undeploy(&e.db, &e.game, 1).unwrap();
        assert_eq!(rep.restored, ["r6/config/inputUserMappings.xml"]);
        assert_eq!(fs::read_to_string(e.game.join("r6/config/inputUserMappings.xml")).unwrap(), "vanilla");
    }

    #[test]
    fn conflicts_need_a_decision_and_the_loser_gets_its_file_back() {
        let e = env();
        let (st1, p1) = stage(&e, 1, "A", &[("shared.archive", "A")]);
        let (st2, p2) = stage(&e, 2, "B", &[("shared.archive", "B")]);
        deploy(&e.db, &e.game, &e.lib, 1, &st1, &p1, &DeployOptions::default()).unwrap();
        let out = deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &DeployOptions::default()).unwrap();
        match out { DeployOutcome::Conflicts { conflicts } => assert_eq!(conflicts[0].owner_name, "A"), o => panic!("{o:?}") }
        let target = e.game.join("archive/pc/mod/shared.archive");
        assert_eq!(fs::read_to_string(&target).unwrap(), "A", "nothing written on conflict");

        deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &DeployOptions { take_conflicts: true, ..Default::default() }).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "B");
        let rep = undeploy(&e.db, &e.game, 2).unwrap();
        assert_eq!(rep.restored.len(), 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "A", "A owns it again");
        assert_eq!(owner_of(&e.db, "archive/pc/mod/shared.archive").unwrap().unwrap().0, 1);
    }

    #[test]
    fn take_them_over_is_remembered_whichever_mod_goes_in_first() {
        let e = env();
        let (st1, p1) = stage(&e, 1, "A", &[("shared.archive", "A"), ("a_only.archive", "a")]);
        let (st2, p2) = stage(&e, 2, "B", &[("shared.archive", "B")]);
        let target = e.game.join("archive/pc/mod/shared.archive");
        let opts = DeployOptions::default();
        deploy(&e.db, &e.game, &e.lib, 1, &st1, &p1, &opts).unwrap();
        deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &DeployOptions { take_conflicts: true, ..Default::default() }).unwrap();
        assert_eq!(crate::rules::list(&e.db).unwrap().wins.len(), 1);

        // both out (a profile without them), then in again in the same order: no question this time
        undeploy(&e.db, &e.game, 2).unwrap();
        undeploy(&e.db, &e.game, 1).unwrap();
        deploy(&e.db, &e.game, &e.lib, 1, &st1, &p1, &opts).unwrap();
        let out = deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &opts).unwrap();
        assert!(matches!(&out, DeployOutcome::Deployed { took_from, .. } if took_from.len() == 1), "{out:?}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "B");

        // the other order: B first, then A goes in and stays out of B's path
        undeploy(&e.db, &e.game, 2).unwrap();
        undeploy(&e.db, &e.game, 1).unwrap();
        deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &opts).unwrap();
        let out = deploy(&e.db, &e.game, &e.lib, 1, &st1, &p1, &opts).unwrap();
        assert!(matches!(out, DeployOutcome::Deployed { linked: 1, .. }), "{out:?}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "B");
        assert!(e.game.join("archive/pc/mod/a_only.archive").exists());
        // B goes: A gets the path it stayed out of
        undeploy(&e.db, &e.game, 2).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "A");
        assert_eq!(owner_of(&e.db, "archive/pc/mod/shared.archive").unwrap().unwrap().0, 1);

        // a mod that left the game is not handed paths later
        deploy(&e.db, &e.game, &e.lib, 2, &st2, &p2, &opts).unwrap();
        undeploy(&e.db, &e.game, 1).unwrap();
        undeploy(&e.db, &e.game, 2).unwrap();
        assert!(!target.exists(), "A was off, so nothing comes back for it");
    }

    #[test]
    fn a_renamed_archive_is_linked_under_its_new_name_and_back() {
        let e = env();
        let (st, plan) = stage(&e, 1, "Coat", &[("coat.archive", "C")]);
        crate::rules::set_rename(&e.db, 1, "archive/pc/mod/coat.archive", "!coat.archive").unwrap();
        deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        assert!(e.game.join("archive/pc/mod/!coat.archive").exists() && !e.game.join("archive/pc/mod/coat.archive").exists());
        undeploy(&e.db, &e.game, 1).unwrap();
        crate::rules::clear_rename(&e.db, 1, "archive/pc/mod/coat.archive").unwrap();
        deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        assert!(e.game.join("archive/pc/mod/coat.archive").exists() && !e.game.join("archive/pc/mod/!coat.archive").exists());
    }

    #[test]
    fn a_file_changed_by_hand_is_left_alone() {
        let e = env();
        let (st, plan) = stage(&e, 1, "A", &[("a.archive", "A")]);
        deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        let t = e.game.join("archive/pc/mod/a.archive");
        fs::remove_file(&t).unwrap();
        fs::write(&t, "user's own").unwrap();
        assert_eq!(verify(&e.db, &e.game).unwrap().len(), 1);
        let rep = undeploy(&e.db, &e.game, 1).unwrap();
        assert_eq!(rep.left_alone, ["archive/pc/mod/a.archive"]);
        assert_eq!(fs::read_to_string(&t).unwrap(), "user's own");
    }

    #[test]
    fn a_failed_deploy_is_rolled_back() {
        let e = env();
        let (st, mut plan) = stage(&e, 1, "A", &[("a.archive", "A")]);
        plan.push(Placement { source: "missing.archive".into(), target: "archive/pc/mod/missing.archive".into(), kind: crate::cyberpunk::layout::FileKind::Archive });
        assert!(deploy(&e.db, &e.game, &e.lib, 1, &st, &plan, &DeployOptions::default()).is_err());
        assert!(!e.game.join("archive/pc/mod/a.archive").exists());
        let n: i64 = e.db.conn.query_row("SELECT COUNT(*) FROM deployed_files", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }
}
