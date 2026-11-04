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
    /// The mod's author, from its cached Nexus record.
    pub author: Option<String>,
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
    let (plan, chosen) = if let Some(cfg) = &fomod_cfg {
        let _ = fs::remove_dir_all(&fresh);
        archive::extract(&req.archive, &fresh)?;
        let f = crate::fomod::parse(&crate::fomod::decode(&fs::read(fresh.join(cfg))?), cfg)?;
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
    add_to_active_profile(db, install_id)?;

    // an update: take the older file of the same mod out first, put it back if the new one fails
    let older: Vec<i64> = if req.replace_older {
        let mut st = db.conn.prepare("SELECT id FROM installs WHERE mod_id = ?1 AND id != ?2")?;
        let ids = st.query_map(params![req.mod_id, install_id], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
        ids
    } else { Vec::new() };
    for id in &older { undeploy_any(db, game, *id)?; }

    let outcome = deploy::deploy(db, game, library, install_id, &staging, &plan.files, &req.options);
    match outcome {
        Ok(d @ DeployOutcome::Deployed { .. }) => {
            for id in &older { remove_install_record(db, *id)?; }
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

/// Deploy an install that is already staged (after a conflict or copy decision, or a profile switch).
pub fn redeploy(db: &Db, game: &Path, library: &Path, install_id: i64, opts: &DeployOptions) -> Result<DeployOutcome> {
    let (staging, name): (String, String) = db.conn.query_row("SELECT staging, name FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let plan = staged_plan(Path::new(&staging), &name);
    deploy::deploy(db, game, library, install_id, Path::new(&staging), &plan.files, opts)
}

/// Remove a mod. A mod tracked from Vortex leaves the game and Vortex's manifest, but its folder in Vortex's
/// staging folder is kept (it is Vortex's), so it is still there in Vortex.
pub fn uninstall(db: &Db, game: &Path, install_id: i64) -> Result<UndeployReport> {
    let rep = undeploy_any(db, game, install_id)?;
    remove_install_record(db, install_id)?;
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

fn remove_install_record(db: &Db, install_id: i64) -> Result<()> {
    let row: Option<(String, String)> = db.conn.query_row("SELECT staging, source FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    db.conn.execute("DELETE FROM profile_mods WHERE install_id = ?1", params![install_id])?;
    db.conn.execute("DELETE FROM installs WHERE id = ?1", params![install_id])?;
    // never delete a Vortex mod folder: it belongs to Vortex
    if let Some((s, source)) = row { if source != "vortex" && !s.is_empty() { let _ = fs::remove_dir_all(s); } }
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
                (SELECT COUNT(*) FROM deployed_files d WHERE d.install_id = i.id), COALESCE(pm.enabled, 0), COALESCE(pm.load_order, 9999),
                i.source, i.category, i.group_name, (SELECT detail FROM mod_cache c WHERE c.mod_id = i.mod_id)
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
        let author: Option<String> = r.get::<_, Option<String>>(16).ok().flatten()
            .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok())
            .and_then(|v| v["author"].as_str().or(v["uploader"]["name"].as_str()).map(str::to_owned));
        let group = custom.clone().unwrap_or_else(|| preset_group(category.as_deref(), mod_type, &name).to_string());
        Ok(InstallRow {
            id: r.get(0)?, mod_id: r.get(1)?, file_id: r.get(2)?, version: r.get(4)?, mod_type,
            frameworks: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(), staging: r.get(7)?, installed_at: r.get(8)?, updated_at: r.get(9)?,
            deployed_files: r.get(10)?, enabled: r.get(11)?, load_order: r.get(12)?, source: r.get(13)?, category, group, custom_group: custom.is_some(), name, picture, author,
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
    fn install_update_and_uninstall() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("PocketMods");
        let db = Db::memory().unwrap();
        let a1 = dir.path().join("v1.zip");
        zip(&a1, &[("Mod/archive/pc/mod/m.archive", "v1"), ("Mod/readme.txt", "hi")]);
        let req = InstallRequest { mod_id: 5, file_id: 10, name: "My Mod".into(), version: "1".into(), archive: a1, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None };
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
        let req = InstallRequest { mod_id: 3, file_id: 3, name: "J".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None };
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
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None };
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
        let req = InstallRequest { mod_id: 1, file_id: 1, name: "X".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None };
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
