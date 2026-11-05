//! The Cyberpunk framework mods, how to spot them on disk, and how they depend on each other.
//!
//! UNVERIFIED: the Nexus mod IDs and the marker file paths below come from the frameworks' install
//! instructions as commonly published, not from a live check. Each one is a plain table entry, so a wrong
//! value is a one-line fix. Detection only needs one marker file to exist.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Framework {
    Cet,
    Red4ext,
    Redscript,
    ArchiveXl,
    TweakXl,
    Codeware,
    Cybercmd,
}

pub struct FrameworkInfo {
    pub id: Framework,
    pub name: &'static str,
    /// UNVERIFIED Nexus mod ID on cyberpunk2077.
    pub nexus_mod_id: u64,
    /// Words that, found in a mod's name or summary, suggest it needs this framework. Matched as whole words.
    pub aliases: &'static [&'static str],
    /// Game-relative files; any one present means the framework is installed. UNVERIFIED paths.
    pub markers: &'static [&'static str],
    /// The file whose PE version resource gives the installed version, when it has one.
    pub version_file: Option<&'static str>,
    /// Frameworks this one needs to load.
    pub needs: &'static [Framework],
}

pub const FRAMEWORKS: &[FrameworkInfo] = &[
    FrameworkInfo {
        id: Framework::Cet,
        name: "Cyber Engine Tweaks",
        nexus_mod_id: 107,
        aliases: &["cyber engine tweaks", "cet"],
        markers: &["bin/x64/plugins/cyber_engine_tweaks.asi"],
        version_file: Some("bin/x64/plugins/cyber_engine_tweaks.asi"),
        needs: &[],
    },
    FrameworkInfo {
        id: Framework::Red4ext,
        name: "RED4ext",
        nexus_mod_id: 2380,
        aliases: &["red4ext"],
        markers: &["red4ext/RED4ext.dll"],
        version_file: Some("red4ext/RED4ext.dll"),
        needs: &[],
    },
    FrameworkInfo {
        id: Framework::Redscript,
        name: "redscript",
        nexus_mod_id: 1511,
        aliases: &["redscript"],
        markers: &["engine/tools/scc.exe", "engine/tools/scc_lib.dll", "red4ext/plugins/redscript/redscript.dll"],
        version_file: None,
        needs: &[],
    },
    FrameworkInfo {
        id: Framework::ArchiveXl,
        name: "ArchiveXL",
        nexus_mod_id: 4198,
        aliases: &["archivexl", "archive xl"],
        markers: &["red4ext/plugins/ArchiveXL/ArchiveXL.dll"],
        version_file: Some("red4ext/plugins/ArchiveXL/ArchiveXL.dll"),
        needs: &[Framework::Red4ext],
    },
    FrameworkInfo {
        id: Framework::TweakXl,
        name: "TweakXL",
        nexus_mod_id: 4197,
        aliases: &["tweakxl", "tweak xl"],
        markers: &["red4ext/plugins/TweakXL/TweakXL.dll"],
        version_file: Some("red4ext/plugins/TweakXL/TweakXL.dll"),
        needs: &[Framework::Red4ext],
    },
    FrameworkInfo {
        id: Framework::Codeware,
        name: "Codeware",
        nexus_mod_id: 7780,
        aliases: &["codeware"],
        markers: &["red4ext/plugins/Codeware/Codeware.dll"],
        version_file: Some("red4ext/plugins/Codeware/Codeware.dll"),
        needs: &[Framework::Red4ext, Framework::Redscript],
    },
    FrameworkInfo {
        id: Framework::Cybercmd,
        name: "cybercmd",
        nexus_mod_id: 5176,
        aliases: &["cybercmd"],
        markers: &["bin/x64/plugins/cybercmd.asi", "bin/x64/plugins/cybercmd.dll"],
        version_file: None,
        needs: &[],
    },
];

pub fn info(id: Framework) -> &'static FrameworkInfo {
    FRAMEWORKS.iter().find(|f| f.id == id).expect("every framework has a table entry")
}

pub fn by_mod_id(mod_id: u64) -> Option<&'static FrameworkInfo> {
    FRAMEWORKS.iter().find(|f| f.nexus_mod_id == mod_id)
}

/// Joins a game-relative path written with forward slashes onto the game root.
pub fn game_path(root: &Path, rel: &str) -> std::path::PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFramework {
    pub id: Framework,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    /// Modification time of the marker file: when this copy of the framework was installed or updated.
    pub file_time: Option<i64>,
}

/// Looks for every framework under the game root.
pub fn detect(root: &Path) -> Vec<InstalledFramework> {
    FRAMEWORKS
        .iter()
        .map(|f| {
            let found = f.markers.iter().map(|m| game_path(root, m)).find(|p| p.is_file());
            let version = f
                .version_file
                .map(|v| game_path(root, v))
                .filter(|p| p.is_file())
                .and_then(|p| crate::game::pe_version(&p).ok().flatten());
            let file_time = found.as_ref().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok()).map(
                |t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
            );
            InstalledFramework { id: f.id, name: f.name.into(), installed: found.is_some(), version, file_time }
        })
        .collect()
}

/// The frameworks `wanted` needs, including the ones those need, in load order (dependencies first).
pub fn closure(wanted: &[Framework]) -> Vec<Framework> {
    fn visit(f: Framework, out: &mut Vec<Framework>) {
        if out.contains(&f) {
            return;
        }
        for d in info(f).needs {
            visit(*d, out);
        }
        out.push(f);
    }
    let mut out = Vec::new();
    for f in wanted {
        visit(*f, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_puts_dependencies_first() {
        let c = closure(&[Framework::Codeware]);
        assert_eq!(c, vec![Framework::Red4ext, Framework::Redscript, Framework::Codeware]);
        let c = closure(&[Framework::ArchiveXl, Framework::TweakXl]);
        assert_eq!(c, vec![Framework::Red4ext, Framework::ArchiveXl, Framework::TweakXl]);
    }

    #[test]
    fn detect_finds_markers() {
        let dir = tempfile::tempdir().unwrap();
        let p = game_path(dir.path(), "red4ext/plugins/ArchiveXL/ArchiveXL.dll");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"not a real dll").unwrap();
        let found = detect(dir.path());
        let axl = found.iter().find(|f| f.id == Framework::ArchiveXl).unwrap();
        assert!(axl.installed);
        assert!(axl.version.is_none(), "a fake dll has no version resource");
        assert!(!found.iter().find(|f| f.id == Framework::Cet).unwrap().installed);
    }

    #[test]
    fn mod_ids_are_unique() {
        let mut ids: Vec<u64> = FRAMEWORKS.iter().map(|f| f.nexus_mod_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), FRAMEWORKS.len());
    }
}
