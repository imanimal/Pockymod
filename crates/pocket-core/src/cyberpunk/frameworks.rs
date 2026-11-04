//! The Cyberpunk 2077 framework mods. They hook the game executable, so they break on game patches.
//! Nexus mod IDs were checked against the live API on 2026-11-04.
//! `files` are the paths (relative to the game folder) that show a framework is installed. They follow each
//! framework's documented layout; UNVERIFIED against a real install in this build.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Framework { Cet, Red4ext, Redscript, ArchiveXl, TweakXl, Codeware, Cybercmd }

pub struct FrameworkInfo {
    pub id: Framework,
    pub key: &'static str,
    pub name: &'static str,
    pub nexus_mod_id: u32,
    /// A Nexus tag that marks mods using it, where one exists (verified live: only CET has one).
    pub tag: Option<&'static str>,
    /// Words that, in a mod's name, summary or description, suggest it needs this framework.
    pub words: &'static [&'static str],
    /// Files whose presence shows the framework is installed. The first is also the one whose version is read.
    pub files: &'static [&'static str],
    /// Frameworks this one needs.
    pub needs: &'static [Framework],
}

pub const FRAMEWORKS: &[FrameworkInfo] = &[
    FrameworkInfo { id: Framework::Cet, key: "cet", name: "Cyber Engine Tweaks", nexus_mod_id: 107, tag: Some("Cyber Engine Tweaks"),
        words: &["Cyber Engine Tweaks", "CET"], files: &["bin/x64/plugins/cyber_engine_tweaks.asi"], needs: &[] },
    FrameworkInfo { id: Framework::Red4ext, key: "red4ext", name: "RED4ext", nexus_mod_id: 2380, tag: None,
        words: &["RED4ext"], files: &["red4ext/RED4ext.dll", "bin/x64/winmm.dll"], needs: &[] },
    FrameworkInfo { id: Framework::Redscript, key: "redscript", name: "redscript", nexus_mod_id: 1511, tag: None,
        words: &["redscript"], files: &["engine/tools/scc.exe", "engine/config/base/scripts.ini"], needs: &[] },
    FrameworkInfo { id: Framework::ArchiveXl, key: "archivexl", name: "ArchiveXL", nexus_mod_id: 4198, tag: None,
        words: &["ArchiveXL", "Archive XL"], files: &["red4ext/plugins/ArchiveXL/ArchiveXL.dll"], needs: &[Framework::Red4ext] },
    FrameworkInfo { id: Framework::TweakXl, key: "tweakxl", name: "TweakXL", nexus_mod_id: 4197, tag: None,
        words: &["TweakXL", "Tweak XL"], files: &["red4ext/plugins/TweakXL/TweakXL.dll"], needs: &[Framework::Red4ext] },
    FrameworkInfo { id: Framework::Codeware, key: "codeware", name: "Codeware", nexus_mod_id: 7780, tag: None,
        words: &["Codeware"], files: &["red4ext/plugins/Codeware/Codeware.dll"], needs: &[Framework::Red4ext] },
    FrameworkInfo { id: Framework::Cybercmd, key: "cybercmd", name: "cybercmd", nexus_mod_id: 5176, tag: None,
        words: &["cybercmd"], files: &["bin/x64/plugins/cybercmd.asi"], needs: &[] },
];

pub fn info(f: Framework) -> &'static FrameworkInfo { FRAMEWORKS.iter().find(|i| i.id == f).expect("every framework has info") }
pub fn by_key(key: &str) -> Option<&'static FrameworkInfo> { FRAMEWORKS.iter().find(|i| i.key == key) }
pub fn by_mod_id(id: u32) -> Option<&'static FrameworkInfo> { FRAMEWORKS.iter().find(|i| i.nexus_mod_id == id) }
