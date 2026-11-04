//! Moving the mod library to a new folder, and linking the game's mod files again after the game itself moved.
//!
//! The library holds every staged mod, the backups of game files mods replaced, removed mods (trash) and
//! downloads. The database and each staged mod's `.pocket-manifest.json` store absolute paths into it, so a move
//! rewrites them all:
//! - On the same drive the folder is renamed in one step. Hard links in the game point at the files, not at
//!   their paths, so nothing in the game changes.
//! - On another drive it is copied, the paths are rewritten, and the game's files are linked to the new copies
//!   (when the new drive is the game's) or kept as copies (when it is not: hard links cannot cross drives, so
//!   from then on new mods are copied into the game). The old folder is deleted last.
//!
//! After the game moved (a new Steam library folder, a copy on another drive), the mod files in the new game
//! folder are copies rather than links. `relink_game` links them to the library again, or records them as
//! copies when the library is on another drive.

use crate::db::Db;
use crate::deploy::same_volume;
use crate::error::{Error, Result};
use rusqlite::params;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCheck {
    pub from: PathBuf,
    pub to: PathBuf,
    /// The new place is on the game's drive (hard links keep working).
    pub same_drive_as_game: bool,
    /// The new place is on the library's drive now (the move is a quick rename).
    pub quick: bool,
    pub files: usize,
    pub bytes: u64,
    /// Why it cannot be done, in plain words.
    pub problem: Option<String>,
}

fn inside(child: &Path, parent: &Path) -> bool {
    let c = fs::canonicalize(child).unwrap_or_else(|_| child.to_path_buf());
    let p = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    let lc = c.to_string_lossy().replace('\\', "/").to_lowercase();
    let lp = p.to_string_lossy().replace('\\', "/").to_lowercase();
    lc == lp || lc.starts_with(&format!("{}/", lp.trim_end_matches('/')))
}

