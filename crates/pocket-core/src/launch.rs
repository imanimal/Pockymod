//! Starting the game. With REDmods installed it must start with `-modded`, or the game ignores them (a common
//! "REDmods do nothing" report with other managers).
//! UNVERIFIED: that a Steam copy started from its executable keeps the argument when Steam relaunches it.

use crate::cyberpunk::game::EXE;
use crate::error::{Error, Result};
use std::path::Path;

pub fn launch(game: &Path, modded: bool) -> Result<()> {
    let exe = game.join(EXE);
    if !exe.exists() { return Err(Error::Invalid("Cyberpunk2077.exe was not found. Check the game folder in Settings.".into())); }
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

/// Take every deployed mod out of the game, remembering which. Returns how many came out and how many files the
/// app does not track are still in mod folders (those it cannot take out).
pub fn vanilla_prepare(db: &crate::db::Db, game: &Path) -> Result<(usize, usize)> {
    if db.setting(VANILLA_KEY)?.is_some() { return Err(Error::Invalid("The game is already running without mods. Close it first; your mods go back by themselves.".into())); }
    let ids: Vec<i64> = {
        let mut st = db.conn.prepare("SELECT DISTINCT install_id FROM deployed_files")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    db.set_setting(VANILLA_KEY, &serde_json::to_string(&ids).unwrap_or_default())?;
    for id in &ids { crate::install::undeploy_any(db, game, *id)?; }
    let left = ["archive/pc/mod", "mods", "r6/scripts", "r6/tweaks", "red4ext/plugins", "bin/x64/plugins"].iter()
        .filter_map(|d| std::fs::read_dir(game.join(d)).ok()).map(|rd| rd.flatten().count()).sum();
    Ok((ids.len(), left))
}

/// Put back what `vanilla_prepare` took out. Safe to call when nothing is pending.
pub fn vanilla_restore(db: &crate::db::Db, game: &Path, library: &Path, opts: &crate::deploy::DeployOptions) -> Result<usize> {
    let Some(saved) = db.setting(VANILLA_KEY)? else { return Ok(0) };
    let ids: Vec<i64> = serde_json::from_str(&saved).unwrap_or_default();
    let mut n = 0;
    for id in ids {
        let exists: bool = db.conn.query_row("SELECT EXISTS(SELECT 1 FROM installs WHERE id = ?1)", rusqlite::params![id], |r| r.get(0))?;
        if exists && crate::install::redeploy_any(db, game, library, id, opts).is_ok() { n += 1; }
    }
    db.conn.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![VANILLA_KEY])?;
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
            options: Default::default(), fomod_choices: None, origin: None, keep_existing: false, file_name: None, replace_only: None };
        crate::install::install(&db, &game, &lib, &req).unwrap();
        let (out, _) = vanilla_prepare(&db, &game).unwrap();
        assert_eq!(out, 1);
        assert!(!game.join("archive/pc/mod/a.archive").exists());
        assert!(vanilla_prepare(&db, &game).is_err(), "not twice");
        assert_eq!(vanilla_restore(&db, &game, &lib, &Default::default()).unwrap(), 1);
        assert!(game.join("archive/pc/mod/a.archive").exists());
        assert_eq!(vanilla_restore(&db, &game, &lib, &Default::default()).unwrap(), 0);
    }
}
