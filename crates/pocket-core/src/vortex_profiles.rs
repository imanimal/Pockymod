//! Vortex's profiles for Cyberpunk 2077, brought in as Pocket Mods profiles, so mods brought in from Vortex keep
//! the sets they were in.
//!
//! From Vortex's source (github.com/Nexus-Mods/vortex: `src/util/LevelPersist.ts`, `src/util/StorePersistor.ts`,
//! `src/extensions/profile_management`), read 2026-11-04. UNVERIFIED against a real `state.v2` folder:
//! - Vortex keeps its persisted state in a LevelDB folder, `%APPDATA%\Vortex\state.v2`. Every leaf of the state is
//!   one key: its path joined with `###`, the value JSON (`"cyberpunk2077"`, `true`, `1730000000000`).
//! - A profile is `persistent###profiles###<id>###…`: `gameId`, `name`, `lastActivated`, and
//!   `modState###<mod id>###enabled` for each mod. A mod's id is its folder in the staging folder, the same name
//!   `vortex.rs` tracks a Vortex mod by (`installs.external`).
//! - The profile last used for a game is `settings###profiles###lastActiveProfile###<game id>`.
//!
//! Vortex's database is never opened where it is: its files are copied to a temporary folder first (Vortex may be
//! running and holds a lock, and opening a LevelDB can rewrite its log), and the copy is deleted after.

use crate::db::{now, Db};
use crate::deploy::{DeployOptions, DeployOutcome};
use crate::error::{Error, Result};
use crate::{install, profiles};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SEP: &str = "###";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VortexProfile {
    pub id: String,
    pub name: String,
    /// The mods turned on in it, by Vortex's mod id (the staging folder name).
    pub enabled: BTreeSet<String>,
    /// Milliseconds since 1970; 0 when not known.
    pub last_activated: i64,
    /// The one Vortex last used for the game, so the one deployed now.
    pub active: bool,
}

/// `%APPDATA%\Vortex\state.v2`, when it is there.
pub fn state_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Vortex").join("state.v2")).filter(|p| p.join("CURRENT").exists())
}

/// The profiles for `game` from a list of Vortex's state keys and values.
pub fn from_entries<I: IntoIterator<Item = (String, String)>>(entries: I, game: &str) -> Vec<VortexProfile> {
    #[derive(Default)]
    struct P { game: Option<String>, name: Option<String>, last: i64, mods: BTreeMap<String, bool> }
    let mut by: BTreeMap<String, P> = BTreeMap::new();
    let mut last_active: Option<String> = None;
    for (k, v) in entries {
        let path: Vec<&str> = k.split(SEP).collect();
        let val: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
        match path.as_slice() {
            ["persistent", "profiles", id, rest @ ..] => {
                let p = by.entry((*id).to_string()).or_default();
                match rest {
                    ["gameId"] => p.game = val.as_str().map(str::to_owned),
                    ["name"] => p.name = val.as_str().map(str::to_owned),
                    ["lastActivated"] => p.last = val.as_i64().or_else(|| val.as_f64().map(|f| f as i64)).unwrap_or(0),
                    ["modState", m, "enabled"] => { p.mods.insert((*m).to_string(), val.as_bool().unwrap_or(false)); }
                    _ => {}
                }
            }
            ["settings", "profiles", "lastActiveProfile", g] if *g == game => last_active = val.as_str().map(str::to_owned),
            _ => {}
        }
    }
    let mut out: Vec<VortexProfile> = by.into_iter().filter(|(_, p)| p.game.as_deref() == Some(game)).map(|(id, p)| VortexProfile {
        active: last_active.as_deref() == Some(id.as_str()),
        name: p.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Vortex {}", &id[..id.len().min(6)])),
        enabled: p.mods.into_iter().filter(|(_, on)| *on).map(|(m, _)| m).collect(),
        last_activated: p.last,
        id,
    }).collect();
    // no record of the last one used: the most recently activated
    if !out.iter().any(|p| p.active) {
        if let Some(p) = out.iter_mut().max_by_key(|p| p.last_activated) { p.active = true; }
    }
    out.sort_by(|a, b| b.active.cmp(&a.active).then(b.last_activated.cmp(&a.last_activated)));
    out
}

