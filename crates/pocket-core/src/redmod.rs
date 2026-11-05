//! REDmod: the game's own mod format, deployed by a tool that ships with the free REDmod DLC.
//!
//! REDmods live in `<game>/mods/<folder>/` with an info.json. After installing or reordering them, the game's
//! `tools/redmod/bin/redMod.exe deploy` step has to run, in load order.
//! UNVERIFIED: the exact command-line shape (`deploy -root=<game> -mod=<a> -mod=<b>`) follows the REDmod docs and
//! what other managers pass; it isn't confirmed against the tool here. The parser of its output is lenient.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RedmodInfo {
    pub folder: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

pub fn tool_path(game_root: &Path) -> PathBuf {
    crate::frameworks::game_path(game_root, "tools/redmod/bin/redMod.exe")
}

/// Every REDmod folder under `<game>/mods`, sorted by folder name.
pub fn list(game_root: &Path) -> Vec<RedmodInfo> {
    let dir = game_root.join("mods");
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<RedmodInfo> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let folder = e.file_name().to_string_lossy().into_owned();
            let info = std::fs::read_to_string(e.path().join("info.json")).ok()?;
            let v: serde_json::Value = serde_json::from_str(info.trim_start_matches('\u{feff}')).unwrap_or_default();
            Some(RedmodInfo {
                name: v["name"].as_str().unwrap_or(&folder).to_string(),
                version: v["version"].as_str().map(String::from),
                description: v["description"].as_str().map(String::from),
                folder,
            })
        })
        .collect();
    out.sort_by_key(|a| a.folder.to_lowercase());
    out
}

/// The saved order for the REDmods that are present, with new ones appended alphabetically.
pub fn merge_order(saved: &[String], present: &[String]) -> Vec<String> {
    let mut out: Vec<String> = saved.iter().filter(|s| present.iter().any(|p| p.eq_ignore_ascii_case(s))).cloned().collect();
    let mut new: Vec<String> = present.iter().filter(|p| !out.iter().any(|o| o.eq_ignore_ascii_case(p))).cloned().collect();
    new.sort_by_key(|s| s.to_lowercase());
    out.extend(new);
    out
}

/// The arguments for a deploy in the given load order (first loads first).
pub fn deploy_args(game_root: &Path, order: &[String]) -> Vec<String> {
    let mut args = vec!["deploy".to_string(), format!("-root={}", game_root.display())];
    args.extend(order.iter().map(|m| format!("-mod={m}")));
    args
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployOutcome {
    pub ok: bool,
    /// Plain-language problems found in the tool's output.
    pub problems: Vec<String>,
    /// The last lines of the tool's output, for the details view.
    pub log_tail: Vec<String>,
    pub at: i64,
}

/// Turns the tool's output into plain sentences. Unknown error lines are passed through.
pub fn explain(output: &str, exit_ok: bool) -> DeployOutcome {
    let mut problems = Vec::new();
    for line in output.lines() {
        let l = line.to_ascii_lowercase();
        let msg = if l.contains("script") && (l.contains("compil") || l.contains("error")) && l.contains("fail") {
            Some("A REDmod's scripts failed to compile. Disable REDmods one at a time to find it, or check each for a game-patch update.".to_string())
        } else if l.contains("could not find") || l.contains("not found") && l.contains("mod") {
            Some(format!("REDmod couldn't find a mod it was told to deploy: {}", line.trim()))
        } else if l.contains("access") && l.contains("denied") {
            Some("Windows blocked REDmod from writing to the game folder. Close the game and try again; if the game is under Program Files, see the permission note in Health.".to_string())
        } else if l.contains("info.json") && (l.contains("invalid") || l.contains("parse")) {
            Some(format!("A REDmod's info.json is broken: {}", line.trim()))
        } else if l.starts_with("error") || l.contains("[error]") || l.contains(" error:") {
            Some(line.trim().to_string())
        } else {
            None
        };
        if let Some(m) = msg {
            if !problems.contains(&m) {
                problems.push(m);
            }
        }
    }
    if !exit_ok && problems.is_empty() {
        problems.push("REDmod deploy stopped with an error and gave no reason. The log below has the tool's own output.".into());
    }
    let lines: Vec<String> = output.lines().map(String::from).collect();
    let tail = lines[lines.len().saturating_sub(40)..].to_vec();
    DeployOutcome { ok: exit_ok && problems.is_empty(), problems, log_tail: tail, at: crate::timefmt::now() }
}

/// Runs the deploy step. Windows only in practice; elsewhere the tool doesn't exist and this says so.
pub async fn run_deploy(game_root: &Path, order: &[String]) -> Result<DeployOutcome> {
    let tool = tool_path(game_root);
    if !tool.is_file() {
        return Err(Error::NotFound(
            "The REDmod tool isn't installed. Add the free REDmod DLC in Steam (Properties, DLC) or GOG Galaxy, then try again.".into(),
        ));
    }
    let mut cmd = tokio::process::Command::new(&tool);
    cmd.args(deploy_args(game_root, order)).current_dir(tool.parent().unwrap_or(game_root));
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console flashes up
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().await.map_err(|e| Error::io_at(e, &tool))?;
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok(explain(&text, out.status.success()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_redmods_with_info() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path().join("mods").join("Beta");
        std::fs::create_dir_all(&m).unwrap();
        std::fs::write(m.join("info.json"), "\u{feff}{\"name\":\"Beta Mod\",\"version\":\"1.0\"}").unwrap();
        std::fs::create_dir_all(dir.path().join("mods").join("NoInfo")).unwrap();
        let l = list(dir.path());
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "Beta Mod");
        assert_eq!(l[0].folder, "Beta");
    }

    #[test]
    fn order_keeps_saved_and_appends_new() {
        let saved = vec!["b".to_string(), "gone".into(), "a".into()];
        let present = vec!["a".to_string(), "c".into(), "B".into()];
        assert_eq!(merge_order(&saved, &present), vec!["b", "a", "c"]);
    }

    #[test]
    fn args_follow_order() {
        let a = deploy_args(Path::new("G"), &["x".into(), "y".into()]);
        assert_eq!(a, vec!["deploy", "-root=G", "-mod=x", "-mod=y"]);
    }

    #[test]
    fn explains_failures() {
        let o = explain("Compiling scripts...\nScript compilation failed\n", false);
        assert!(!o.ok);
        assert!(o.problems[0].contains("failed to compile"));
        let o = explain("Deploying...\nDone.\n", true);
        assert!(o.ok && o.problems.is_empty());
        let o = explain("", false);
        assert_eq!(o.problems.len(), 1);
    }
}
