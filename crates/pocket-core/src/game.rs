//! Finding Cyberpunk 2077 and reading which build is installed.
//!
//! Steam and GOG are detected; Epic is best effort (Nexus's own app doesn't support it either).
//! UNVERIFIED: the GOG product ID and the Epic manifest fields are from public launcher conventions, not checked
//! against a live install here. The Steam app ID 1091500 is the store page's.

use crate::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const STEAM_APP_ID: &str = "1091500";
/// UNVERIFIED GOG product ID for Cyberpunk 2077.
pub const GOG_GAME_IDS: &[&str] = &["1423049311"];
pub const EXE_REL: &str = "bin/x64/Cyberpunk2077.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Launcher {
    Steam,
    Gog,
    Epic,
    Manual,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall {
    pub root: PathBuf,
    pub launcher: Launcher,
    /// The exe's product version string, or its fixed file version.
    pub version: Option<String>,
    /// When the installed exe was built (PE header time stamp), used as "the patch came out" time.
    /// UNVERIFIED that CD Projekt's builds carry a real time stamp; falls back to the file's modified time.
    pub build_time: Option<i64>,
    pub under_program_files: bool,
    pub redmod_tool: bool,
}

pub fn is_game_root(root: &Path) -> bool {
    crate::frameworks::game_path(root, EXE_REL).is_file()
}

/// Reads one install's facts. Errors when the folder isn't a Cyberpunk 2077 folder.
pub fn inspect(root: &Path, launcher: Launcher) -> Result<GameInstall> {
    if !is_game_root(root) {
        return Err(Error::NotFound(format!("{} doesn't look like a Cyberpunk 2077 folder (no {EXE_REL}).", root.display())));
    }
    let exe = crate::frameworks::game_path(root, EXE_REL);
    let version = pe_version(&exe).ok().flatten();
    let build_time = pe_timestamp(&exe).ok().flatten().or_else(|| {
        std::fs::metadata(&exe).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
    });
    Ok(GameInstall {
        root: root.to_path_buf(),
        launcher,
        version,
        build_time,
        under_program_files: root.to_string_lossy().to_ascii_lowercase().contains("program files"),
        redmod_tool: crate::redmod::tool_path(root).is_file(),
    })
}

/// A PE file's version: the ProductVersion string when there is one, else the fixed file version.
pub fn pe_version(path: &Path) -> Result<Option<String>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(e, path))?;
    let Ok(pe) = pelite::PeFile::from_bytes(&bytes) else { return Ok(None) };
    let Ok(res) = pe.resources() else { return Ok(None) };
    let Ok(vi) = res.version_info() else { return Ok(None) };
    for lang in vi.translation() {
        if let Some(v) = vi.value(*lang, "ProductVersion") {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Ok(Some(v));
            }
        }
    }
    Ok(vi.fixed().map(|f| {
        let v = f.dwFileVersion;
        format!("{}.{}.{}.{}", v.Major, v.Minor, v.Patch, v.Build)
    }))
}

/// The PE header's link time stamp, when it looks like a real date (reproducible builds store a hash there).
pub fn pe_timestamp(path: &Path) -> Result<Option<i64>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(e, path))?;
    let Ok(pe) = pelite::PeFile::from_bytes(&bytes) else { return Ok(None) };
    let ts = pe.file_header().TimeDateStamp as i64;
    // 2020-01-01 .. now + 1 day: anything else is not a date
    Ok((ts > 1_577_836_800 && ts < crate::timefmt::now() + 86_400).then_some(ts))
}

