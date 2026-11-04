//! Mods already in the game folder that no manager owns (copied in by hand, or by a manager that left no record).
//! Tracking one makes it an ordinary Pocket Mods install without touching the game: each file gets a second hard
//! link in the mod library (a copy when the library is on another drive), so turning it off or removing it later is
//! as reversible as any other mod.

use crate::cyberpunk::frameworks::FRAMEWORKS;
use crate::cyberpunk::layout::{plan_install, ModType};
use crate::db::{now, Db};
use crate::error::Result;
use crate::install;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMod {
    pub name: String,
    pub kind: ModType,
    /// Game-relative files.
    pub files: Vec<String>,
    /// The Nexus mod, when the folder is a known framework.
    pub mod_id: Option<u32>,
}

fn rel(game: &Path, p: &Path) -> String { p.strip_prefix(game).map(|x| x.to_string_lossy().replace('\\', "/")).unwrap_or_default() }

fn walk(dir: &Path, game: &Path) -> Vec<String> {
    walkdir::WalkDir::new(dir).into_iter().flatten().filter(|e| e.file_type().is_file()).map(|e| rel(game, e.path()))
        .filter(|p| !p.contains("vortex.deployment")).collect()
}

/// Mod-shaped things in the game folder, minus every file a tracked install already owns.
pub fn scan(db: &Db, game: &Path, also_skip: &BTreeSet<String>) -> Result<Vec<FoundMod>> {
    let owned: BTreeSet<String> = {
        let mut st = db.conn.prepare("SELECT target FROM deployed_files")?;
        let v = st.query_map([], |r| r.get::<_, String>(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    let free = |p: &String| { let k = p.to_lowercase(); !owned.contains(&k) && !also_skip.contains(&k) };
    let mut units: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // archives, with their .xl beside them
    if let Ok(rd) = fs::read_dir(game.join("archive/pc/mod")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !e.path().is_file() { continue; }
            let stem = n.trim_end_matches(".xl").trim_end_matches(".archive").to_string();
            if n.ends_with(".archive") || n.ends_with(".xl") { units.entry(stem).or_default().push(rel(game, &e.path())); }
        }
    }
    // folders that are one mod each
    for (dir, need) in [("mods", Some("info.json")), ("r6/scripts", None), ("r6/tweaks", None), ("bin/x64/plugins/cyber_engine_tweaks/mods", None), ("red4ext/plugins", None)] {
        let Ok(rd) = fs::read_dir(game.join(dir)) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                if need.is_some_and(|f| !e.path().join(f).exists()) { continue; }
                units.entry(n).or_default().extend(walk(&e.path(), game));
            } else if dir.starts_with("r6/") && (n.ends_with(".reds") || n.ends_with(".yaml") || n.ends_with(".yml")) {
                let stem = n.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(n.clone());
                units.entry(stem).or_default().push(rel(game, &e.path()));
            }
        }
    }
    let mut out = Vec::new();
    for (name, files) in units {
        let files: Vec<String> = files.into_iter().filter(free).collect();
        if files.is_empty() { continue; }
        let plan = plan_install(&files, &name);
        let fw = FRAMEWORKS.iter().find(|f| f.files.iter().any(|p| files.iter().any(|x| x.eq_ignore_ascii_case(p))) || f.name.eq_ignore_ascii_case(&name));
        out.push(FoundMod { name: fw.map(|f| f.name.to_string()).unwrap_or(name), kind: if fw.is_some() { ModType::Framework } else { plan.mod_type.unwrap_or(ModType::Unknown) }, files, mod_id: fw.map(|f| f.nexus_mod_id) });
    }
    Ok(out)
}

/// Track found mods: link their files into the library and record them as installs owned by Pocket Mods.
pub fn track(db: &Db, game: &Path, library: &Path, mods: &[FoundMod]) -> Result<Vec<i64>> {
    let profile = install::active_profile(db)?;
    let mut ids = Vec::new();
    for m in mods {
        let local = 900_000_000 + (m.name.bytes().fold(11u32, |a, c| a.wrapping_mul(31).wrapping_add(c as u32)) % 90_000_000);
        let mod_id = m.mod_id.unwrap_or(local);
        let staging = library.join("staging").join(format!("found-{}", crate::cyberpunk::layout::safe_folder(&m.name)));
        let plan = plan_install(&m.files, &m.name);
        let fws: Vec<&str> = plan.uses.iter().map(|f| crate::cyberpunk::frameworks::info(*f).key).collect();
        db.conn.execute("INSERT OR IGNORE INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, source) VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7, 'found')",
            params![mod_id, local, m.name, m.kind.as_str(), serde_json::to_string(&fws).unwrap_or_default(), staging.to_string_lossy(), now()])?;
        let id: i64 = db.conn.query_row("SELECT id FROM installs WHERE mod_id = ?1 AND file_id = ?2", params![mod_id, local], |r| r.get(0))?;
        db.conn.execute("INSERT OR IGNORE INTO profile_mods (profile_id, install_id, enabled, load_order) VALUES (?1, ?2, 1, (SELECT COALESCE(MAX(load_order), -1) + 1 FROM profile_mods WHERE profile_id = ?1))", params![profile, id])?;
        for f in &m.files {
            let from = game.join(f);
            let to = staging.join(f);
            if let Some(p) = to.parent() { fs::create_dir_all(p)?; }
            let method = if to.exists() || fs::hard_link(&from, &to).is_ok() { "hardlink" } else { fs::copy(&from, &to)?; "copy" };
            let size = fs::metadata(&from).map(|x| x.len()).unwrap_or(0);
            db.conn.execute("INSERT OR IGNORE INTO deployed_files (target, install_id, source, method, size, deployed_at, target_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![f.to_lowercase(), id, to.to_string_lossy(), method, size as i64, now(), f])?;
        }
        ids.push(id);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_hand_installed_mods_and_tracks_them_reversibly() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        for p in ["archive/pc/mod/Hat.archive", "archive/pc/mod/Hat.archive.xl", "mods/BetterQuests/info.json", "mods/BetterQuests/archives/a.archive",
                  "r6/scripts/Fixes/fix.reds", "red4ext/plugins/ArchiveXL/ArchiveXL.dll", "mods/NotAMod/readme.txt"] {
            let f = game.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, p).unwrap();
        }
        let db = Db::memory().unwrap();
        let found = scan(&db, &game, &BTreeSet::new()).unwrap();
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["ArchiveXL", "BetterQuests", "Fixes", "Hat"]);
        assert_eq!(found.iter().find(|m| m.name == "ArchiveXL").unwrap().mod_id, Some(4198));
        assert_eq!(found.iter().find(|m| m.name == "Hat").unwrap().files.len(), 2);

        let lib = dir.path().join("lib");
        let ids = track(&db, &game, &lib, &found).unwrap();
        assert_eq!(ids.len(), 4);
        assert!(scan(&db, &game, &BTreeSet::new()).unwrap().is_empty(), "tracked files are not found again");
        assert!(crate::deploy::verify(&db, &game).unwrap().is_empty());
        // now an ordinary mod: off and on again with nothing lost
        crate::deploy::undeploy(&db, &game, ids[3]).unwrap();
        assert!(!game.join("archive/pc/mod/Hat.archive").exists());
        crate::install::redeploy(&db, &game, &lib, ids[3], &Default::default()).unwrap();
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/Hat.archive")).unwrap(), "archive/pc/mod/Hat.archive");
    }
}
