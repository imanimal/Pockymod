//! Finding the game and reading versions.
//!
//! - Steam (app 1091500): the Steam path from the registry, every library in `steamapps/libraryfolders.vdf`, and the
//!   game's `appmanifest_1091500.acf`.
//! - GOG (product 1423049311): the GOG Galaxy registry key.
//! - Epic: the launcher's manifests in `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`. Best effort: Nexus's own
//!   app does not support Epic.
//! IDs and locations are as documented by the stores and used by other managers; UNVERIFIED on a real machine here.
//!
//! Versions come from the PE version resource of `bin/x64/Cyberpunk2077.exe` and the framework DLLs. How the
//! executable's version maps to a patch number (2.3, 2.31 …) is UNVERIFIED, so the patch is read from the product
//! version when it looks like one, and can always be set in Settings.

use super::frameworks::{Framework, FRAMEWORKS};
use crate::error::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const STEAM_APP_ID: &str = "1091500";
pub const GOG_PRODUCT_ID: &str = "1423049311";
pub const EXE: &str = "bin/x64/Cyberpunk2077.exe";
pub const REDMOD_EXE: &str = "tools/redmod/bin/redMod.exe";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall { pub store: &'static str, pub path: PathBuf, pub exists: bool }

/// Every install found on this machine, Steam first.
pub fn detect() -> Vec<GameInstall> {
    let mut out: Vec<GameInstall> = Vec::new();
    let mut push = |store: &'static str, p: PathBuf| {
        if !out.iter().any(|g| g.path == p) { let exists = p.join(EXE).exists(); out.push(GameInstall { store, path: p, exists }); }
    };
    for p in steam_candidates() { push("Steam", p); }
    for p in gog_candidates() { push("GOG", p); }
    for p in epic_candidates() { push("Epic", p); }
    out.sort_by_key(|g| !g.exists);
    out
}

/// Library folders listed in a `libraryfolders.vdf`.
pub fn vdf_library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines().filter_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("\"path\"")?.trim();
        let v = rest.trim_matches('"');
        Some(PathBuf::from(v.replace("\\\\", "\\")))
    }).collect()
}

/// `installdir` from an app manifest (`.acf`).
pub fn acf_install_dir(acf: &str) -> Option<String> {
    acf.lines().find_map(|l| l.trim().strip_prefix("\"installdir\"").map(|r| r.trim().trim_matches('"').to_string()))
}

fn steam_from_root(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(v) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) { libs.extend(vdf_library_paths(&v)); }
    libs.into_iter().filter_map(|lib| {
        let acf = std::fs::read_to_string(lib.join(format!("steamapps/appmanifest_{STEAM_APP_ID}.acf"))).ok()?;
        Some(lib.join("steamapps/common").join(acf_install_dir(&acf).unwrap_or_else(|| "Cyberpunk 2077".into())))
    }).collect()
}

#[cfg(windows)]
fn reg_str(root: winreg::HKEY, path: &str, name: &str) -> Option<String> {
    winreg::RegKey::predef(root).open_subkey(path).ok()?.get_value::<String, _>(name).ok()
}

fn steam_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let mut roots: Vec<PathBuf> = Vec::new();
        if let Some(p) = reg_str(HKEY_CURRENT_USER, "Software\\Valve\\Steam", "SteamPath") { roots.push(PathBuf::from(p)); }
        if let Some(p) = reg_str(HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath") { roots.push(PathBuf::from(p)); }
        roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
        roots.dedup();
        roots.iter().flat_map(|r| steam_from_root(r)).collect()
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        [home.join(".steam/steam"), home.join(".local/share/Steam")].iter().flat_map(|r| steam_from_root(r)).collect()
    }
}

fn gog_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        [format!("SOFTWARE\\WOW6432Node\\GOG.com\\Games\\{GOG_PRODUCT_ID}"), format!("SOFTWARE\\GOG.com\\Games\\{GOG_PRODUCT_ID}")]
            .iter().filter_map(|k| reg_str(HKEY_LOCAL_MACHINE, k, "path")).map(PathBuf::from).collect()
    }
    #[cfg(not(windows))]
    { Vec::new() }
}

/// An Epic manifest is JSON with `DisplayName` and `InstallLocation`.
pub fn epic_install_location(manifest: &str) -> Option<PathBuf> {
    let v: serde_json::Value = serde_json::from_str(manifest).ok()?;
    let name = v.get("DisplayName")?.as_str()?;
    if !name.to_lowercase().contains("cyberpunk 2077") { return None; }
    v.get("InstallLocation")?.as_str().map(PathBuf::from)
}

