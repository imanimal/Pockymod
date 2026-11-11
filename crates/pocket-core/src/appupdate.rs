//! New versions of Pockymod, from this project's GitHub releases. The check reads the latest release; `download`
//! fetches its Windows installer (the NSIS `*-setup.exe` asset) into the app's data folder and, when the release
//! also carries `<installer>.sha256`, checks it against that. The shell (src-tauri/src/background.rs) runs it when the
//! user is away. The check answers 404 while the repository has no public release (seen on 2026-11-04).
//! Code-signing the installer (a release task) is what lets Windows vouch for it; until then the download is trusted
//! as GitHub's HTTPS answer for this repository.

use crate::error::{Error, Result};
use serde::Serialize;

pub const RELEASES: &str = "https://api.github.com/repos/imanimal/Pockymod/releases/latest";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub current: String, pub latest: Option<String>, pub newer: bool, pub notes: Option<String>, pub url: Option<String>, pub message: String,
    /// The installer asset's address and its checksum file's, when the release has them.
    #[serde(skip)]
    pub installer: Option<(String, Option<String>)>,
}

/// Numbers first (trailing zeros don't count); with the same numbers a release beats a pre-release ("1.0.0" is newer
/// than "1.0.0-alpha"), and two pre-releases compare by their suffix.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let split = |v: &str| {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let (num, pre) = v.split_once('-').map_or((v, None), |(n, p)| (n, Some(p.trim().to_ascii_lowercase()).filter(|p| !p.is_empty())));
        (crate::health::version_key(num), pre)
    };
    let ((ln, lp), (cn, cp)) = (split(latest), split(current));
    if ln != cn { return ln > cn; }
    match (lp, cp) {
        (None, Some(_)) => true,
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

pub async fn check(http: &reqwest::Client) -> Result<AppUpdate> {
    let current = crate::APP_VERSION.to_string();
    let resp = http.get(RELEASES).header("Accept", "application/vnd.github+json").send().await?;
    if resp.status().as_u16() == 404 {
        return Ok(AppUpdate { current, latest: None, newer: false, notes: None, url: None, message: "No release published yet".into(), installer: None });
    }
    if !resp.status().is_success() { return Err(Error::Http { status: resp.status().as_u16(), message: "GitHub didn't answer the update check".into() }); }
    let v: serde_json::Value = resp.json().await?;
    let latest = v["tag_name"].as_str().map(|t| t.trim_start_matches('v').to_string());
    let newer = latest.as_deref().is_some_and(|l| is_newer(l, &current));
    let assets: Vec<(String, String)> = v["assets"].as_array().into_iter().flatten()
        .filter_map(|a| Some((a["name"].as_str()?.to_string(), a["browser_download_url"].as_str()?.to_string()))).collect();
    let installer = assets.iter().find(|(n, _)| n.to_ascii_lowercase().ends_with("-setup.exe")).map(|(n, u)| {
        (u.clone(), assets.iter().find(|(m, _)| m.eq_ignore_ascii_case(&format!("{n}.sha256"))).map(|(_, u)| u.clone()))
    });
    Ok(AppUpdate { installer,
        message: if newer { format!("Pockymod {} is out", latest.as_deref().unwrap_or("?")) } else { "You're on the newest version".into() },
        current, newer, notes: v["body"].as_str().map(|s| s.chars().take(4000).collect()), url: v["html_url"].as_str().map(str::to_owned), latest,
    })
}

/// What a queue item says when the "mod" is Pockymod's own installer, from its Nexus page.
pub const OWN_INSTALLER_NOTE: &str = "That's Pockymod itself, not a mod, so nothing was installed. It updates on its own, or press Check for updates in About";

/// Pockymod's own installer (`Pockymod_1.0.0_x64-setup.exe`, or Nexus's renamed copy of it), given file names or an
/// archive's entries. Its Nexus page offers Mod Manager Download like any mod's, and that is not a mod for the game.
pub fn is_own_installer<S: AsRef<str>>(names: &[S]) -> bool {
    names.iter().any(|n| {
        let n = n.as_ref();
        let base = n.rsplit(['/', '\\']).next().unwrap_or(n).to_ascii_lowercase();
        base.starts_with("pockymod") && base.contains("setup") && base.ends_with(".exe")
    })
}

/// Only GitHub's own download hosts, over HTTPS.
fn from_github(u: &str) -> bool {
    u.starts_with("https://github.com/imanimal/Pockymod/releases/download/") || u.starts_with("https://objects.githubusercontent.com/")
}

/// Download the release's installer into `dir` (once per version) and check its checksum when there is one.
pub async fn download(http: &reqwest::Client, u: &AppUpdate, dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let (url, sum) = u.installer.clone().ok_or_else(|| Error::Invalid("This release has no Windows installer".into()))?;
    if !from_github(&url) { return Err(Error::Invalid("The installer isn't on this project's GitHub releases".into())); }
    let version = u.latest.clone().unwrap_or_default();
    let safe: String = version.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect();
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("Pockymod-{safe}-setup.exe"));
    let ok_marker = dir.join(format!("Pockymod-{safe}-setup.exe.ok"));
    if path.exists() && ok_marker.exists() { return Ok(path); }
    let bytes = http.get(&url).send().await?.error_for_status().map_err(Error::from)?.bytes().await?;
    if let Some(s) = sum.filter(|s| from_github(s)) {
        let want = http.get(&s).send().await?.error_for_status().map_err(Error::from)?.text().await?;
        let want = want.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let got = sha256_hex(&bytes);
        if want != got { return Err(Error::Invalid("The installer failed its checksum and was deleted".into())); }
    }
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    std::fs::write(&ok_marker, version)?;
    // older installers are not needed
    for e in std::fs::read_dir(dir)?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with("Pockymod-") && !n.starts_with(&format!("Pockymod-{safe}-")) { let _ = std::fs::remove_file(e.path()); }
    }
    Ok(path)
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(b))
}

