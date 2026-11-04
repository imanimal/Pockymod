//! The user's standing choices about mods that want the same files, so a choice made once holds on every deploy,
//! profile switch and update:
//! - "Take them over": mod A's files win over mod B's on the paths both want. When A goes in, it takes those paths
//!   from B without asking; when B goes in while A is there, B stays out of them (and gets them back if A goes).
//! - Archive renames: an `.archive` deployed under another name (a `!` prefix), so it sorts, and so loads, earlier.
//!   The staged file keeps its name; only the link in the game is named differently. Removing the rule and
//!   deploying again puts the original name back.
//!
//! Both are kept by Nexus mod ID (an update is a new install of the same mod), and by install for two files of one
//! mod.

use crate::cyberpunk::layout::Placement;
use crate::db::{now, Db};
use crate::error::Result;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict { Wins, Loses }

fn mod_of(db: &Db, install_id: i64) -> Result<Option<(u32, String)>> {
    Ok(db.conn.query_row("SELECT mod_id, name FROM installs WHERE id = ?1", params![install_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?)
}

/// Does a standing choice say whether `me` wins over `other`? The newest matching choice counts.
pub fn verdict(db: &Db, me: i64, other: i64) -> Result<Option<Verdict>> {
    let (Some((mm, _)), Some((om, _))) = (mod_of(db, me)?, mod_of(db, other)?) else { return Ok(None) };
    let row: Option<i64> = db.conn.query_row(
        "SELECT winner_install = ?1 OR (winner_mod = ?3 AND ?3 != ?4) FROM conflict_rules
         WHERE (winner_install = ?1 AND loser_install = ?2) OR (winner_install = ?2 AND loser_install = ?1)
            OR (?3 != ?4 AND ((winner_mod = ?3 AND loser_mod = ?4) OR (winner_mod = ?4 AND loser_mod = ?3)))
         ORDER BY created_at DESC, id DESC LIMIT 1",
        params![me, other, mm, om], |r| r.get(0)).optional()?;
    Ok(row.map(|w| if w != 0 { Verdict::Wins } else { Verdict::Loses }))
}

/// Remember that `winner`'s files win over `loser`'s (replacing any earlier choice between the two).
pub fn remember_win(db: &Db, winner: i64, loser: i64, files: usize) -> Result<()> {
    let (Some((wm, wn)), Some((lm, ln))) = (mod_of(db, winner)?, mod_of(db, loser)?) else { return Ok(()) };
    db.conn.execute(
        "DELETE FROM conflict_rules WHERE (winner_install IN (?1, ?2) AND loser_install IN (?1, ?2))
            OR (?3 != ?4 AND winner_mod IN (?3, ?4) AND loser_mod IN (?3, ?4) AND winner_mod != loser_mod)",
        params![winner, loser, wm, lm])?;
    db.conn.execute(
        "INSERT INTO conflict_rules (winner_mod, winner_install, winner_name, loser_mod, loser_install, loser_name, files, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![wm, winner, wn, lm, loser, ln, files as i64, now()])?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WinRule { pub id: i64, pub winner: String, pub loser: String, pub winner_install: i64, pub loser_install: i64, pub files: i64, pub created_at: i64 }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameRule { pub mod_id: u32, pub name: String, pub target: String, pub renamed: String, pub install_id: Option<i64>, pub created_at: i64 }

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rules { pub wins: Vec<WinRule>, pub renames: Vec<RenameRule> }

/// Every standing choice, newest first. Names are the installs' current names when they are still installed.
pub fn list(db: &Db) -> Result<Rules> {
    let mut st = db.conn.prepare(
        "SELECT r.id, COALESCE(w.name, r.winner_name), COALESCE(l.name, r.loser_name), r.winner_install, r.loser_install, r.files, r.created_at
         FROM conflict_rules r LEFT JOIN installs w ON w.id = r.winner_install LEFT JOIN installs l ON l.id = r.loser_install ORDER BY r.created_at DESC, r.id DESC")?;
    let wins = st.query_map([], |r| Ok(WinRule { id: r.get(0)?, winner: r.get(1)?, loser: r.get(2)?, winner_install: r.get(3)?, loser_install: r.get(4)?, files: r.get(5)?, created_at: r.get(6)? }))?
        .collect::<std::result::Result<_, _>>()?;
    let mut st = db.conn.prepare(
        "SELECT a.mod_id, COALESCE((SELECT name FROM installs i WHERE i.mod_id = a.mod_id ORDER BY installed_at DESC LIMIT 1), a.install_name), a.target, a.renamed,
                (SELECT id FROM installs i WHERE i.mod_id = a.mod_id ORDER BY installed_at DESC LIMIT 1), a.created_at
         FROM archive_renames a ORDER BY a.created_at DESC")?;
    let renames = st.query_map([], |r| Ok(RenameRule { mod_id: r.get(0)?, name: r.get(1)?, target: r.get(2)?, renamed: r.get(3)?, install_id: r.get(4)?, created_at: r.get(5)? }))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(Rules { wins, renames })
}

/// Forget a "Take them over" choice. The game stays as it is; the next deploy of those mods asks again.
pub fn forget_win(db: &Db, id: i64) -> Result<()> {
    db.conn.execute("DELETE FROM conflict_rules WHERE id = ?1", params![id])?;
    Ok(())
}

fn tkey(target: &str) -> String { target.replace('\\', "/").to_lowercase() }

/// The archive renames for an install's mod: target (lower case, game-relative) to the new file name.
pub fn renames_for(db: &Db, install_id: i64) -> Result<BTreeMap<String, String>> {
    let Some((mod_id, _)) = mod_of(db, install_id)? else { return Ok(BTreeMap::new()) };
    let mut st = db.conn.prepare("SELECT target, renamed FROM archive_renames WHERE mod_id = ?1")?;
    let v = st.query_map(params![mod_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

/// A plan with the install's archive renames applied (same order, same sources).
pub fn apply_renames(db: &Db, install_id: i64, plan: &[Placement]) -> Result<Vec<Placement>> {
    let map = renames_for(db, install_id)?;
    Ok(plan.iter().map(|p| match map.get(&tkey(&p.target)) {
        Some(new_name) => {
            let parent = p.target.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            Placement { target: if parent.is_empty() { new_name.clone() } else { format!("{parent}/{new_name}") }, ..p.clone() }
        }
        None => p.clone(),
    }).collect())
}

/// Deploy `target` (as the plan names it) of an install's mod under the file name `renamed` from now on.
pub fn set_rename(db: &Db, install_id: i64, target: &str, renamed: &str) -> Result<()> {
    let Some((mod_id, name)) = mod_of(db, install_id)? else { return Ok(()) };
    db.conn.execute("INSERT INTO archive_renames (mod_id, target, renamed, install_name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(mod_id, target) DO UPDATE SET renamed = ?3, install_name = ?4, created_at = ?5",
        params![mod_id, tkey(target), renamed, name, now()])?;
    Ok(())
}

pub fn clear_rename(db: &Db, mod_id: u32, target: &str) -> Result<()> {
    db.conn.execute("DELETE FROM archive_renames WHERE mod_id = ?1 AND target = ?2", params![mod_id, tkey(target)])?;
    Ok(())
}

/// A name for `file` that sorts before every name in `before` (byte order, as the game sorts archive names), made
/// with the fewest `!` in front. `!` is the lowest character Windows allows at the start of a name apart from a
/// space. None when one of them starts with a space (nothing sensible sorts before it).
pub fn load_first_name(file: &str, before: &[String]) -> Option<String> {
    let base = file.trim_start_matches('!');
    for n in 1..=8 {
        let cand = format!("{}{base}", "!".repeat(n));
        if before.iter().all(|b| cand.as_bytes() < b.as_bytes()) { return Some(cand); }
        if before.iter().any(|b| b.starts_with(' ')) { return None; }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(db: &Db, id: i64, mod_id: u32, name: &str) {
        db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, ?2, ?1, ?3, '1', 'archive', '[]', '', ?1)",
            params![id, mod_id, name]).unwrap();
    }

    #[test]
    fn a_choice_holds_both_ways_and_across_updates() {
        let db = Db::memory().unwrap();
        add(&db, 1, 10, "A");
        add(&db, 2, 20, "B");
        assert_eq!(verdict(&db, 1, 2).unwrap(), None);
        remember_win(&db, 1, 2, 3).unwrap();
        assert_eq!(verdict(&db, 1, 2).unwrap(), Some(Verdict::Wins));
        assert_eq!(verdict(&db, 2, 1).unwrap(), Some(Verdict::Loses));
        // A is updated: a new install of mod 10
        add(&db, 3, 10, "A v2");
        assert_eq!(verdict(&db, 3, 2).unwrap(), Some(Verdict::Wins));
        // the user changes their mind: B wins now, and only that choice is kept
        remember_win(&db, 2, 3, 3).unwrap();
        assert_eq!(verdict(&db, 3, 2).unwrap(), Some(Verdict::Loses));
        let l = list(&db).unwrap();
        assert_eq!(l.wins.len(), 1);
        assert_eq!((l.wins[0].winner.as_str(), l.wins[0].loser.as_str()), ("B", "A v2"));
        forget_win(&db, l.wins[0].id).unwrap();
        assert_eq!(verdict(&db, 3, 2).unwrap(), None);
    }

    #[test]
    fn two_files_of_one_mod_are_told_apart_by_install() {
        let db = Db::memory().unwrap();
        add(&db, 1, 10, "Main");
        add(&db, 2, 10, "Patch");
        remember_win(&db, 2, 1, 1).unwrap();
        assert_eq!(verdict(&db, 2, 1).unwrap(), Some(Verdict::Wins));
        add(&db, 3, 10, "Other file");
        assert_eq!(verdict(&db, 3, 1).unwrap(), None, "a mod-wide rule would be wrong inside one mod");
        // a choice against another mod leaves the one inside this mod alone
        add(&db, 4, 20, "Elsewhere");
        remember_win(&db, 1, 4, 1).unwrap();
        assert_eq!(verdict(&db, 2, 1).unwrap(), Some(Verdict::Wins));
        assert_eq!(verdict(&db, 4, 1).unwrap(), Some(Verdict::Loses));
    }

    #[test]
    fn renames_apply_to_the_plan_only() {
        let db = Db::memory().unwrap();
        add(&db, 1, 10, "Coat");
        let plan = vec![
            Placement { source: "x/coat.archive".into(), target: "archive/pc/mod/Coat.archive".into(), kind: crate::cyberpunk::layout::FileKind::Archive },
            Placement { source: "x/coat.archive.xl".into(), target: "archive/pc/mod/Coat.archive.xl".into(), kind: crate::cyberpunk::layout::FileKind::ArchiveXl },
        ];
        set_rename(&db, 1, "archive/pc/mod/Coat.archive", "!Coat.archive").unwrap();
        let p = apply_renames(&db, 1, &plan).unwrap();
        assert_eq!(p[0].target, "archive/pc/mod/!Coat.archive");
        assert_eq!(p[0].source, "x/coat.archive");
        assert_eq!(p[1].target, "archive/pc/mod/Coat.archive.xl");
        assert_eq!(list(&db).unwrap().renames[0].renamed, "!Coat.archive");
        clear_rename(&db, 10, "archive/pc/mod/Coat.archive").unwrap();
        assert_eq!(apply_renames(&db, 1, &plan).unwrap(), plan);
    }

    #[test]
    fn load_first_names() {
        assert_eq!(load_first_name("coat.archive", &["basegame_hair.archive".into()]).as_deref(), Some("!coat.archive"));
        assert_eq!(load_first_name("coat.archive", &["!!x.archive".into()]).as_deref(), Some("!!coat.archive"));
        assert_eq!(load_first_name("coat.archive", &["!!a.archive".into()]).as_deref(), Some("!!!coat.archive"));
        assert_eq!(load_first_name("!coat.archive", &["#x.archive".into()]).as_deref(), Some("!coat.archive"));
        assert_eq!(load_first_name("coat.archive", &[" x.archive".into()]), None);
    }
}