/// Steam's libraryfolders.vdf: every library path that lists the given app.
pub fn steam_libraries_with_app(vdf: &str, app_id: &str) -> Vec<PathBuf> {
    // A tiny reader for the KeyValues text format: tokens are quoted strings and braces.
    let mut tokens = Vec::new();
    let mut chars = vdf.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some(n) = chars.next() {
                                s.push(n);
                            }
                        }
                        '"' => break,
                        _ => s.push(c),
                    }
                }
                tokens.push(Some(s));
            }
            '{' | '}' => {
                tokens.push(None);
                if c == '}' {
                    tokens.push(None);
                }
            }
            _ => {}
        }
    }
    // Walk: a library block has "path" and an "apps" block. Track the latest path and whether app_id appeared.
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        if let Some(t) = &tokens[i] {
            if t.eq_ignore_ascii_case("path") {
                if let Some(Some(v)) = tokens.get(i + 1) {
                    path = Some(v.clone());
                    i += 2;
                    continue;
                }
            }
            if t == app_id {
                if let Some(p) = &path {
                    let pb = PathBuf::from(p);
                    if !out.contains(&pb) {
                        out.push(pb);
                    }
                }
            }
        }
        i += 1;
    }
    out
}

/// The `installdir` from an appmanifest_<id>.acf.
pub fn acf_install_dir(acf: &str) -> Option<String> {
    let lower = acf.to_ascii_lowercase();
    let i = lower.find("\"installdir\"")?;
    let rest = &acf[i + "\"installdir\"".len()..];
    let start = rest.find('"')? + 1;
    let end = rest[start..].find('"')? + start;
    Some(rest[start..end].to_string())
}

fn steam_candidates(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_root.to_path_buf()];
    let vdf = steam_root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(&vdf) {
        for l in steam_libraries_with_app(&text, STEAM_APP_ID) {
            if !libs.contains(&l) {
                libs.push(l);
            }
        }
    }
    libs.into_iter()
        .map(|lib| {
            let apps = lib.join("steamapps");
            let dir = std::fs::read_to_string(apps.join(format!("appmanifest_{STEAM_APP_ID}.acf")))
                .ok()
                .and_then(|a| acf_install_dir(&a))
                .unwrap_or_else(|| "Cyberpunk 2077".into());
            apps.join("common").join(dir)
        })
        .collect()
}

