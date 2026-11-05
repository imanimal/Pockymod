//! Cyberpunk-aware facts the Nexus API doesn't give: which frameworks a mod needs, what kind of mod it is,
//! and whether it has been updated since the installed game patch.
//!
//! Sources, from weakest to strongest: words in the name and summary, the mod's requirement links, and the
//! file list of one of its archives (Nexus's content preview, or the archive itself once downloaded).

use crate::frameworks::{self, Framework};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModKind {
    /// Under `mods/<name>/` with an info.json: deployed by the game's REDmod tool.
    Redmod,
    /// `.archive` files only (plus ArchiveXL `.xl`).
    Archive,
    /// redscript, CET Lua, TweakXL tweaks or RED4ext plugins: code that can break on game patches.
    Script,
    /// One of the framework mods themselves.
    Framework,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compat {
    /// Updated after the installed game patch came out.
    Updated,
    /// Not updated since the patch, but archive-only mods rarely break.
    Likely,
    /// Not updated since the patch and it runs code: may be broken.
    Risky,
    /// No installed game version to compare with.
    Unknown,
}

/// What we could work out, and from what.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Derived {
    pub frameworks: BTreeSet<Framework>,
    pub kind: Option<ModKind>,
    /// "text", "requirements", "files": which inputs contributed.
    pub sources: BTreeSet<String>,
}

fn words(text: &str) -> String {
    // lower case, every non-alphanumeric run becomes one space, padded so " cet " matches at the ends
    let mut out = String::with_capacity(text.len() + 2);
    out.push(' ');
    let mut last_space = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    if !last_space {
        out.push(' ');
    }
    out
}

/// Frameworks named in free text. `self_id` keeps a framework's own page from "requiring" itself.
pub fn from_text(text: &str, self_id: Option<u64>) -> BTreeSet<Framework> {
    let w = words(text);
    frameworks::FRAMEWORKS
        .iter()
        .filter(|f| Some(f.nexus_mod_id) != self_id)
        .filter(|f| f.aliases.iter().any(|a| w.contains(&format!(" {a} "))))
        .map(|f| f.id)
        .collect()
}

/// Frameworks among a mod's required mod IDs.
pub fn from_requirements(required_mod_ids: &[u64]) -> BTreeSet<Framework> {
    required_mod_ids.iter().filter_map(|id| frameworks::by_mod_id(*id)).map(|f| f.id).collect()
}

/// Kind and frameworks from an archive's file list (paths inside the archive, any separator).
pub fn from_files(paths: &[String]) -> (Option<ModKind>, BTreeSet<Framework>) {
    let mut fw = BTreeSet::new();
    let (mut redmod, mut archive, mut script) = (false, false, false);
    for raw in paths {
        let p = raw.replace('\\', "/").to_ascii_lowercase();
        let name = p.rsplit('/').next().unwrap_or(&p);
        let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        if p.contains("mods/") && name == "info.json" {
            redmod = true;
        }
        match ext {
            "archive" => archive = true,
            "xl" => {
                archive = true;
                fw.insert(Framework::ArchiveXl);
            }
            "reds" => {
                script = true;
                fw.insert(Framework::Redscript);
            }
            "lua" => {
                script = true;
                fw.insert(Framework::Cet);
            }
            "yaml" | "yml" | "tweak" if p.contains("tweaks/") || !p.contains('/') => {
                script = true;
                fw.insert(Framework::TweakXl);
            }
            "dll" if p.contains("red4ext/plugins/") => {
                script = true;
                fw.insert(Framework::Red4ext);
            }
            _ => {}
        }
    }
    let kind = if redmod {
        Some(ModKind::Redmod)
    } else if script {
        Some(ModKind::Script)
    } else if archive {
        Some(ModKind::Archive)
    } else if paths.is_empty() {
        None
    } else {
        Some(ModKind::Other)
    };
    (kind, fw)
}

