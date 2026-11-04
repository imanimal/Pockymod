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
}

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
}

pub fn staging_dir(library: &Path, mod_id: u32, file_id: u32) -> PathBuf { library.join("staging").join(format!("{mod_id}-{file_id}")) }

/// The plan for files already in a staging folder.
pub fn staged_plan(staging: &Path, name: &str) -> InstallPlan {
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
    let plan = plan_install(&entries, &req.name);
    if plan.files.is_empty() { return Err(Error::Invalid("Nothing in this archive belongs in the game folder. It may need a manual install; see the mod's page.".into())); }
    if !plan.unknown.is_empty() && !req.accept_unknown { return Ok(InstallOutcome::NeedsReview { plan }); }

    // extract to a fresh folder, then swap it in
    let staging = staging_dir(library, req.mod_id, req.file_id);
    let fresh = staging.with_extension("new");
    let _ = fs::remove_dir_all(&fresh);
    archive::extract(&req.archive, &fresh)?;

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
    add_to_active_profile(db, install_id)?;

    // an update: take the older file of the same mod out first, put it back if the new one fails
    let older: Vec<i64> = if req.replace_older {
        let mut st = db.conn.prepare("SELECT id FROM installs WHERE mod_id = ?1 AND id != ?2")?;
        let ids = st.query_map(params![req.mod_id, install_id], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
        ids
    } else { Vec::new() };
    for id in &older { deploy::undeploy(db, game, *id)?; }

    let outcome = deploy::deploy(db, game, library, install_id, &staging, &plan.files, &req.options);
    match outcome {
        Ok(d @ DeployOutcome::Deployed { .. }) => {
            for id in &older { remove_install_record(db, *id)?; }
            Ok(InstallOutcome::Installed { install_id, mod_type, files: plan.files.len(), needs_redmod_deploy: plan.needs_redmod_deploy, deploy: d, replaced: older, warnings: plan.warnings })
        }
        other => {
            for id in &older { let _ = redeploy(db, game, library, *id, &DeployOptions { allow_copy: req.options.allow_copy, take_conflicts: false }); }
            match other {
                Ok(d) => Ok(InstallOutcome::Staged { install_id, deploy: d }),
                Err(e) => Err(e),
            }
        }
    }
}

/// Deploy an install that is already staged (after a conflict or copy decision, or a profile switch).
pub fn redeploy(db: &Db, game: &Path, library: &Path, install_id: i64, opts: &DeployOptions) -> Result<DeployOutcome> {
    let (staging, name): (String, String) = db.conn.query_row("SELECT staging, name FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let plan = staged_plan(Path::new(&staging), &name);
    deploy::deploy(db, game, library, install_id, Path::new(&staging), &plan.files, opts)
}

pub fn uninstall(db: &Db, game: &Path, install_id: i64) -> Result<UndeployReport> {
    let rep = deploy::undeploy(db, game, install_id)?;
    let staging: Option<String> = db.conn.query_row("SELECT staging FROM installs WHERE id = ?1", params![install_id], |r| r.get(0)).optional()?;
    remove_install_record(db, install_id)?;
    if let Some(s) = staging { let _ = fs::remove_dir_all(s); }
    Ok(rep)
}

fn remove_install_record(db: &Db, install_id: i64) -> Result<()> {
    let staging: Option<String> = db.conn.query_row("SELECT staging FROM installs WHERE id = ?1", params![install_id], |r| r.get(0)).optional()?;
    db.conn.execute("DELETE FROM profile_mods WHERE install_id = ?1", params![install_id])?;
    db.conn.execute("DELETE FROM installs WHERE id = ?1", params![install_id])?;
    if let Some(s) = staging { let _ = fs::remove_dir_all(s); }
    Ok(())
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
                (SELECT COUNT(*) FROM deployed_files d WHERE d.install_id = i.id), COALESCE(pm.enabled, 0), COALESCE(pm.load_order, 9999)
         FROM installs i LEFT JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1 ORDER BY COALESCE(pm.load_order, 9999), i.name")?;
    let rows = st.query_map(params![p], |r| Ok(InstallRow {
        id: r.get(0)?, mod_id: r.get(1)?, file_id: r.get(2)?, name: r.get(3)?, version: r.get(4)?, mod_type: ModType::parse(&r.get::<_, String>(5)?),
        frameworks: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(), staging: r.get(7)?, installed_at: r.get(8)?, updated_at: r.get(9)?,
        deployed_files: r.get(10)?, enabled: r.get(11)?, load_order: r.get(12)?,
    }))?;
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
    fn install_update_and_uninstall() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        let db = Db::memory().unwrap();
        let a1 = dir.path().join("v1.zip");
        zip(&a1, &[("Mod/archive/pc/mod/m.archive", "v1"), ("Mod/readme.txt", "hi")]);
        let req = InstallRequest { mod_id: 5, file_id: 10, name: "My Mod".into(), version: "1".into(), archive: a1, accept_unknown: false, replace_older: true, options: Default::default() };
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
    fn unknown_files_ask_first() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let a = dir.path().join("x.zip");
        zip(&a, &[("a.archive", "x"), ("tool.exe", "x")]);
        let db = Db::memory().unwrap();
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default() };
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
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default() };
        assert!(install(&db, dir.path(), &dir.path().join("lib"), &req).is_err());
    }
}
