//! The game's REDmod deploy step (`tools/redmod/bin/redMod.exe deploy`), which compiles REDmods in load order.
//! UNVERIFIED: the argument form `deploy -root="<game>" -mod="<name>" …` (one -mod per mod, in load order) follows
//! CD PROJEKT RED's REDmod documentation as remembered; check against `redMod.exe deploy -help` on a real install.

use super::game::REDMOD_EXE;
use crate::error::{Error, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedmodResult {
    pub ok: bool,
    pub mods: Vec<String>,
    /// Plain-language problems, one per line of the tool's errors.
    pub problems: Vec<String>,
    /// The tool's output, last 200 lines.
    pub log: String,
}

pub fn deploy_args(game: &Path, mods: &[String]) -> Vec<String> {
    let mut a = vec!["deploy".to_string(), format!("-root={}", game.display())];
    a.extend(mods.iter().map(|m| format!("-mod={m}")));
    a
}

/// Turn the tool's error lines into sentences a player can act on.
pub fn plain_problems(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let l = line.to_lowercase();
        let is_err = (l.contains("error") && !l.contains("0 error") && !l.contains("no error")) || l.contains("failed") || l.contains("exception");
        if !is_err { continue; }
        let text = line.trim();
        let plain = if l.contains("access is denied") || l.contains("unauthorized") {
            "Windows blocked REDmod from writing to the game folder. Close the game, and if it is under Program Files, see Settings for the fix.".to_string()
        } else if l.contains("script") && (l.contains("compil") || l.contains("syntax")) {
            format!("A REDmod script did not compile. A mod may be outdated for this game patch. ({text})")
        } else if l.contains("info.json") {
            format!("A REDmod has a broken or missing info.json. ({text})")
        } else if l.contains("not found") || l.contains("cannot find") || l.contains("could not find") {
            format!("REDmod could not find a file it needs. Reinstall the mod named in the message. ({text})")
        } else {
            text.to_string()
        };
        if !out.contains(&plain) { out.push(plain); }
    }
    out
}

pub fn run_deploy(game: &Path, mods: &[String]) -> Result<RedmodResult> {
    let exe = game.join(REDMOD_EXE);
    if !exe.exists() {
        return Err(Error::Invalid("The REDmod tool is not installed. It is a free DLC: add \"REDmod\" in Steam (DLC) or GOG Galaxy, then try again.".into()));
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(deploy_args(game, mods)).current_dir(exe.parent().unwrap_or(game));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| Error::Invalid(format!("Could not start REDmod: {e}")))?;
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let problems = plain_problems(&text);
    let log: Vec<&str> = text.lines().collect();
    let log = log[log.len().saturating_sub(200)..].join("\n");
    Ok(RedmodResult { ok: out.status.success() && problems.is_empty(), mods: mods.to_vec(), problems, log })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn args_keep_load_order() {
        let a = deploy_args(Path::new("D:/Games/Cyberpunk 2077"), &["B".into(), "A".into()]);
        assert_eq!(a, ["deploy", "-root=D:/Games/Cyberpunk 2077", "-mod=B", "-mod=A"]);
    }
    #[test]
    fn problems_in_plain_words() {
        let p = plain_problems("Deploying 3 mods\n0 errors in pass 1\n[ERROR] Script compilation failed: unexpected token in mods/X/scripts/a.script\nSystem.UnauthorizedAccessException: Access is denied\nDone");
        assert_eq!(p.len(), 2);
        assert!(p[0].starts_with("A REDmod script did not compile"));
        assert!(p[1].starts_with("Windows blocked REDmod"));
    }
}