/// Epic's launcher manifests (*.item JSON files): the InstallLocation of the one named Cyberpunk 2077.
pub fn epic_from_manifests(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("item"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(|v| v["DisplayName"].as_str().map(|n| n.to_ascii_lowercase().contains("cyberpunk 2077")).unwrap_or(false))
        .filter_map(|v| v["InstallLocation"].as_str().map(PathBuf::from))
        .collect()
}

/// Every Cyberpunk 2077 install we can find on this machine.
pub fn detect_all() -> Vec<GameInstall> {
    let mut found: Vec<GameInstall> = Vec::new();
    let mut push = |root: PathBuf, l: Launcher| {
        if found.iter().any(|g| g.root == root) {
            return;
        }
        if let Ok(g) = inspect(&root, l) {
            found.push(g);
        }
    };
    #[cfg(windows)]
    {
        if let Some(steam) = win::steam_root() {
            for c in steam_candidates(&steam) {
                push(c, Launcher::Steam);
            }
        }
        for p in win::gog_paths() {
            push(p, Launcher::Gog);
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = dirs::home_dir() {
            for steam in [home.join(".steam/steam"), home.join(".local/share/Steam")] {
                for c in steam_candidates(&steam) {
                    push(c, Launcher::Steam);
                }
            }
        }
    }
    let program_data = std::env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    for p in epic_from_manifests(&program_data.join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests")) {
        push(p, Launcher::Epic);
    }
    found
}

/// Where the mod store goes by default: on the game's drive, so hard links work.
/// Game on the same drive as the user profile → %LOCALAPPDATA%\PocketMods\store; otherwise <drive>:\PocketMods.
pub fn default_store(game_root: &Path) -> PathBuf {
    let local = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    if drive_of(&local) == drive_of(game_root) {
        local.join("PocketMods").join("store")
    } else {
        match drive_of(game_root) {
            Some(d) => PathBuf::from(format!("{d}\\PocketMods")),
            None => local.join("PocketMods").join("store"),
        }
    }
}

/// The drive letter part ("D:") of a Windows path, upper-cased.
pub fn drive_of(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let b = s.as_bytes();
    (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()).then(|| s[..2].to_ascii_uppercase())
}

#[cfg(windows)]
pub mod win {
    use std::path::PathBuf;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    pub fn steam_root() -> Option<PathBuf> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(k) = hkcu.open_subkey("Software\\Valve\\Steam") {
            if let Ok(p) = k.get_value::<String, _>("SteamPath") {
                return Some(PathBuf::from(p.replace('/', "\\")));
            }
        }
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for key in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
            if let Ok(k) = hklm.open_subkey(key) {
                if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                    return Some(PathBuf::from(p));
                }
            }
        }
        None
    }

    pub fn gog_paths() -> Vec<PathBuf> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let mut out = Vec::new();
        for id in super::GOG_GAME_IDS {
            for base in ["SOFTWARE\\WOW6432Node\\GOG.com\\Games", "SOFTWARE\\GOG.com\\Games"] {
                if let Ok(k) = hklm.open_subkey(format!("{base}\\{id}")) {
                    if let Ok(p) = k.get_value::<String, _>("path") {
                        out.push(PathBuf::from(p));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps"
		{
			"228980"		"0"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"label"		""
		"apps"
		{
			"1091500"		"71000000000"
			"1245620"		"1"
		}
	}
}"#;

    #[test]
    fn finds_the_library_with_cyberpunk() {
        assert_eq!(steam_libraries_with_app(VDF, STEAM_APP_ID), vec![PathBuf::from("D:\\SteamLibrary")]);
        assert!(steam_libraries_with_app(VDF, "999").is_empty());
    }

    #[test]
    fn reads_acf_installdir() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"1091500\"\n\t\"installdir\"\t\t\"Cyberpunk 2077\"\n}";
        assert_eq!(acf_install_dir(acf).as_deref(), Some("Cyberpunk 2077"));
    }

    #[test]
    fn steam_candidates_use_installdir() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("steamapps");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("appmanifest_1091500.acf"), "\"installdir\" \"CP77\"").unwrap();
        assert_eq!(steam_candidates(dir.path())[0], apps.join("common").join("CP77"));
    }

    #[test]
    fn epic_manifests() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.item"), r#"{"DisplayName":"Cyberpunk 2077","InstallLocation":"E:\\Epic\\Cyberpunk2077"}"#).unwrap();
        std::fs::write(dir.path().join("b.item"), r#"{"DisplayName":"Other","InstallLocation":"E:\\Other"}"#).unwrap();
        assert_eq!(epic_from_manifests(dir.path()), vec![PathBuf::from("E:\\Epic\\Cyberpunk2077")]);
    }

    #[test]
    fn inspect_rejects_non_game_folders_and_reads_fake_ones() {
        let dir = tempfile::tempdir().unwrap();
        assert!(inspect(dir.path(), Launcher::Manual).is_err());
        let exe = crate::frameworks::game_path(dir.path(), EXE_REL);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"MZ not really").unwrap();
        let g = inspect(dir.path(), Launcher::Manual).unwrap();
        assert_eq!(g.version, None);
        assert!(g.build_time.is_some(), "falls back to the file time");
        assert!(!g.redmod_tool);
    }

    #[test]
    fn drive_letters() {
        assert_eq!(drive_of(Path::new("d:\\Games\\Cyberpunk 2077")).as_deref(), Some("D:"));
        assert_eq!(drive_of(Path::new("/home/x")), None);
    }

    /// Reads a real PE file's version resource. Windows only: uses a system exe that is always there.
    #[cfg(windows)]
    #[test]
    fn reads_a_real_exe_version() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let v = pe_version(&Path::new(&windir).join("System32").join("notepad.exe")).unwrap();
        assert!(v.is_some(), "notepad.exe has a version resource");
    }
}
