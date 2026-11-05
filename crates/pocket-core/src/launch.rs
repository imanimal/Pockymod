//! Starting the game. With REDmods installed it must start with `-modded`, or the game ignores them (a common
//! "REDmods do nothing" report with other managers).
//! UNVERIFIED: that a Steam copy started from its executable keeps the argument when Steam relaunches it.

use crate::cyberpunk::game::EXE;
use crate::error::{Error, Result};
use std::path::Path;

pub fn launch(game: &Path, modded: bool) -> Result<()> {
    let exe = game.join(EXE);
    if !exe.exists() { return Err(Error::Invalid("Cyberpunk2077.exe not found. Check the game folder in Settings".into())); }
    let mut cmd = std::process::Command::new(&exe);
    if modded { cmd.arg("-modded"); }
    cmd.current_dir(exe.parent().unwrap_or(game));
    cmd.spawn().map_err(|e| Error::Invalid(format!("Could not start the game: {e}")))?;
    Ok(())
}

pub fn has_redmods(db: &crate::db::Db) -> Result<bool> {
    let active = crate::install::active_profile(db)?;
    let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM installs i JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1 WHERE pm.enabled = 1 AND i.mod_type = 'redmod'",
        rusqlite::params![active], |r| r.get(0))?;
    Ok(n > 0)
}

// ── starting without mods ──
// Every mod this app tracks comes out of the game (hard links, so it is quick), the game starts, and once it has
// closed they all go back. Which mods to put back is saved first, so a crash or closing the app in between loses
// nothing: the next start puts them back.

pub const VANILLA_KEY: &str = "launch.vanilla";

/// Take every deployed mod out of the game, remembering which and in what order. Returns how many came out and how
/// many files the app does not track are still in mod folders (those it cannot take out). redscript's compiled
/// scripts are swapped for the game's own while the mods are out, or the vanilla game would load modded scripts.
pub fn vanilla_prepare(db: &crate::db::Db, game: &Path) -> Result<(usize, usize)> {
    if db.setting(VANILLA_KEY)?.is_some() { return Err(Error::Invalid("The game is already running without mods. Close it first and your mods come back".into())); }
    // oldest deploy first, so putting them back in this order gives each path to the mod that won it last
    let ids: Vec<i64> = {
        let mut st = db.conn.prepare("SELECT install_id FROM deployed_files GROUP BY install_id ORDER BY MAX(deployed_at), install_id")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    db.set_setting(VANILLA_KEY, &serde_json::to_string(&ids).unwrap_or_default())?;
    // taking out a mod that won a clash gives the path back to the one it took it from: go round until none is left
    for _ in 0..8 {
        let left: Vec<i64> = {
            let mut st = db.conn.prepare("SELECT DISTINCT install_id FROM deployed_files")?;
            let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            v
        };
        if left.is_empty() { break; }
        for id in left { crate::install::undeploy_any(db, game, id)?; }
    }
    swap_scripts(game, true);
    let left = ["archive/pc/mod", "mods", "r6/scripts", "r6/tweaks", "red4ext/plugins", "bin/x64/plugins"].iter()
        .filter_map(|d| std::fs::read_dir(game.join(d)).ok()).map(|rd| rd.flatten().count()).sum();
    Ok((ids.len(), left))
}

/// redscript keeps the game's own compiled scripts as `final.redscripts.bk`. Going vanilla puts those in place and
/// keeps the modded file aside; coming back swaps them again.
fn swap_scripts(game: &Path, vanilla: bool) {
    let cache = game.join("r6/cache");
    let (fin, bk, held) = (cache.join("final.redscripts"), cache.join("final.redscripts.bk"), cache.join("final.redscripts.pocket-modded"));
    if vanilla {
        if bk.is_file() && fin.is_file() && !held.exists() && std::fs::rename(&fin, &held).is_ok() && std::fs::copy(&bk, &fin).is_err() {
            let _ = std::fs::rename(&held, &fin);
        }
    } else if held.is_file() {
        let _ = std::fs::remove_file(&fin);
        let _ = std::fs::rename(&held, &fin);
    }
}

/// Put back what `vanilla_prepare` took out, in the same order and with the clashes won as before. Mods turned off
/// meanwhile stay out; any that fail stay listed, so a later call (or the next start) tries again.
pub fn vanilla_restore(db: &crate::db::Db, game: &Path, library: &Path, opts: &crate::deploy::DeployOptions) -> Result<usize> {
    let Some(saved) = db.setting(VANILLA_KEY)? else { return Ok(0) };
    let ids: Vec<i64> = serde_json::from_str(&saved).unwrap_or_default();
    let active = crate::install::active_profile(db)?;
    let win = crate::deploy::DeployOptions { take_conflicts: true, ..opts.clone() };
    let mut n = 0;
    let mut failed = Vec::new();
    for id in ids {
        let on: bool = db.conn.query_row("SELECT COALESCE((SELECT enabled FROM profile_mods WHERE install_id = ?1 AND profile_id = ?2), 0)", rusqlite::params![id, active], |r| r.get(0))?;
        if !on { continue; }
        match crate::install::redeploy_any(db, game, library, id, &win) {
            Ok(crate::deploy::DeployOutcome::Deployed { .. }) => n += 1,
            _ => failed.push(id),
        }
    }
    swap_scripts(game, false);
    if failed.is_empty() { db.conn.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![VANILLA_KEY])?; }
    else { db.set_setting(VANILLA_KEY, &serde_json::to_string(&failed).unwrap_or_default())?; }
    Ok(n)
}