/// What moving the library from `from` to `to` would do, and whether it can be done.
pub fn check_move(game: &Path, from: &Path, to: &Path) -> MoveCheck {
    let (mut files, mut bytes) = (0, 0);
    for e in walkdir::WalkDir::new(from).into_iter().flatten().filter(|e| e.file_type().is_file()) { files += 1; bytes += e.metadata().map(|m| m.len()).unwrap_or(0); }
    let problem = if !from.is_dir() { Some(format!("The library folder {} is not there, so there is nothing to move.", from.display())) }
        else if inside(to, from) { Some("The new place is inside the library itself. Choose a folder outside it.".into()) }
        else if inside(to, game) { Some("The new place is inside the game folder. Choose a folder outside it, on the same drive.".into()) }
        else if inside(game, to) { Some("The game folder is inside the new place. Choose a folder of its own.".into()) }
        else if to.exists() && fs::read_dir(to).map(|mut d| d.next().is_some()).unwrap_or(true) { Some(format!("{} is not empty. Choose a new or empty folder.", to.display())) }
        else if to.to_string_lossy().to_lowercase().contains("program files") { Some("Windows does not let normal users write under Program Files. Choose another folder.".into()) }
        else { None };
    let target = to.parent().unwrap_or(to);
    MoveCheck { from: from.to_path_buf(), to: to.to_path_buf(), same_drive_as_game: same_volume(target, game), quick: same_volume(target, from), files, bytes, problem }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MoveReport {
    /// "renamed" (one step on the same drive) or "copied".
    pub how: String,
    pub files: usize,
    pub bytes: u64,
    /// Stored paths that now point at the new place.
    pub rewritten: usize,
    /// Game files linked to their new copies.
    pub relinked: usize,
    /// Game files kept as copies (the library is on another drive than the game now).
    pub copies: usize,
    /// The old folder, when it could not be deleted completely.
    pub left_behind: Option<String>,
    /// The copy fallback was turned on, because hard links to the new place are impossible.
    pub copy_turned_on: bool,
}

/// `path` with `from` swapped for `to`, when it is inside `from`. Windows paths compare without case.
pub fn rebase(path: &str, from: &Path, to: &Path) -> Option<String> {
    let f = from.to_string_lossy();
    let f = f.trim_end_matches(['/', '\\']);
    if path.len() < f.len() || !path.is_char_boundary(f.len()) || !path[..f.len()].replace('\\', "/").eq_ignore_ascii_case(&f.replace('\\', "/")) { return None; }
    let rest = &path[f.len()..];
    if !(rest.is_empty() || rest.starts_with(['/', '\\'])) { return None; }
    Some(format!("{}{rest}", to.to_string_lossy().trim_end_matches(['/', '\\'])))
}

/// Point every stored path inside `from` at `to`: staged mods, deployed files' sources and backups, removed mods,
/// downloads, and the library setting. One transaction. Returns how many values changed.
pub fn rewrite_paths(db: &Db, from: &Path, to: &Path) -> Result<usize> {
    let tx = db.conn.unchecked_transaction()?;
    let mut n = 0;
    let mut update = |sql_select: &str, sql_update: &str| -> Result<()> {
        let rows: Vec<(rusqlite::types::Value, String)> = {
            let mut st = tx.prepare(sql_select)?;
            let v = st.query_map([], |r| Ok((r.get::<_, rusqlite::types::Value>(0)?, r.get::<_, String>(1)?)))?.collect::<std::result::Result<_, _>>()?;
            v
        };
        for (key, val) in rows {
            if let Some(new) = rebase(&val, from, to) { tx.execute(sql_update, params![key, new])?; n += 1; }
        }
        Ok(())
    };
    update("SELECT id, staging FROM installs", "UPDATE installs SET staging = ?2 WHERE id = ?1")?;
    update("SELECT target, source FROM deployed_files", "UPDATE deployed_files SET source = ?2 WHERE target = ?1")?;
    update("SELECT target, backup FROM deployed_files WHERE backup IS NOT NULL", "UPDATE deployed_files SET backup = ?2 WHERE target = ?1")?;
    update("SELECT id, kept FROM trash WHERE kept IS NOT NULL", "UPDATE trash SET kept = ?2 WHERE id = ?1")?;
    update("SELECT id, path FROM downloads", "UPDATE downloads SET path = ?2 WHERE id = ?1")?;
    // a removed mod's record keeps where its staged files were, for putting it back
    let rows: Vec<(i64, String)> = {
        let mut st = tx.prepare("SELECT id, data FROM trash")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    for (id, data) in rows {
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&data) else { continue };
        if let Some(new) = v["staging"].as_str().and_then(|s| rebase(s, from, to)) {
            v["staging"] = serde_json::Value::String(new);
            tx.execute("UPDATE trash SET data = ?2 WHERE id = ?1", params![id, v.to_string()])?;
            n += 1;
        }
    }
    tx.execute("INSERT INTO settings (key, value) VALUES ('library.dir', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1", params![to.to_string_lossy()])?;
    tx.commit()?;
    Ok(n)
}

fn copy_tree(from: &Path, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(usize, u64)> {
    let all: Vec<walkdir::DirEntry> = walkdir::WalkDir::new(from).into_iter().collect::<std::result::Result<_, _>>().map_err(|e| Error::Invalid(e.to_string()))?;
    let total: u64 = all.iter().filter(|e| e.file_type().is_file()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum();
    let (mut files, mut done) = (0, 0);
    for e in &all {
        let rel = e.path().strip_prefix(from).map_err(|e| Error::Invalid(e.to_string()))?;
        let dst = to.join(rel);
        if e.file_type().is_dir() { fs::create_dir_all(&dst)?; continue; }
        if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
        done += fs::copy(e.path(), &dst)?;
        files += 1;
        progress(done, total);
    }
    Ok((files, done))
}

/// Move the library from `from` to `to` (see the module comment). `progress` gets bytes copied and the total
/// when it has to copy.
pub fn move_library(db: &Db, game: &Path, from: &Path, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<MoveReport> {
    move_library_as(db, game, from, to, false, progress)
}

fn move_library_as(db: &Db, game: &Path, from: &Path, to: &Path, force_copy: bool, progress: &mut dyn FnMut(u64, u64)) -> Result<MoveReport> {
    let check = check_move(game, from, to);
    if let Some(p) = check.problem { return Err(Error::Invalid(p)); }
    let mut rep = MoveReport { files: check.files, bytes: check.bytes, ..Default::default() };
    if let Some(p) = to.parent() { fs::create_dir_all(p)?; }
    if to.is_dir() { fs::remove_dir(to)?; }
    let renamed = !force_copy && check.quick && fs::rename(from, to).is_ok();
    if renamed {
        rep.how = "renamed".into();
        match rewrite_paths(db, from, to) {
            Ok(n) => rep.rewritten = n,
            Err(e) => { let _ = fs::rename(to, from); return Err(e); }
        }
    } else {
        rep.how = "copied".into();
        if let Err(e) = copy_tree(from, to, progress) { let _ = fs::remove_dir_all(to); return Err(e); }
        // which deployed files lead into the library, before their paths change
        let rows: Vec<(String, String, String, String)> = {
            let mut st = db.conn.prepare("SELECT target, target_path, source, method FROM deployed_files")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
            v
        };
        match rewrite_paths(db, from, to) {
            Ok(n) => rep.rewritten = n,
            Err(e) => { let _ = fs::remove_dir_all(to); return Err(e); }
        }
        let link = same_volume(to, game);
        for (target, target_path, source, method) in rows {
            let Some(new_src) = rebase(&source, from, to) else { continue };
            if method != "hardlink" { continue; }
            let dst = game.join(&target_path);
            if link {
                // the game's file is a link to the old copy; link it to the new one
                let tmp = dst.with_extension("pocket-relink");
                if fs::hard_link(&new_src, &tmp).is_ok() {
                    let _ = fs::remove_file(&dst);
                    if fs::rename(&tmp, &dst).is_ok() { rep.relinked += 1; continue; }
                    let _ = fs::remove_file(&tmp);
                }
            }
            // another drive: the game keeps its file (same content as the new copy), recorded as a copy
            db.conn.execute("UPDATE deployed_files SET method = 'copy' WHERE target = ?1", params![target])?;
            rep.copies += 1;
        }
        if fs::remove_dir_all(from).is_err() && from.exists() { rep.left_behind = Some(from.to_string_lossy().into_owned()); }
    }
    refresh_manifests(db, to)?;
    if !same_volume(to, game) {
        db.set_setting("deploy.allowCopy", "1")?;
        rep.copy_turned_on = true;
    }
    Ok(rep)
}

/// Write each staged mod's manifest again (its paths moved with the library).
fn refresh_manifests(db: &Db, library: &Path) -> Result<()> {
    let ids: Vec<(i64, String)> = {
        let mut st = db.conn.prepare("SELECT id, staging FROM installs")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    for (id, staging) in ids {
        let s = Path::new(&staging);
        if s.starts_with(library) && s.join(crate::deploy::MANIFEST).is_file() { crate::deploy::refresh_manifest(s, db, id)?; }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelinkReport {
    pub checked: usize,
    /// Already linked to the library.
    pub ok: usize,
    /// Copies in the game replaced by links to the library.
    pub relinked: usize,
    /// Kept as copies (the library is on another drive than the game).
    pub copies: usize,
    /// Missing in the game, put back.
    pub restored: usize,
    /// Changed in the game (by a patch or by hand), left alone.
    pub changed: usize,
}

/// Does `game` look like a moved or copied game folder, with the mod files the database lists?
pub fn looks_like_moved_copy(db: &Db, game: &Path) -> Result<bool> {
    let rows: Vec<String> = {
        let mut st = db.conn.prepare("SELECT target_path FROM deployed_files WHERE method != 'vortex'")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    let present = rows.iter().filter(|t| game.join(t).is_file()).count();
    Ok(!rows.is_empty() && present * 2 >= rows.len())
}

/// Link the game's mod files to the library again (after the game folder moved). Vortex's files are Vortex's.
pub fn relink_game(db: &Db, game: &Path, allow_copy: bool) -> Result<RelinkReport> {
    let rows: Vec<(String, String, String, i64)> = {
        let mut st = db.conn.prepare("SELECT target, target_path, source, size FROM deployed_files WHERE method != 'vortex'")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    let mut rep = RelinkReport::default();
    for (target, target_path, source, size) in rows {
        let (src, dst) = (PathBuf::from(&source), game.join(&target_path));
        if !src.is_file() { continue; }
        rep.checked += 1;
        let set = |m: &str| db.conn.execute("UPDATE deployed_files SET method = ?2 WHERE target = ?1", params![target, m]);
        if !dst.exists() {
            if let Some(p) = dst.parent() { fs::create_dir_all(p)?; }
            if fs::hard_link(&src, &dst).is_ok() { set("hardlink")?; rep.restored += 1; }
            else if allow_copy { fs::copy(&src, &dst)?; set("copy")?; rep.restored += 1; }
            continue;
        }
        if same_file::is_same_file(&src, &dst).unwrap_or(false) { set("hardlink")?; rep.ok += 1; continue; }
        let len = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        if len != size as u64 { rep.changed += 1; continue; }
        let tmp = dst.with_extension("pocket-relink");
        if same_volume(&src, game) && fs::hard_link(&src, &tmp).is_ok() {
            fs::remove_file(&dst)?;
            fs::rename(&tmp, &dst)?;
            set("hardlink")?;
            rep.relinked += 1;
        } else {
            let _ = fs::remove_file(&tmp);
            set("copy")?;
            rep.copies += 1;
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::{deploy, verify, DeployOptions};
    use crate::cyberpunk::layout::plan_install;

    struct Env { _dir: tempfile::TempDir, root: PathBuf, game: PathBuf, lib: PathBuf, db: Db }
    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let game = root.join("Cyberpunk 2077");
        let lib = root.join("PocketMods");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        fs::create_dir_all(game.join("r6/config")).unwrap();
        fs::write(game.join("r6/config/inputUserMappings.xml"), "vanilla").unwrap();
        let db = Db::memory().unwrap();
        let st = lib.join("staging/1-1");
        fs::create_dir_all(st.join("archive/pc/mod")).unwrap();
        fs::write(st.join("archive/pc/mod/coat.archive"), "coat").unwrap();
        fs::create_dir_all(st.join("r6/config")).unwrap();
        fs::write(st.join("r6/config/inputUserMappings.xml"), "modded").unwrap();
        db.conn.execute("INSERT INTO installs (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (1, 1, 1, 'Coat', '1', 'archive', '[]', ?1, 0)", params![st.to_string_lossy()]).unwrap();
        let plan = plan_install(&["archive/pc/mod/coat.archive".into(), "r6/config/inputUserMappings.xml".into()], "Coat").files;
        deploy(&db, &game, &lib, 1, &st, &plan, &DeployOptions::default()).unwrap();
        fs::create_dir_all(lib.join("downloads")).unwrap();
        fs::write(lib.join("downloads/coat.zip"), "zip").unwrap();
        db.conn.execute("INSERT INTO downloads (mod_id, file_id, name, version, path, size, md5, downloaded_at) VALUES (1, 1, 'Coat', '1', ?1, 3, '', 0)", params![lib.join("downloads/coat.zip").to_string_lossy()]).unwrap();
        Env { _dir: dir, root, game, lib, db }
    }

    fn paths_point_at(e: &Env, to: &Path) {
        let staging: String = e.db.conn.query_row("SELECT staging FROM installs WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert!(Path::new(&staging).starts_with(to), "{staging}");
        let dl: String = e.db.conn.query_row("SELECT path FROM downloads", [], |r| r.get(0)).unwrap();
        assert!(Path::new(&dl).is_file() && Path::new(&dl).starts_with(to));
        let backup: String = e.db.conn.query_row("SELECT backup FROM deployed_files WHERE backup IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert!(Path::new(&backup).is_file() && Path::new(&backup).starts_with(to));
        assert_eq!(e.db.setting("library.dir").unwrap().unwrap(), to.to_string_lossy());
        let manifest = fs::read_to_string(Path::new(&staging).join(crate::deploy::MANIFEST)).unwrap();
        assert!(manifest.contains(&*to.to_string_lossy()) && !manifest.contains(&*e.lib.join("staging").to_string_lossy()), "{manifest}");
        assert!(verify(&e.db, &e.game).unwrap().is_empty(), "every deployed file is still the mod's");
    }

    #[test]
    fn moving_on_the_same_drive_is_a_rename_and_keeps_the_links() {
        let e = env();
        let to = e.root.join("Mods/PocketMods");
        let r = move_library(&e.db, &e.game, &e.lib, &to, &mut |_, _| {}).unwrap();
        assert_eq!(r.how, "renamed");
        assert!(!e.lib.exists() && to.join("staging/1-1/archive/pc/mod/coat.archive").is_file());
        assert!(r.rewritten >= 4);
        paths_point_at(&e, &to);
        // removing the mod still puts the game's own file back, from the moved backup
        crate::deploy::undeploy(&e.db, &e.game, 1).unwrap();
        assert_eq!(fs::read_to_string(e.game.join("r6/config/inputUserMappings.xml")).unwrap(), "vanilla");
    }

    #[test]
    fn moving_by_copy_links_the_game_to_the_new_files() {
        let e = env();
        let to = e.root.join("Elsewhere/PocketMods");
        let mut seen = 0;
        let r = move_library_as(&e.db, &e.game, &e.lib, &to, true, &mut |done, _| seen = done).unwrap();
        assert_eq!(r.how, "copied");
        assert!(seen > 0 && !e.lib.exists());
        assert_eq!(r.relinked, 2);
        paths_point_at(&e, &to);
        let src = to.join("staging/1-1/archive/pc/mod/coat.archive");
        assert!(same_file::is_same_file(&src, e.game.join("archive/pc/mod/coat.archive")).unwrap());
    }

    #[test]
    fn a_move_into_a_bad_place_is_refused() {
        let e = env();
        assert!(check_move(&e.game, &e.lib, &e.lib.join("sub")).problem.is_some());
        assert!(check_move(&e.game, &e.lib, &e.game.join("mods-lib")).problem.is_some());
        fs::create_dir_all(e.root.join("full")).unwrap();
        fs::write(e.root.join("full/x"), "x").unwrap();
        assert!(check_move(&e.game, &e.lib, &e.root.join("full")).problem.is_some());
        let ok = check_move(&e.game, &e.lib, &e.root.join("new"));
        assert!(ok.problem.is_none() && ok.quick && ok.same_drive_as_game && ok.files >= 3);
    }

    #[test]
    fn paths_rebase_only_inside_the_folder() {
        let from = Path::new("D:\\PocketMods");
        let to = Path::new("E:\\Games\\PocketMods");
        assert_eq!(rebase("D:\\PocketMods\\staging\\1-1", from, to).as_deref(), Some("E:\\Games\\PocketMods\\staging\\1-1"));
        assert_eq!(rebase("d:\\pocketmods\\x", from, to).as_deref(), Some("E:\\Games\\PocketMods\\x"));
        assert_eq!(rebase("D:\\PocketMods2\\x", from, to), None);
        assert_eq!(rebase("C:\\Vortex\\mods\\x", from, to), None);
    }

    #[test]
    fn a_copied_game_folder_is_linked_again() {
        let e = env();
        let moved = e.root.join("Moved Cyberpunk 2077");
        for f in walkdir::WalkDir::new(&e.game).into_iter().flatten().filter(|f| f.file_type().is_file()) {
            let dst = moved.join(f.path().strip_prefix(&e.game).unwrap());
            fs::create_dir_all(dst.parent().unwrap()).unwrap();
            fs::copy(f.path(), &dst).unwrap();
        }
        assert!(looks_like_moved_copy(&e.db, &moved).unwrap());
        assert!(!looks_like_moved_copy(&e.db, &e.root.join("elsewhere")).unwrap());
        assert_eq!(verify(&e.db, &moved).unwrap().len(), 2, "copies, not links");
        let r = relink_game(&e.db, &moved, false).unwrap();
        assert_eq!((r.checked, r.relinked, r.changed), (2, 2, 0));
        assert!(verify(&e.db, &moved).unwrap().is_empty());
        assert_eq!(relink_game(&e.db, &moved, false).unwrap().ok, 2);
    }
}
