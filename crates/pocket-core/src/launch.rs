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