/// Is the game running? (Windows: the task list; elsewhere always false.)
pub fn game_running() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("tasklist").args(["/FI", "IMAGENAME eq Cyberpunk2077.exe", "/NH", "/FO", "CSV"]).creation_flags(NO_WINDOW).output();
        out.map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("cyberpunk2077.exe")).unwrap_or(false)
    }
    #[cfg(not(windows))]
    { false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn vanilla_takes_mods_out_and_puts_them_back() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        let lib = dir.path().join("lib");
        let a = dir.path().join("a.zip");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(fs::File::create(&a).unwrap());
            z.start_file("archive/pc/mod/a.archive", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"a").unwrap();
            z.finish().unwrap();
        }
        let db = crate::db::Db::memory().unwrap();
        let req = crate::install::InstallRequest { mod_id: 1, file_id: 1, name: "A".into(), version: "1".into(), archive: a, accept_unknown: false, replace_older: true,
            options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None, same_file_only: false };
        crate::install::install(&db, &game, &lib, &req).unwrap();
        let (out, _) = vanilla_prepare(&db, &game).unwrap();
        assert_eq!(out, 1);
        assert!(!game.join("archive/pc/mod/a.archive").exists());
        assert!(vanilla_prepare(&db, &game).is_err(), "not twice");
        assert_eq!(vanilla_restore(&db, &game, &lib, &Default::default()).unwrap(), 1);
        assert!(game.join("archive/pc/mod/a.archive").exists());
        assert_eq!(vanilla_restore(&db, &game, &lib, &Default::default()).unwrap(), 0);
    }

    fn zip(path: &Path, files: &[(&str, &str)]) {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, b) in files { z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap(); z.write_all(b.as_bytes()).unwrap(); }
        z.finish().unwrap();
    }

    #[test]
    fn vanilla_keeps_clash_winners_and_swaps_compiled_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("bin/x64")).unwrap();
        fs::create_dir_all(game.join("r6/cache")).unwrap();
        fs::write(game.join("r6/cache/final.redscripts"), "modded").unwrap();
        fs::write(game.join("r6/cache/final.redscripts.bk"), "vanilla").unwrap();
        let lib = dir.path().join("lib");
        let db = crate::db::Db::memory().unwrap();
        let mut ids = Vec::new();
        for (m, body) in [(1u32, "A"), (2, "B")] {
            let a = dir.path().join(format!("{m}.zip"));
            zip(&a, &[("archive/pc/mod/x.archive", body), (if m == 1 { "archive/pc/mod/a.archive" } else { "archive/pc/mod/b.archive" }, body)]);
            let req = crate::install::InstallRequest { mod_id: m, file_id: m, name: format!("M{m}"), version: "1".into(), archive: a, accept_unknown: false, replace_older: true,
                options: crate::deploy::DeployOptions { take_conflicts: true, ..Default::default() }, fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None, same_file_only: false };
            if let crate::install::InstallOutcome::Installed { install_id, .. } = crate::install::install(&db, &game, &lib, &req).unwrap() { ids.push(install_id); }
        }
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/x.archive")).unwrap(), "B");
        vanilla_prepare(&db, &game).unwrap();
        assert!(!game.join("archive/pc/mod/x.archive").exists(), "nothing of either mod is left in");
        assert_eq!(fs::read_to_string(game.join("r6/cache/final.redscripts")).unwrap(), "vanilla");
        assert_eq!(vanilla_restore(&db, &game, &lib, &Default::default()).unwrap(), 2);
        assert_eq!(fs::read_to_string(game.join("archive/pc/mod/x.archive")).unwrap(), "B", "the clash goes to the mod that had won it");
        assert!(game.join("archive/pc/mod/b.archive").exists() && game.join("archive/pc/mod/a.archive").exists());
        assert_eq!(fs::read_to_string(game.join("r6/cache/final.redscripts")).unwrap(), "modded");
    }
}
