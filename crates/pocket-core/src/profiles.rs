//! Profiles (named sets of enabled mods) and the REDmod load order. Switching profile undeploys what the new one
//! turns off and deploys what it turns on, so the game folder always matches the active profile.

use crate::db::{now, Db};
use crate::deploy::{DeployOptions, DeployOutcome};
use crate::error::{Error, Result};
use crate::install::{self, staged_plan};
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile { pub id: i64, pub name: String, pub active: bool, pub enabled: i64 }

pub fn list(db: &Db) -> Result<Vec<Profile>> {
    let mut st = db.conn.prepare("SELECT p.id, p.name, p.active, (SELECT COUNT(*) FROM profile_mods m WHERE m.profile_id = p.id AND m.enabled = 1) FROM profiles p ORDER BY p.id")?;
    let rows = st.query_map([], |r| Ok(Profile { id: r.get(0)?, name: r.get(1)?, active: r.get(2)?, enabled: r.get(3)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// A new profile, a copy of another one (its mods, on/off and order) or empty.
pub fn create(db: &Db, name: &str, copy_from: Option<i64>) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() { return Err(Error::Invalid("A profile needs a name.".into())); }
    db.conn.execute("INSERT INTO profiles (name, active) VALUES (?1, 0)", params![name]).map_err(|_| Error::Invalid(format!("There is already a profile called {name}.")))?;
    let id = db.conn.last_insert_rowid();
    match copy_from {
        Some(src) => { db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) SELECT ?1, install_id, enabled, load_order FROM profile_mods WHERE profile_id = ?2", params![id, src])?; }
        None => { db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) SELECT ?1, id, 0, id FROM installs", params![id])?; }
    }
    Ok(id)
}

pub fn rename(db: &Db, id: i64, name: &str) -> Result<()> {
    db.conn.execute("UPDATE profiles SET name = ?2 WHERE id = ?1", params![id, name.trim()]).map_err(|_| Error::Invalid("That name is taken.".into()))?;
    Ok(())
}

pub fn delete(db: &Db, id: i64) -> Result<()> {
    let active: bool = db.conn.query_row("SELECT active FROM profiles WHERE id = ?1", params![id], |r| r.get(0))?;
    if active { return Err(Error::Invalid("Switch to another profile before deleting this one.".into())); }
    db.conn.execute("DELETE FROM profile_mods WHERE profile_id = ?1", params![id])?;
    db.conn.execute("DELETE FROM profiles WHERE id = ?1", params![id])?;
    Ok(())
}

fn enabled_set(db: &Db, profile: i64) -> Result<Vec<i64>> {
    let mut st = db.conn.prepare("SELECT install_id FROM profile_mods WHERE profile_id = ?1 AND enabled = 1 ORDER BY load_order")?;
    let ids = st.query_map(params![profile], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwitchReport { pub removed: Vec<i64>, pub deployed: Vec<i64>, pub not_deployed: Vec<(i64, String)> }

pub fn activate(db: &Db, game: &Path, library: &Path, id: i64, opts: &DeployOptions) -> Result<SwitchReport> {
    let old = install::active_profile(db)?;
    if old == id { return Ok(SwitchReport::default()); }
    let before = enabled_set(db, old)?;
    let after = enabled_set(db, id)?;
    let mut rep = SwitchReport::default();
    // one mod that fails must not leave the game half switched: carry on, and report it
    for i in before.iter().filter(|i| !after.contains(i)) {
        match install::undeploy_any(db, game, *i) { Ok(_) => rep.removed.push(*i), Err(e) => rep.not_deployed.push((*i, format!("could not be taken out: {e}"))) }
    }
    db.conn.execute("UPDATE profiles SET active = (id = ?1)", params![id])?;
    for i in after.iter().filter(|i| !before.contains(i)) {
        match install::redeploy_any(db, game, library, *i, opts) {
            Ok(DeployOutcome::Deployed { .. }) => rep.deployed.push(*i),
            Ok(DeployOutcome::Conflicts { conflicts }) => rep.not_deployed.push((*i, format!("{} files clash with other mods", conflicts.len()))),
            Ok(DeployOutcome::NeedsCopy { reason }) => rep.not_deployed.push((*i, reason)),
            Err(e) => rep.not_deployed.push((*i, e.to_string())),
        }
    }
    db.set_setting("loadorder.changedAt", &now().to_string())?;
    Ok(rep)
}

/// Turn a mod on or off in the active profile, deploying or undeploying it.
pub fn set_enabled(db: &Db, game: &Path, library: &Path, install_id: i64, on: bool, opts: &DeployOptions) -> Result<Option<DeployOutcome>> {
    let p = install::active_profile(db)?;
    db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(load_order), -1) + 1 FROM profile_mods WHERE profile_id = ?1))
                     ON CONFLICT(profile_id, install_id) DO UPDATE SET enabled = ?3", params![p, install_id, on as i32])?;
    db.set_setting("loadorder.changedAt", &now().to_string())?;
    if on { Ok(Some(install::redeploy_any(db, game, library, install_id, opts)?)) } else { install::undeploy_any(db, game, install_id)?; Ok(None) }
}

/// Set the order of the active profile's mods (first loads first).
pub fn set_order(db: &Db, install_ids: &[i64]) -> Result<()> {
    let p = install::active_profile(db)?;
    let tx = db.conn.unchecked_transaction()?;
    for (i, id) in install_ids.iter().enumerate() {
        tx.execute("UPDATE profile_mods SET load_order = ?3 WHERE profile_id = ?1 AND install_id = ?2", params![p, id, i as i64])?;
    }
    tx.commit()?;
    db.set_setting("loadorder.changedAt", &now().to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedmodEntry { pub install_id: i64, pub name: String, pub folders: Vec<String>, pub enabled: bool, pub load_order: i64 }

/// The active profile's REDmods in load order, with the `mods/<folder>` names each one deploys.
pub fn redmod_order(db: &Db) -> Result<Vec<RedmodEntry>> {
    let p = install::active_profile(db)?;
    let mut st = db.conn.prepare("SELECT i.id, i.name, i.staging, pm.enabled, pm.load_order FROM installs i JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1 WHERE i.mod_type = 'redmod' ORDER BY pm.load_order")?;
    let rows: Vec<(i64, String, String, bool, i64)> = st.query_map(params![p], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<std::result::Result<_, _>>()?;
    Ok(rows.into_iter().map(|(install_id, name, staging, enabled, load_order)| {
        let folders = staged_plan(Path::new(&staging), &name).redmods;
        RedmodEntry { install_id, name, folders, enabled, load_order }
    }).collect())
}

/// When REDmods were last deployed, and whether anything changed since.
pub fn redmod_needs_deploy(db: &Db) -> Result<bool> {
    let last: i64 = db.setting("redmod.lastDeploy")?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let changed: i64 = db.setting("loadorder.changedAt")?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let newest: i64 = db.conn.query_row("SELECT COALESCE(MAX(installed_at), 0) FROM installs WHERE mod_type = 'redmod'", [], |r| r.get(0))?;
    let any: i64 = db.conn.query_row("SELECT COUNT(*) FROM installs WHERE mod_type = 'redmod'", [], |r| r.get(0))?;
    Ok(any > 0 && (newest > last || changed > last))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn switching_profiles_changes_the_game_folder() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("lib");
        let db = Db::memory().unwrap();
        let mk = |n: &str| {
            let a = dir.path().join(format!("{n}.zip"));
            let mut z = zip::ZipWriter::new(fs::File::create(&a).unwrap());
            z.start_file(format!("{n}.archive"), zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(n.as_bytes()).unwrap();
            z.finish().unwrap();
            a
        };
        for (i, n) in ["a", "b"].iter().enumerate() {
            let req = install::InstallRequest { mod_id: i as u32 + 1, file_id: 1, name: n.to_string(), version: "1".into(), archive: mk(n), accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None, same_file_only: false };
            install::install(&db, &game, &lib, &req).unwrap();
        }
        let default = install::active_profile(&db).unwrap();
        let other = create(&db, "Only A", Some(default)).unwrap();
        // turn B off in the new profile, then switch
        db.conn.execute("UPDATE profile_mods SET enabled = 0 WHERE profile_id = ?1 AND install_id = 2", params![other]).unwrap();
        let rep = activate(&db, &game, &lib, other, &DeployOptions::default()).unwrap();
        assert_eq!(rep.removed, [2]);
        assert!(game.join("archive/pc/mod/a.archive").exists() && !game.join("archive/pc/mod/b.archive").exists());
        let back = activate(&db, &game, &lib, default, &DeployOptions::default()).unwrap();
        assert_eq!(back.deployed, [2]);
        assert!(game.join("archive/pc/mod/b.archive").exists());
        assert!(delete(&db, default).is_err(), "the active profile stays");
        delete(&db, other).unwrap();
        assert_eq!(list(&db).unwrap().len(), 1);

        set_enabled(&db, &game, &lib, 1, false, &DeployOptions::default()).unwrap();
        assert!(!game.join("archive/pc/mod/a.archive").exists());
        set_order(&db, &[2, 1]).unwrap();
        assert_eq!(install::list_installs(&db).unwrap()[0].id, 2);
    }

    #[test]
    fn a_take_over_choice_survives_a_profile_switch() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("lib");
        let db = Db::memory().unwrap();
        for (i, body) in ["from A", "from B"].iter().enumerate() {
            let a = dir.path().join(format!("{i}.zip"));
            let mut z = zip::ZipWriter::new(fs::File::create(&a).unwrap());
            z.start_file("shared.archive", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(body.as_bytes()).unwrap();
            z.finish().unwrap();
            let req = install::InstallRequest { mod_id: i as u32 + 1, file_id: 1, name: format!("Mod {i}"), version: "1".into(), archive: a, accept_unknown: false, replace_older: true, options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None, same_file_only: false };
            install::install(&db, &game, &lib, &req).unwrap();
        }
        let target = game.join("archive/pc/mod/shared.archive");
        // the second clashes; the user takes the files over
        assert!(matches!(install::redeploy(&db, &game, &lib, 2, &DeployOptions::default()).unwrap(), DeployOutcome::Conflicts { .. }));
        install::redeploy(&db, &game, &lib, 2, &DeployOptions { take_conflicts: true, ..Default::default() }).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "from B");
        let default = install::active_profile(&db).unwrap();
        let empty = create(&db, "Empty", None).unwrap();
        activate(&db, &game, &lib, empty, &DeployOptions::default()).unwrap();
        assert!(!target.exists());
        let back = activate(&db, &game, &lib, default, &DeployOptions::default()).unwrap();
        assert!(back.not_deployed.is_empty(), "no mod comes back as a clash: {:?}", back.not_deployed);
        assert_eq!(back.deployed.len(), 2);
        assert_eq!(fs::read_to_string(&target).unwrap(), "from B");
    }
}
