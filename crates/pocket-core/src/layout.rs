//! Where each file in a mod archive goes in the game folder, by mod type.
//!
//! Well-packed Cyberpunk mods mirror the game folder (`archive/`, `bin/`, `r6/`, `red4ext/`, `mods/`, `engine/`),
//! often inside one wrapper folder. Loosely packed ones get placed by what they contain:
//!   .archive / .xl            → archive/pc/mod/
//!   .reds                     → r6/scripts/<mod>/
//!   a folder with init.lua    → bin/x64/plugins/cyber_engine_tweaks/mods/<folder>/   (a CET mod)
//!   a folder with info.json   → mods/<folder>/                                       (a REDmod)
//!   .yaml / .yml / .tweak     → r6/tweaks/<mod>/
//! Readmes, images and other documents are left out. Anything else is reported back, not guessed.

use crate::classify::ModKind;
use crate::{Error, Result};
use serde::Serialize;
use std::collections::BTreeMap;

/// Folders at the top of the game directory that mods write into.
const ROOTS: &[&str] = &["archive", "bin", "engine", "r6", "red4ext", "mods"];
const DOC_EXT: &[&str] = &["txt", "md", "pdf", "png", "jpg", "jpeg", "gif", "webp", "bmp", "url", "html", "htm", "rtf", "docx", "psd"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// Path inside the extracted archive.
    pub from: String,
    /// Game-relative destination, forward slashes.
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPlan {
    pub placements: Vec<Placement>,
    /// Documents we chose not to install.
    pub skipped_docs: Vec<String>,
    /// Files we could not place. Installing proceeds without them, and the UI lists them.
    pub unplaced: Vec<String>,
    pub kind: ModKind,
    /// REDmod folder names this mod adds under mods/ (they go into the load order).
    pub redmods: Vec<String>,
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

fn sanitize_folder(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "mod".into()
    } else {
        s
    }
}

/// Plans an install from the archive's file list. `mod_name` names folders for loose scripts and tweaks.
pub fn plan(files: &[String], mod_name: &str) -> Result<LayoutPlan> {
    let files: Vec<String> = files.iter().map(|f| f.replace('\\', "/").trim_start_matches('/').to_string()).collect();
    if files.iter().any(|f| f.to_ascii_lowercase().ends_with("fomod/moduleconfig.xml")) {
        return Err(Error::Layout(
            "This mod has an installer with options (a FOMOD). Pocket Mods doesn't run those yet; pick the variant you want \
             on the mod's Files tab, or install it by hand."
                .into(),
        ));
    }
    let folder = sanitize_folder(mod_name);
    let mut placements = Vec::new();
    let mut skipped_docs = Vec::new();
    let mut unplaced = Vec::new();

    // Folders that hold an init.lua (CET) or info.json (REDmod) claim everything under them.
    let mut claimed: BTreeMap<String, String> = BTreeMap::new(); // archive dir prefix -> game dir
    for f in &files {
        let lower = f.to_ascii_lowercase();
        let parts: Vec<&str> = f.split('/').collect();
        if root_index(&parts).is_some() {
            continue; // already mirrors the game folder
        }
        let name = parts.last().copied().unwrap_or("").to_ascii_lowercase();
        if (name == "init.lua" || name == "info.json") && parts.len() >= 2 {
            let dir = parts[..parts.len() - 1].join("/");
            let dir_name = parts[parts.len() - 2];
            let target = if name == "init.lua" {
                format!("bin/x64/plugins/cyber_engine_tweaks/mods/{dir_name}")
            } else {
                format!("mods/{dir_name}")
            };
            // an info.json only means REDmod when the folder also has archives/ or a recognisable REDmod part
            if name == "info.json" && !files.iter().any(|g| {
                let gl = g.to_ascii_lowercase();
                gl.starts_with(&format!("{}/", dir.to_ascii_lowercase())) && (gl.contains("/archives/") || gl.contains("/scripts/") || gl.contains("/tweaks/") || gl.contains("/customsounds/"))
            }) {
                continue;
            }
            claimed.entry(dir).or_insert(target);
            let _ = lower;
        }
    }

    for f in &files {
        let parts: Vec<&str> = f.split('/').collect();
        let name = parts.last().copied().unwrap_or("");
        let ext = ext_of(name);
        if let Some(i) = root_index(&parts) {
            placements.push(Placement { from: f.clone(), to: parts[i..].join("/") });
            continue;
        }
        if let Some((dir, target)) = claimed.iter().find(|(d, _)| f.starts_with(&format!("{d}/"))) {
            let rest = &f[dir.len() + 1..];
            placements.push(Placement { from: f.clone(), to: format!("{target}/{rest}") });
            continue;
        }
        let to = match ext.as_str() {
            "archive" | "xl" => Some(format!("archive/pc/mod/{name}")),
            "reds" => Some(format!("r6/scripts/{folder}/{name}")),
            "yaml" | "yml" | "tweak" => Some(format!("r6/tweaks/{folder}/{name}")),
            _ => None,
        };
        match to {
            Some(to) => placements.push(Placement { from: f.clone(), to }),
            None if DOC_EXT.contains(&ext.as_str()) || name.eq_ignore_ascii_case("license") => skipped_docs.push(f.clone()),
            None => unplaced.push(f.clone()),
        }
    }

    if placements.is_empty() {
        return Err(Error::Layout(if unplaced.is_empty() {
            "The archive only holds documents; there is nothing to install.".into()
        } else {
            format!(
                "Pocket Mods couldn't tell where these files go: {}. Check the mod's install notes.",
                unplaced.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            )
        }));
    }
    // two archive files with the same name flattened into archive/pc/mod would collide
    let mut seen = BTreeMap::new();
    for p in &placements {
        if let Some(prev) = seen.insert(p.to.to_ascii_lowercase(), &p.from) {
            return Err(Error::Layout(format!(
                "Two files in this archive would land on the same place ({}): {prev} and {}. It probably holds several \
                 variants; pick one on the mod's Files tab.",
                p.to, p.from
            )));
        }
    }

    let redmods: Vec<String> = {
        let mut v: Vec<String> = placements
            .iter()
            .filter_map(|p| p.to.strip_prefix("mods/").and_then(|r| r.split('/').next()).map(String::from))
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let kind = kind_of(&placements);
    Ok(LayoutPlan { placements, skipped_docs, unplaced, kind, redmods })
}

/// Index of the first path part that is a game root folder, when the rest of the path fits under it.
fn root_index(parts: &[&str]) -> Option<usize> {
    for (i, p) in parts.iter().enumerate().take(parts.len().saturating_sub(1)) {
        let lp = p.to_ascii_lowercase();
        if ROOTS.contains(&lp.as_str()) {
            let next = parts.get(i + 1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let fits = match lp.as_str() {
                "archive" => next == "pc",
                "bin" => next == "x64",
                "r6" => ["scripts", "tweaks", "config", "input", "cache"].contains(&next.as_str()),
                "engine" => ["config", "tools"].contains(&next.as_str()),
                "red4ext" => next == "plugins" || parts.len() == i + 2,
                "mods" => parts.len() >= i + 3,
                _ => false,
            };
            if fits {
                return Some(i);
            }
        }
    }
    None
}

fn kind_of(placements: &[Placement]) -> ModKind {
    let has = |prefix: &str| placements.iter().any(|p| p.to.to_ascii_lowercase().starts_with(prefix));
    if has("mods/") {
        ModKind::Redmod
    } else if has("r6/scripts/") || has("r6/tweaks/") || has("red4ext/") || has("bin/x64/plugins/") || has("engine/") {
        ModKind::Script
    } else if has("archive/") {
        ModKind::Archive
    } else {
        ModKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }
    fn to(p: &LayoutPlan) -> Vec<&str> {
        p.placements.iter().map(|x| x.to.as_str()).collect()
    }

    #[test]
    fn mirrored_layout_strips_wrapper() {
        let p = plan(&v(&["Cool Mod v2/archive/pc/mod/cool.archive", "Cool Mod v2/README.txt"]), "Cool").unwrap();
        assert_eq!(to(&p), vec!["archive/pc/mod/cool.archive"]);
        assert_eq!(p.skipped_docs, vec!["Cool Mod v2/README.txt"]);
        assert_eq!(p.kind, ModKind::Archive);
    }

    #[test]
    fn loose_files_are_placed_by_type() {
        let p = plan(&v(&["cool.archive", "thing.reds", "tweak.yaml"]), "Cool Mod!").unwrap();
        assert_eq!(to(&p), vec!["archive/pc/mod/cool.archive", "r6/scripts/Cool Mod_/thing.reds", "r6/tweaks/Cool Mod_/tweak.yaml"]);
        assert_eq!(p.kind, ModKind::Script);
    }

    #[test]
    fn cet_folder_goes_under_cet_mods() {
        let p = plan(&v(&["Wrapper/MyCetMod/init.lua", "Wrapper/MyCetMod/modules/a.lua"]), "x").unwrap();
        assert_eq!(
            to(&p),
            vec!["bin/x64/plugins/cyber_engine_tweaks/mods/MyCetMod/init.lua", "bin/x64/plugins/cyber_engine_tweaks/mods/MyCetMod/modules/a.lua"]
        );
    }

    #[test]
    fn redmod_folder_goes_under_mods() {
        let p = plan(&v(&["MyRedmod/info.json", "MyRedmod/archives/a.archive"]), "x").unwrap();
        assert_eq!(to(&p), vec!["mods/MyRedmod/info.json", "mods/MyRedmod/archives/a.archive"]);
        assert_eq!(p.kind, ModKind::Redmod);
        assert_eq!(p.redmods, vec!["MyRedmod"]);
        // already mirrored
        let p = plan(&v(&["mods/Other/info.json", "mods/Other/archives/b.archive"]), "x").unwrap();
        assert_eq!(p.redmods, vec!["Other"]);
    }

    #[test]
    fn red4ext_and_bin_are_recognised() {
        let p = plan(&v(&["red4ext/plugins/X/X.dll", "bin/x64/plugins/cyber_engine_tweaks/mods/y/init.lua"]), "x").unwrap();
        assert_eq!(to(&p), vec!["red4ext/plugins/X/X.dll", "bin/x64/plugins/cyber_engine_tweaks/mods/y/init.lua"]);
    }

    #[test]
    fn unknown_files_are_reported_not_guessed() {
        let p = plan(&v(&["a.archive", "mystery.bin"]), "x").unwrap();
        assert_eq!(p.unplaced, vec!["mystery.bin"]);
        assert!(plan(&v(&["mystery.bin"]), "x").is_err());
        assert!(plan(&v(&["readme.md"]), "x").is_err());
    }

    #[test]
    fn variants_that_collide_are_refused() {
        let e = plan(&v(&["Option A/x.archive", "Option B/x.archive"]), "x").unwrap_err();
        assert!(e.to_string().contains("same place"));
    }

    #[test]
    fn fomod_is_refused_clearly() {
        let e = plan(&v(&["fomod/ModuleConfig.xml", "A/archive/pc/mod/a.archive"]), "x").unwrap_err();
        assert!(e.to_string().contains("FOMOD"));
    }
}