/// Combines every source we have for one mod.
pub fn derive(
    mod_id: u64,
    name: &str,
    summary: &str,
    required_mod_ids: Option<&[u64]>,
    files: Option<&[String]>,
) -> Derived {
    let mut d = Derived::default();
    if frameworks::by_mod_id(mod_id).is_some() {
        d.kind = Some(ModKind::Framework);
        d.sources.insert("id".into());
    }
    let t = from_text(&format!("{name} {summary}"), Some(mod_id));
    if !t.is_empty() {
        d.sources.insert("text".into());
        d.frameworks.extend(t);
    }
    if let Some(req) = required_mod_ids {
        d.sources.insert("requirements".into());
        d.frameworks.extend(from_requirements(req));
    }
    if let Some(files) = files {
        let (kind, fw) = from_files(files);
        d.sources.insert("files".into());
        // a framework's own files say "script", but it is a framework
        if d.kind.is_none() {
            d.kind = kind;
        }
        if d.kind != Some(ModKind::Framework) {
            d.frameworks.extend(fw);
        }
    }
    if let Some(own) = frameworks::by_mod_id(mod_id) {
        d.frameworks.remove(&own.id);
        d.frameworks.extend(own.needs.iter().copied());
    }
    d
}

/// Is this mod likely to work with the installed patch? `patch_time` is when the installed game build came out.
pub fn compat(updated_at: i64, patch_time: Option<i64>, d: &Derived) -> Compat {
    let Some(patch) = patch_time else { return Compat::Unknown };
    if updated_at >= patch {
        return Compat::Updated;
    }
    let runs_code = matches!(d.kind, Some(ModKind::Script | ModKind::Framework | ModKind::Redmod)) || !d.frameworks.is_empty();
    if runs_code {
        Compat::Risky
    } else {
        Compat::Likely
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_matches_whole_words_only() {
        let f = from_text("Requires Cyber Engine Tweaks and ArchiveXL.", None);
        assert!(f.contains(&Framework::Cet) && f.contains(&Framework::ArchiveXl));
        // "cet" inside another word is not CET
        assert!(from_text("Lancet facets", None).is_empty());
        assert!(from_text("needs CET 1.32", None).contains(&Framework::Cet));
        // a framework's own page doesn't require itself
        assert!(from_text("RED4ext is a script extender", Some(2380)).is_empty());
    }

    #[test]
    fn file_lists_give_kind() {
        let files = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_files(&files(&["archive/pc/mod/a.archive"])).0, Some(ModKind::Archive));
        let (k, f) = from_files(&files(&["r6\\scripts\\x\\x.reds", "archive/pc/mod/a.archive"]));
        assert_eq!(k, Some(ModKind::Script));
        assert!(f.contains(&Framework::Redscript));
        assert_eq!(from_files(&files(&["mods/Cool/info.json", "mods/Cool/archives/x.archive"])).0, Some(ModKind::Redmod));
        let (_, f) = from_files(&files(&["bin/x64/plugins/cyber_engine_tweaks/mods/m/init.lua"]));
        assert!(f.contains(&Framework::Cet));
        let (_, f) = from_files(&files(&["r6/tweaks/m/a.yaml", "archive/pc/mod/a.xl"]));
        assert!(f.contains(&Framework::TweakXl) && f.contains(&Framework::ArchiveXl));
        assert_eq!(from_files(&files(&["readme.txt"])).0, Some(ModKind::Other));
    }

    #[test]
    fn derive_merges_sources() {
        let d = derive(9999, "Cool Jacket", "An ArchiveXL outfit", Some(&[2380]), Some(&["archive/pc/mod/j.archive".into()]));
        assert_eq!(d.kind, Some(ModKind::Archive));
        assert!(d.frameworks.contains(&Framework::ArchiveXl) && d.frameworks.contains(&Framework::Red4ext));
        assert_eq!(d.sources.len(), 3);
        // a framework mod is a framework, and lists what it needs, not itself
        let d = derive(7780, "Codeware", "Codeware library", None, Some(&["red4ext/plugins/Codeware/Codeware.dll".into()]));
        assert_eq!(d.kind, Some(ModKind::Framework));
        assert!(!d.frameworks.contains(&Framework::Codeware));
        assert!(d.frameworks.contains(&Framework::Red4ext));
    }

    #[test]
    fn compat_rules() {
        let archive = Derived { kind: Some(ModKind::Archive), ..Default::default() };
        let script = Derived { kind: Some(ModKind::Script), ..Default::default() };
        assert_eq!(compat(10, None, &archive), Compat::Unknown);
        assert_eq!(compat(10, Some(5), &script), Compat::Updated);
        assert_eq!(compat(1, Some(5), &script), Compat::Risky);
        assert_eq!(compat(1, Some(5), &archive), Compat::Likely);
    }
}
