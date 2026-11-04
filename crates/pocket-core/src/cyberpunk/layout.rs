//! Where Cyberpunk 2077 mod files go, per mod type. Used to plan an install from an archive's file list and to
//! classify a mod from the file list Nexus has indexed.
//!
//! Game-relative destinations (from the community install docs; UNVERIFIED against every mod layout):
//! - `.archive`, `.xl` (ArchiveXL)            → `archive/pc/mod/`
//! - REDmod (`<name>/info.json` + content)     → `mods/<name>/`
//! - redscript `.reds`                          → `r6/scripts/<mod>/`
//! - TweakXL `.yaml`/`.yml`/`.tweak`             → `r6/tweaks/<mod>/`
//! - Cyber Engine Tweaks Lua (`init.lua`)        → `bin/x64/plugins/cyber_engine_tweaks/mods/<mod>/`
//! - RED4ext plugins                             → `red4ext/plugins/<mod>/`
//! - anything already under a game folder (`archive/`, `bin/`, `engine/`, `mods/`, `r6/`, `red4ext/`) keeps its path.

use super::frameworks::Framework;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModType { Redmod, Archive, Script, Framework, Mixed, Unknown }

impl ModType {
    pub fn as_str(&self) -> &'static str {
        match self { ModType::Redmod => "redmod", ModType::Archive => "archive", ModType::Script => "script", ModType::Framework => "framework", ModType::Mixed => "mixed", ModType::Unknown => "unknown" }
    }
    pub fn parse(s: &str) -> ModType {
        match s { "redmod" => ModType::Redmod, "archive" => ModType::Archive, "script" => ModType::Script, "framework" => ModType::Framework, "mixed" => ModType::Mixed, _ => ModType::Unknown }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FileKind { Archive, ArchiveXl, Redmod, Redscript, Tweak, CetMod, Red4extPlugin, FrameworkCore, GameFile, Doc, Unknown }

/// One file to put in the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// Path inside the archive, as listed.
    pub source: String,
    /// Path relative to the game folder, with forward slashes.
    pub target: String,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub files: Vec<Placement>,
    /// Docs and images left out of the deploy.
    pub skipped: Vec<String>,
    /// Files with no known place. The install asks before going on.
    pub unknown: Vec<String>,
    pub mod_type: Option<ModType>,
    /// Frameworks the files show the mod uses.
    pub uses: BTreeSet<Framework>,
    pub redmods: Vec<String>,
    pub needs_redmod_deploy: bool,
    pub warnings: Vec<String>,
}

const ROOTS: &[&str] = &["archive", "bin", "engine", "mods", "r6", "red4ext"];
const DOC_EXT: &[&str] = &["txt", "md", "pdf", "png", "jpg", "jpeg", "gif", "webp", "url", "html", "htm", "rtf", "doc", "docx", "psd", "bmp"];

fn norm(p: &str) -> String {
    let p = p.replace('\\', "/");
    let p = p.trim_start_matches("./").trim_start_matches('/');
    p.split('/').filter(|s| !s.is_empty() && *s != ".").collect::<Vec<_>>().join("/")
}