/// Vortex's profiles for Cyberpunk 2077, read from a copy of its state folder.
pub fn read(state: &Path) -> Result<Vec<VortexProfile>> {
    let tmp = std::env::temp_dir().join(format!("pocket-mods-vortex-{}-{}", std::process::id(), now()));
    let copy = || -> Result<()> {
        fs::create_dir_all(&tmp)?;
        for e in fs::read_dir(state)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            // the lock is Vortex's, the info logs are text: only the database files are needed
            if n == "LOCK" || n.starts_with("LOG") || !e.path().is_file() { continue; }
            fs::copy(e.path(), tmp.join(&n))?;
        }
        Ok(())
    };
    let res = copy().and_then(|_| {
        let opts = rusty_leveldb::Options { create_if_missing: false, ..Default::default() };
        let mut db = rusty_leveldb::DB::open(&tmp, opts).map_err(|e| Error::Invalid(format!("Vortex's settings could not be read: {e}")))?;
        let mut it = db.new_iter().map_err(|e| Error::Invalid(format!("Vortex's settings could not be read: {e}")))?;
        let mut entries = Vec::new();
        use rusty_leveldb::LdbIterator;
        while let Some((k, v)) = it.next() {
            let k = String::from_utf8_lossy(&k).into_owned();
            if k.starts_with("persistent###profiles###") || k.starts_with("settings###profiles###") {
                entries.push((k, String::from_utf8_lossy(&v).into_owned()));
            }
        }
        Ok(from_entries(entries, crate::GAME_DOMAIN))
    });
    let _ = fs::remove_dir_all(&tmp);
    res
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportedProfile { pub name: String, pub created: bool, pub on: usize, pub of: usize }

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub profiles: Vec<ImportedProfile>,
    /// The profile active afterwards: the one Vortex has deployed.
    pub active: Option<String>,
    /// Mods the switch put in or took out of the game (only when the game did not match Vortex's profile).
    pub deployed: usize,
    pub removed: usize,
    pub not_deployed: Vec<(i64, String)>,
}