fn epic_candidates() -> Vec<PathBuf> {
    let base = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    let dir = base.join("Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok()).filter_map(|m| epic_install_location(&m)).collect()
}

/// File and product versions from a PE file's version resource, as "major.minor.patch.build".
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeVersion { pub file: String, pub product: String, pub product_string: Option<String> }

pub fn pe_version(path: &Path) -> Result<PeVersion> {
    use pelite::pe64::{Pe, PeFile};
    let map = pelite::FileMap::open(path).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    let file = PeFile::from_bytes(&map).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    let none = |e: String| Error::Invalid(format!("{} has no version information: {e}", path.display()));
    let res = file.resources().map_err(|e| none(e.to_string()))?;
    let vi = res.version_info().map_err(|e| none(e.to_string()))?;
    let fixed = vi.fixed().ok_or_else(|| Error::Invalid(format!("{} has no fixed version", path.display())))?;
    let v = |x: pelite::image::VS_VERSION| format!("{}.{}.{}.{}", x.Major, x.Minor, x.Patch, x.Build);
    let product_string = vi.file_info().strings.values().find_map(|m| m.get("ProductVersion").cloned());
    Ok(PeVersion { file: v(fixed.dwFileVersion), product: v(fixed.dwProductVersion), product_string })
}

/// A patch number like "2.31" from the executable's product version string, when it is written that way.
pub fn patch_from_product(s: &str) -> Option<String> {
    let mut parts = s.trim().split(['.', ' ']).filter(|p| !p.is_empty());
    let major: u32 = parts.next()?.parse().ok()?;
    let minor = parts.next()?;
    (major == 2 && minor.chars().all(|c| c.is_ascii_digit())).then(|| format!("2.{minor}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkState {
    pub framework: Framework,
    pub key: &'static str,
    pub name: &'static str,
    pub installed: bool,
    pub version: Option<String>,
    /// Unix seconds of the framework's main file.
    pub modified: Option<i64>,
    /// The file is older than the game's executable: it was there before the last game patch.
    pub older_than_game: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub path: PathBuf,
    pub exe_found: bool,
    pub version: Option<PeVersion>,
    /// From the product version when it reads like a patch number; else None (use the Settings choice).
    pub detected_patch: Option<String>,
    /// When the executable was last written, i.e. when the last patch was installed.
    pub exe_modified: Option<i64>,
    pub redmod_available: bool,
    pub frameworks: Vec<FrameworkState>,
    pub writable: bool,
    /// The Phantom Liberty expansion is installed (its archives are in archive/pc/ep1).
    #[serde(default)]
    pub phantom_liberty: bool,
}

fn mtime(p: &Path) -> Option<i64> {
    std::fs::metadata(p).ok()?.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

pub fn inspect(game: &Path) -> GameInfo {
    let exe = game.join(EXE);
    let version = pe_version(&exe).ok();
    let detected_patch = version.as_ref().and_then(|v| v.product_string.as_deref().and_then(patch_from_product).or_else(|| patch_from_product(&v.product)));
    let exe_modified = mtime(&exe);
    let frameworks = FRAMEWORKS.iter().map(|f| {
        let main = f.files.iter().map(|p| game.join(p)).find(|p| p.exists());
        let version = main.as_ref().filter(|p| p.extension().is_some_and(|e| e == "dll" || e == "asi" || e == "exe")).and_then(|p| pe_version(p).ok()).map(|v| v.product_string.unwrap_or(v.file));
        let modified = main.as_deref().and_then(mtime);
        // a patch rewrites the executable; a framework file older than that predates the patch
        let older = matches!((modified, exe_modified), (Some(m), Some(e)) if m + 60 < e);
        FrameworkState { framework: f.id, key: f.key, name: f.name, installed: main.is_some(), version, modified, older_than_game: older }
    }).collect();
    let writable = probe_writable(game);
    GameInfo { path: game.to_path_buf(), exe_found: exe.exists(), version, detected_patch, exe_modified, redmod_available: game.join(REDMOD_EXE).exists(), frameworks, writable,
        phantom_liberty: game.join("archive/pc/ep1").is_dir() }
}

/// Can this user create files in the game folder? (Program Files installs often say no.)
fn probe_writable(game: &Path) -> bool {
    let t = game.join(".pocket-mods-write-test");
    let ok = std::fs::write(&t, b"x").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_library_files() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\t\"apps\" { \"1091500\" \"70000000000\" }\n\t}\n}";
        assert_eq!(vdf_library_paths(vdf), [PathBuf::from("C:\\Program Files (x86)\\Steam"), PathBuf::from("D:\\SteamLibrary")]);
        assert_eq!(acf_install_dir("\"AppState\"\n{\n\t\"appid\"\t\t\"1091500\"\n\t\"installdir\"\t\t\"Cyberpunk 2077\"\n}").as_deref(), Some("Cyberpunk 2077"));
    }

    #[test]
    fn steam_layout_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::write(root.join(format!("steamapps/appmanifest_{STEAM_APP_ID}.acf")), "\"installdir\" \"Cyberpunk 2077\"").unwrap();
        assert_eq!(steam_from_root(root), [root.join("steamapps/common/Cyberpunk 2077")]);
    }

    #[test]
    fn epic_manifest() {
        let m = r#"{"DisplayName": "Cyberpunk 2077", "InstallLocation": "E:\\Epic\\Cyberpunk 2077", "AppName": "Ginger"}"#;
        assert_eq!(epic_install_location(m), Some(PathBuf::from("E:\\Epic\\Cyberpunk 2077")));
        assert!(epic_install_location(r#"{"DisplayName": "Other", "InstallLocation": "x"}"#).is_none());
    }

    #[test]
    fn reads_a_version_resource() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-fixture.dll");
        let v = pe_version(&p).unwrap();
        assert_eq!(v.file, "1.31.0.0");
        assert_eq!(v.product_string.as_deref(), Some("1.31.0"));
    }

    #[test]
    fn patch_numbers() {
        assert_eq!(patch_from_product("2.31").as_deref(), Some("2.31"));
        assert_eq!(patch_from_product("2.3.0.5").as_deref(), Some("2.3"));
        assert_eq!(patch_from_product("3.0.78.42"), None);
    }

    #[test]
    fn inspect_a_fake_game() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path();
        std::fs::create_dir_all(g.join("red4ext/plugins/ArchiveXL")).unwrap();
        std::fs::create_dir_all(g.join("bin/x64")).unwrap();
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-fixture.dll"), g.join("red4ext/RED4ext.dll")).unwrap();
        std::fs::write(g.join("red4ext/plugins/ArchiveXL/ArchiveXL.dll"), b"not a pe").unwrap();
        let info = inspect(g);
        assert!(!info.exe_found && info.writable);
        let r = info.frameworks.iter().find(|f| f.key == "red4ext").unwrap();
        assert!(r.installed);
        assert_eq!(r.version.as_deref(), Some("1.31.0"));
        let a = info.frameworks.iter().find(|f| f.key == "archivexl").unwrap();
        assert!(a.installed && a.version.is_none());
        assert!(!info.frameworks.iter().find(|f| f.key == "cet").unwrap().installed);
    }
}

/// redscript compiles every script into r6/cache/final.redscripts and keeps the game's own as .bk. Without
/// redscript (removed, or after a game patch) a stale compiled file breaks the game; this puts the game's own back.
/// A .bk older than the game's executable predates the last patch (the patch already wrote a fresh file), so it is
/// left alone. r6/cache/modded is REDmod's deploy output and is not touched. Returns how many files it changed.
pub fn reset_scripts(game: &Path) -> std::io::Result<usize> {
    let cache = game.join("r6/cache");
    let bk = cache.join("final.redscripts.bk");
    if !bk.is_file() { return Ok(0); }
    let (bk_time, exe_time) = (mtime(&bk), mtime(&game.join(EXE)));
    if matches!((bk_time, exe_time), (Some(b), Some(e)) if b + 60 < e) { return Ok(0); }
    let fin = cache.join("final.redscripts");
    let _ = std::fs::remove_file(&fin);
    std::fs::rename(&bk, &fin)?;
    Ok(1)
}

/// Compiled scripts are left over from a redscript that is gone.
pub fn stale_scripts(game: &Path) -> bool {
    let bk = game.join("r6/cache/final.redscripts.bk");
    let current = matches!((mtime(&bk), mtime(&game.join(EXE))), (Some(b), Some(e)) if b + 60 >= e) || mtime(&game.join(EXE)).is_none();
    bk.is_file() && current && !game.join("engine/tools/scc.exe").is_file()
}