#[cfg(test)]
mod dl_tests {
    #[test]
    fn only_this_projects_downloads() {
        assert!(super::from_github("https://github.com/imanimal/Pockymod/releases/download/v0.2.0/Pockymod_0.2.0_x64-setup.exe"));
        assert!(!super::from_github("https://github.com/someone/else/releases/download/v1/x.exe"));
        assert!(!super::from_github("http://github.com/imanimal/Pockymod/releases/download/v1/x.exe"));
        assert_eq!(super::sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    use super::*;
    #[test]
    fn versions() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1", "0.1.0"));
    }
    #[test]
    fn pre_releases() {
        assert!(is_newer("1-alpha", "0.1.0"));
        assert!(!is_newer("1-alpha", "1.0.0-alpha"));
        assert!(!is_newer("1.0.0-alpha", "1-alpha"));
        assert!(is_newer("1.0.0", "1.0.0-alpha"));
        assert!(!is_newer("1.0.0-alpha", "1.0.0"));
        assert!(is_newer("1.0.0-beta", "1.0.0-alpha"));
        assert!(is_newer("1-alpha.2", "1.0.0-alpha"));
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("v0.1", "0.1.0"));
    }
    #[test]
    fn spots_its_own_installer() {
        assert!(is_own_installer(&["Pockymod_1.0.0-alpha.2_x64-setup.exe"]));
        assert!(is_own_installer(&["Pockymod_1.0.0_x64-setup-12345-1-0-0-1767225600.exe"]));
        assert!(is_own_installer(&["readme.txt", "Pockymod/Pockymod_1.0.0_x64-setup.exe"]));
        assert!(!is_own_installer(&["bin/x64/plugins/cyber_engine_tweaks.asi", "Pockymod theme.zip"]));
        assert!(!is_own_installer(&["archive/pc/mod/pockymod_setup.archive"]));
        assert!(!is_own_installer::<&str>(&[]));
    }
}