fn ext(p: &str) -> String {
    p.rsplit('/').next().and_then(|f| f.rsplit_once('.')).map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

fn file_name(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }

/// Index of the first path segment that starts a game folder layout, if any.
fn anchor(parts: &[&str]) -> Option<usize> {
    let lower: Vec<String> = parts.iter().map(|s| s.to_lowercase()).collect();
    (0..lower.len().saturating_sub(1)).find(|&i| {
        let s = lower[i].as_str();
        let next = lower.get(i + 1).map(String::as_str).unwrap_or("");
        match s {
            "archive" => next == "pc",
            "bin" => next == "x64",
            // audioware keeps its .yml files in r6/audioware (they are not TweakXL tweaks)
            "r6" => matches!(next, "scripts" | "tweaks" | "config" | "input" | "cache" | "audioware" | "storages"),
            "red4ext" => true,
            "engine" => matches!(next, "config" | "tools"),
            // REDmod: mods/<name>/... with an info.json somewhere at mods/<name>/info.json (checked by the caller)
            "mods" => lower.len() > i + 2,
            _ => false,
        }
    })
}

fn kind_of_target(t: &str) -> FileKind {
    let l = t.to_lowercase();
    let e = ext(&l);
    if DOC_EXT.contains(&e.as_str()) && !l.starts_with("mods/") && !l.starts_with("bin/x64/plugins/cyber_engine_tweaks/") { return FileKind::Doc; }
    if l.starts_with("archive/pc/") { return if e == "xl" { FileKind::ArchiveXl } else { FileKind::Archive }; }
    if l.starts_with("mods/") { return FileKind::Redmod; }
    if l.starts_with("r6/scripts/") { return FileKind::Redscript; }
    if l.starts_with("r6/tweaks/") { return FileKind::Tweak; }
    if l.starts_with("bin/x64/plugins/cyber_engine_tweaks/mods/") { return FileKind::CetMod; }
    if l == "red4ext/red4ext.dll" || l == "bin/x64/winmm.dll" || l == "bin/x64/version.dll" || l == "bin/x64/global.ini"
        || l.starts_with("bin/x64/plugins/cyber_engine_tweaks") || l.ends_with(".asi") || l.starts_with("engine/tools/") || l == "engine/config/base/scripts.ini" {
        return FileKind::FrameworkCore;
    }
    if l.starts_with("red4ext/plugins/") { return FileKind::Red4extPlugin; }
    FileKind::GameFile
}

/// Plan where each file of an archive goes. `mod_name` names folders for loose scripts, tweaks and CET mods.
pub fn plan_install(entries: &[String], mod_name: &str) -> InstallPlan {
    let folder = safe_folder(mod_name);
    let files: Vec<String> = entries.iter().map(|e| norm(e)).filter(|e| !e.is_empty() && !e.ends_with('/')).collect();
    let mut plan = InstallPlan::default();

    // REDmod roots: any folder holding info.json, whose content is REDmod-shaped
    let mut redmod_roots: BTreeSet<String> = BTreeSet::new();
    for f in &files {
        if file_name(f).eq_ignore_ascii_case("info.json") {
            let dir = f.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            let has_content = files.iter().any(|g| {
                g.len() > dir.len() + 1 && g.starts_with(&dir) && {
                    let rest = g[if dir.is_empty() { 0 } else { dir.len() + 1 }..].to_lowercase();
                    rest.starts_with("archives/") || rest.starts_with("scripts/") || rest.starts_with("tweaks/") || rest.starts_with("customsounds/")
                }
            });
            if has_content { redmod_roots.insert(dir); }
        }
    }

    // CET mod roots: folders holding init.lua that are not already under the CET mods folder
    let cet_roots: BTreeSet<String> = files.iter()
        .filter(|f| file_name(f).eq_ignore_ascii_case("init.lua") && !f.to_lowercase().contains("cyber_engine_tweaks/mods/"))
        .map(|f| f.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default())
        .collect();

    for f in &files {
        let parts: Vec<&str> = f.split('/').collect();
        let e = ext(f);
        let lower = f.to_lowercase();

        // 1. REDmod folder
        if let Some(root) = redmod_roots.iter().filter(|r| r.is_empty() || f.starts_with(&format!("{r}/"))).max_by_key(|r| r.len()) {
            let name = if root.is_empty() { folder.clone() } else { root.rsplit('/').next().unwrap_or(root).to_string() };
            let rel = if root.is_empty() { f.as_str() } else { &f[root.len() + 1..] };
            if !plan.redmods.contains(&name) { plan.redmods.push(name.clone()); }
            push(&mut plan, f, format!("mods/{name}/{rel}"));
            continue;
        }
        // 2. already laid out under a game folder
        if let Some(i) = anchor(&parts) {
            let rel = parts[i..].join("/");
            // a loose "mods/<x>/..." without info.json is not REDmod; treat the rest by extension below
            if !rel.to_lowercase().starts_with("mods/") {
                push(&mut plan, f, rel);
                continue;
            }
        }
        if ROOTS.iter().any(|r| lower == *r) { continue; }
        // 3. CET mod folder without its path
        if let Some(root) = cet_roots.iter().filter(|r| r.is_empty() || f.starts_with(&format!("{r}/"))).max_by_key(|r| r.len()) {
            let name = if root.is_empty() { folder.clone() } else { root.rsplit('/').next().unwrap_or(root).to_string() };
            let rel = if root.is_empty() { f.as_str() } else { &f[root.len() + 1..] };
            push(&mut plan, f, format!("bin/x64/plugins/cyber_engine_tweaks/mods/{name}/{rel}"));
            continue;
        }
        // 4. loose files by extension
        match e.as_str() {
            "archive" | "xl" => push(&mut plan, f, format!("archive/pc/mod/{}", file_name(f))),
            "reds" => push(&mut plan, f, format!("r6/scripts/{folder}/{}", file_name(f))),
            "yaml" | "yml" | "tweak" => push(&mut plan, f, format!("r6/tweaks/{folder}/{}", file_name(f))),
            _ if DOC_EXT.contains(&e.as_str()) => plan.skipped.push(f.clone()),
            _ => plan.unknown.push(f.clone()),
        }
    }

    // conflicts inside the archive itself (two variants in one download)
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for p in &plan.files { *seen.entry(p.target.to_lowercase()).or_default() += 1; }
    let dupes: Vec<_> = seen.into_iter().filter(|(_, n)| *n > 1).map(|(t, _)| t).collect();
    if !dupes.is_empty() {
        plan.warnings.push(format!("The archive has {} files that land on the same path (it may hold several variants). Pick one variant before installing.", dupes.len()));
    }
    if !plan.unknown.is_empty() {
        plan.warnings.push(format!("{} files have no known place in the game folder.", plan.unknown.len()));
    }

    for p in &plan.files {
        match p.kind {
            FileKind::Redscript => { plan.uses.insert(Framework::Redscript); }
            FileKind::Tweak => { plan.uses.insert(Framework::TweakXl); }
            FileKind::CetMod => { plan.uses.insert(Framework::Cet); }
            FileKind::ArchiveXl => { plan.uses.insert(Framework::ArchiveXl); }
            FileKind::Red4extPlugin => { plan.uses.insert(Framework::Red4ext); }
            _ => {}
        }
    }
    plan.needs_redmod_deploy = !plan.redmods.is_empty();
    plan.mod_type = Some(type_of(&plan.files));
    plan
}

/// A game-relative path that stays inside the folder: no `..`, no root, no drive letter or stream (`:`).
pub fn safe_target(p: &str) -> bool {
    let p = p.replace('\\', "/");
    !p.is_empty() && !p.starts_with('/') && !p.contains('\0') && p.split('/').all(|s| s != ".." && !s.contains(':'))
}

/// A placement for an explicit target (FOMOD choices), or None for docs and images, and for any path that would
/// leave the game folder (an installer's XML is the mod author's to write).
pub fn placement(source: &str, target: &str) -> Option<Placement> {
    if !safe_target(source) || !safe_target(target) { return None; }
    let target = norm(target);
    let kind = kind_of_target(&target);
    (kind != FileKind::Doc && !target.is_empty()).then(|| Placement { source: norm(source), target, kind })
}

/// The mod type and frameworks for a finished list of placements.
pub fn summarize(files: Vec<Placement>) -> InstallPlan {
    let mut plan = InstallPlan { files, ..Default::default() };
    for p in &plan.files {
        match p.kind {
            FileKind::Redscript => { plan.uses.insert(Framework::Redscript); }
            FileKind::Tweak => { plan.uses.insert(Framework::TweakXl); }
            FileKind::CetMod => { plan.uses.insert(Framework::Cet); }
            FileKind::ArchiveXl => { plan.uses.insert(Framework::ArchiveXl); }
            FileKind::Red4extPlugin => { plan.uses.insert(Framework::Red4ext); }
            _ => {}
        }
        if p.kind == FileKind::Redmod {
            if let Some(name) = p.target.split('/').nth(1) { if !plan.redmods.iter().any(|r| r == name) { plan.redmods.push(name.to_string()); } }
        }
    }
    plan.needs_redmod_deploy = !plan.redmods.is_empty();
    plan.mod_type = Some(type_of(&plan.files));
    plan
}

fn push(plan: &mut InstallPlan, source: &str, target: String) {
    let kind = kind_of_target(&target);
    if kind == FileKind::Doc { plan.skipped.push(source.to_string()); return; }
    plan.files.push(Placement { source: source.to_string(), target, kind });
}

fn type_of(files: &[Placement]) -> ModType {
    let has = |k: FileKind| files.iter().any(|p| p.kind == k);
    if files.is_empty() { return ModType::Unknown; }
    if has(FileKind::FrameworkCore) { return ModType::Framework; }
    if has(FileKind::Redmod) { return ModType::Redmod; }
    let archive = has(FileKind::Archive) || has(FileKind::ArchiveXl);
    let script = has(FileKind::Redscript) || has(FileKind::CetMod) || has(FileKind::Tweak) || has(FileKind::Red4extPlugin);
    match (archive, script) {
        (true, true) => ModType::Mixed,
        (true, false) => ModType::Archive,
        (false, true) => ModType::Script,
        _ => ModType::Unknown,
    }
}

/// A folder name safe on Windows: no reserved characters, no trailing dots or spaces.
pub fn safe_folder(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() { "mod".into() } else { s.chars().take(80).collect() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn audioware_files_stay_in_their_folder() {
        let p = plan_install(&["MyRadio/r6/audioware/MyRadio/sounds.yml".to_string(), "MyRadio/r6/audioware/MyRadio/a.wav".to_string()], "MyRadio");
        let t: Vec<&str> = p.files.iter().map(|f| f.target.as_str()).collect();
        assert_eq!(t.len(), 2);
        assert!(t.iter().all(|x| x.starts_with("r6/audioware/MyRadio/")), "{t:?}");
    }

    use super::*;
    fn v(a: &[&str]) -> Vec<String> { a.iter().map(|s| s.to_string()).collect() }
    fn targets(p: &InstallPlan) -> Vec<&str> { p.files.iter().map(|f| f.target.as_str()).collect() }

    #[test]
    fn loose_archive_goes_to_archive_pc_mod() {
        let p = plan_install(&v(&["Cool Jacket.archive", "readme.txt"]), "Cool Jacket");
        assert_eq!(targets(&p), ["archive/pc/mod/Cool Jacket.archive"]);
        assert_eq!(p.skipped, ["readme.txt"]);
        assert_eq!(p.mod_type, Some(ModType::Archive));
    }

    #[test]
    fn wrapped_layout_is_unwrapped() {
        let p = plan_install(&v(&["MyMod-1.2/archive/pc/mod/a.archive", "MyMod-1.2/archive/pc/mod/a.archive.xl", "MyMod-1.2/r6/scripts/MyMod/x.reds"]), "My Mod");
        assert_eq!(targets(&p), ["archive/pc/mod/a.archive", "archive/pc/mod/a.archive.xl", "r6/scripts/MyMod/x.reds"]);
        assert_eq!(p.mod_type, Some(ModType::Mixed));
        assert!(p.uses.contains(&Framework::ArchiveXl) && p.uses.contains(&Framework::Redscript));
    }

    #[test]
    fn archivexl_layout_from_the_live_file_list() {
        // paths seen in ArchiveXL's indexed file list on Nexus
        let p = plan_install(&v(&["red4ext/plugins/ArchiveXL/ArchiveXL.dll", "r6/scripts/ArchiveXL/ArchiveXL.reds"]), "ArchiveXL");
        assert_eq!(targets(&p), ["red4ext/plugins/ArchiveXL/ArchiveXL.dll", "r6/scripts/ArchiveXL/ArchiveXL.reds"]);
        assert_eq!(p.mod_type, Some(ModType::Script));
    }

    #[test]
    fn red4ext_core_is_a_framework() {
        let p = plan_install(&v(&["bin/x64/winmm.dll", "red4ext/RED4ext.dll", "red4ext/LICENSE.txt"]), "RED4ext");
        assert_eq!(p.mod_type, Some(ModType::Framework));
        assert_eq!(p.skipped, ["red4ext/LICENSE.txt"]);
    }

    #[test]
    fn redmod_folder_with_and_without_mods_prefix() {
        let a = plan_install(&v(&["mods/BetterX/info.json", "mods/BetterX/archives/x.archive"]), "Better X");
        assert_eq!(targets(&a), ["mods/BetterX/info.json", "mods/BetterX/archives/x.archive"]);
        assert!(a.needs_redmod_deploy);
        let b = plan_install(&v(&["BetterX/info.json", "BetterX/scripts/core/a.script"]), "Better X");
        assert_eq!(targets(&b), ["mods/BetterX/info.json", "mods/BetterX/scripts/core/a.script"]);
        assert_eq!(b.redmods, ["BetterX"]);
        assert_eq!(b.mod_type, Some(ModType::Redmod));
    }

    #[test]
    fn loose_cet_mod_and_loose_scripts() {
        let p = plan_install(&v(&["SimpleMenu/init.lua", "SimpleMenu/modules/ui.lua", "fix.reds", "fix.yaml"]), "Simple: Menu?");
        assert_eq!(targets(&p), [
            "bin/x64/plugins/cyber_engine_tweaks/mods/SimpleMenu/init.lua",
            "bin/x64/plugins/cyber_engine_tweaks/mods/SimpleMenu/modules/ui.lua",
            "r6/scripts/Simple_ Menu_/fix.reds",
            "r6/tweaks/Simple_ Menu_/fix.yaml",
        ]);
        assert!(p.uses.contains(&Framework::Cet) && p.uses.contains(&Framework::TweakXl));
    }

    #[test]
    fn backslashes_unknowns_and_variant_clashes() {
        let p = plan_install(&v(&["Option A\\a.archive", "Option B\\a.archive", "weird.bin"]), "x");
        assert_eq!(p.unknown, ["weird.bin"]);
        assert_eq!(p.warnings.len(), 2);
    }
}