/// Bring Vortex's profiles in. Each becomes a Pocket Mods profile of the same name (an existing one of that name is
/// updated): Vortex's mods are on or off as they are in it, and mods from Pocket Mods or the game folder stay as they
/// are in the active profile. The profile Vortex has deployed becomes the active one, so the game folder does not
/// change (or changes only where it no longer matched Vortex). Safe to run again: it only sets the same rows.
pub fn import(db: &Db, game: &Path, library: &Path, opts: &DeployOptions, list: &[VortexProfile]) -> Result<ImportReport> {
    let mut rep = ImportReport::default();
    let tracked: Vec<(String, i64)> = {
        let mut st = db.conn.prepare("SELECT external, id FROM installs WHERE source = 'vortex' AND external IS NOT NULL")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    if tracked.is_empty() || list.is_empty() { return Ok(rep); }
    let p0 = install::active_profile(db)?;
    let p0_name: String = db.conn.query_row("SELECT name FROM profiles WHERE id = ?1", params![p0], |r| r.get(0))?;
    // the deployed one first, so the rest are written to profiles that are not active
    let mut order: Vec<&VortexProfile> = list.iter().collect();
    order.sort_by_key(|p| !p.active);
    let mut used: BTreeSet<String> = BTreeSet::new();
    for vp in order {
        // two Vortex profiles can share a name; Pocket Mods' names are unique
        let mut name = vp.name.trim().to_string();
        let mut n = 2;
        while used.contains(&name.to_lowercase()) { name = format!("{} ({n})", vp.name.trim()); n += 1; }
        used.insert(name.to_lowercase());
        let existing: Option<i64> = db.conn.query_row("SELECT id FROM profiles WHERE name = ?1 COLLATE NOCASE", params![name], |r| r.get(0)).optional()?;
        let (target, created) = match existing { Some(id) => (id, false), None => (profiles::create(db, &name, Some(p0))?, true) };
        let on_in = |folder: &str| vp.enabled.contains(folder);
        let is_live = target == install::active_profile(db)?;
        let mut flips: Vec<(i64, bool)> = Vec::new();
        for (folder, iid) in &tracked {
            let want = on_in(folder);
            if is_live {
                let now_on: bool = db.conn.query_row("SELECT COALESCE((SELECT enabled FROM profile_mods WHERE profile_id = ?1 AND install_id = ?2), 0)", params![target, iid], |r| r.get(0))?;
                if now_on != want { flips.push((*iid, want)); }
            }
            db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(load_order), -1) + 1 FROM profile_mods WHERE profile_id = ?1))
                             ON CONFLICT(profile_id, install_id) DO UPDATE SET enabled = ?3", params![target, iid, want as i32])?;
        }
        // the active profile changed in place: the game follows, off first so nothing clashes on the way
        flips.sort_by_key(|(_, on)| *on);
        for (iid, on) in flips {
            let r = if on { install::redeploy_any(db, game, library, iid, opts).map(Some) } else { install::undeploy_any(db, game, iid).map(|_| None) };
            match r {
                Ok(Some(DeployOutcome::Deployed { .. })) => rep.deployed += 1,
                Ok(Some(DeployOutcome::Conflicts { conflicts })) => rep.not_deployed.push((iid, format!("{} files clash with other mods", conflicts.len()))),
                Ok(Some(DeployOutcome::NeedsCopy { reason })) => rep.not_deployed.push((iid, reason)),
                Ok(None) => rep.removed += 1,
                Err(e) => rep.not_deployed.push((iid, e.to_string())),
            }
        }
        if vp.active && target != install::active_profile(db)? {
            let sw = profiles::activate(db, game, library, target, opts)?;
            rep.deployed += sw.deployed.len();
            rep.removed += sw.removed.len();
            rep.not_deployed.extend(sw.not_deployed);
        }
        if vp.active { rep.active = Some(name.clone()); }
        rep.profiles.push(ImportedProfile { name, created, on: tracked.iter().filter(|(f, _)| on_in(f)).count(), of: tracked.len() });
    }
    if rep.active.is_none() { rep.active = Some(p0_name); }
    db.set_setting("vortex.profilesImported", &now().to_string())?;
    db.set_setting("loadorder.changedAt", &now().to_string())?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(k: &str, v: &str) -> (String, String) { (k.to_string(), v.to_string()) }

    fn sample() -> Vec<(String, String)> {
        vec![
            kv("persistent###profiles###aaa###gameId", "\"cyberpunk2077\""),
            kv("persistent###profiles###aaa###name", "\"Default\""),
            kv("persistent###profiles###aaa###lastActivated", "1730000000000"),
            kv("persistent###profiles###aaa###modState###ModA-1-1-0-1700000000###enabled", "true"),
            kv("persistent###profiles###aaa###modState###ModB-2-1-0-1700000000###enabled", "false"),
            kv("persistent###profiles###bbb###gameId", "\"cyberpunk2077\""),
            kv("persistent###profiles###bbb###name", "\"Hardcore\""),
            kv("persistent###profiles###bbb###lastActivated", "1720000000000"),
            kv("persistent###profiles###bbb###modState###ModB-2-1-0-1700000000###enabled", "true"),
            kv("persistent###profiles###ccc###gameId", "\"skyrimse\""),
            kv("persistent###profiles###ccc###name", "\"Skyrim\""),
            kv("settings###profiles###lastActiveProfile###cyberpunk2077", "\"aaa\""),
        ]
    }

    #[test]
    fn reads_the_profiles_of_this_game_only() {
        let ps = from_entries(sample(), "cyberpunk2077");
        assert_eq!(ps.len(), 2);
        assert_eq!(ps[0].name, "Default");
        assert!(ps[0].active && !ps[1].active);
        assert_eq!(ps[0].enabled.iter().collect::<Vec<_>>(), ["ModA-1-1-0-1700000000"]);
        assert_eq!(ps[1].enabled.iter().collect::<Vec<_>>(), ["ModB-2-1-0-1700000000"]);
    }

    #[test]
    fn without_a_last_active_record_the_newest_is_active() {
        let ps = from_entries(sample().into_iter().filter(|(k, _)| !k.starts_with("settings")), "cyberpunk2077");
        assert!(ps.iter().find(|p| p.name == "Default").unwrap().active);
    }

    fn tracked(db: &Db) -> (i64, i64) {
        let add = |f: &str, m: u32| {
            db.conn.execute("INSERT INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, source, external) VALUES (?1, 1, ?2, '1', 'archive', '[]', '', 0, 'vortex', ?2)", params![m, f]).unwrap();
            db.conn.last_insert_rowid()
        };
        let (a, b) = (add("ModA-1-1-0-1700000000", 1), add("ModB-2-1-0-1700000000", 2));
        let p = install::active_profile(db).unwrap();
        // as Vortex deployed them: A on, B off
        db.conn.execute("INSERT INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, 1, 0), (?1, ?3, 0, 1)", params![p, a, b]).unwrap();
        (a, b)
    }
    fn on(db: &Db, profile: &str, install: i64) -> bool {
        db.conn.query_row("SELECT pm.enabled FROM profile_mods pm JOIN profiles p ON p.id = pm.profile_id WHERE p.name = ?1 AND pm.install_id = ?2", params![profile, install], |r| r.get(0)).unwrap()
    }

    #[test]
    fn each_vortex_profile_becomes_one_here_and_the_game_is_not_touched() {
        let db = Db::memory().unwrap();
        let (a, b) = tracked(&db);
        let dir = tempfile::tempdir().unwrap();
        let rep = import(&db, dir.path(), dir.path(), &Default::default(), &from_entries(sample(), "cyberpunk2077")).unwrap();
        assert_eq!(rep.profiles.len(), 2);
        assert_eq!(rep.active.as_deref(), Some("Default"));
        assert_eq!((rep.deployed, rep.removed), (0, 0), "the game already matched Vortex's active profile");
        assert!(on(&db, "Default", a) && !on(&db, "Default", b));
        assert!(!on(&db, "Hardcore", a) && on(&db, "Hardcore", b));
        // again: nothing new
        let again = import(&db, dir.path(), dir.path(), &Default::default(), &from_entries(sample(), "cyberpunk2077")).unwrap();
        assert!(again.profiles.iter().all(|p| !p.created));
        assert_eq!(profiles::list(&db).unwrap().len(), 2);
    }

    #[test]
    fn vortex_s_active_profile_becomes_the_active_one() {
        let db = Db::memory().unwrap();
        let (a, b) = tracked(&db);
        let dir = tempfile::tempdir().unwrap();
        let mut list = from_entries(sample(), "cyberpunk2077");
        list[0].name = "Main".into(); // Vortex's active profile is not called Default here
        let rep = import(&db, dir.path(), dir.path(), &Default::default(), &list).unwrap();
        assert_eq!(rep.active.as_deref(), Some("Main"));
        let active = profiles::list(&db).unwrap().into_iter().find(|p| p.active).unwrap();
        assert_eq!(active.name, "Main");
        assert!(on(&db, "Main", a) && !on(&db, "Main", b));
        assert_eq!((rep.deployed, rep.removed), (0, 0));
    }

    #[test]
    fn reads_a_real_leveldb_folder() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut db = rusty_leveldb::DB::open(dir.path(), rusty_leveldb::Options::default()).unwrap();
            for (k, v) in sample() { db.put(k.as_bytes(), v.as_bytes()).unwrap(); }
            db.flush().unwrap();
        }
        let ps = read(dir.path()).unwrap();
        assert_eq!(ps.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Default", "Hardcore"]);
        assert!(dir.path().join("CURRENT").exists(), "the original is left as it was");
    }
}
