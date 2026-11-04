//! Checks whether a newer Pocket Mods release is published on GitHub. Only tells; nothing is downloaded or run.
//! Installing updates automatically needs signed builds (Tauri's updater plugin) and is a release task.
//! The check answers 404 while the repository has no public release (seen on 2026-11-04).

use crate::error::{Error, Result};
use serde::Serialize;

pub const RELEASES: &str = "https://api.github.com/repos/imanimal/pocket-mods/releases/latest";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate { pub current: String, pub latest: Option<String>, pub newer: bool, pub notes: Option<String>, pub url: Option<String>, pub message: String }

pub fn is_newer(latest: &str, current: &str) -> bool {
    crate::health::version_key(latest) > crate::health::version_key(current)
}

pub async fn check(http: &reqwest::Client) -> Result<AppUpdate> {
    let current = crate::APP_VERSION.to_string();
    let resp = http.get(RELEASES).header("Accept", "application/vnd.github+json").send().await?;
    if resp.status().as_u16() == 404 {
        return Ok(AppUpdate { current, latest: None, newer: false, notes: None, url: None, message: "No published release found yet.".into() });
    }
    if !resp.status().is_success() { return Err(Error::Http { status: resp.status().as_u16(), message: "GitHub did not answer the update check.".into() }); }
    let v: serde_json::Value = resp.json().await?;
    let latest = v["tag_name"].as_str().map(|t| t.trim_start_matches('v').to_string());
    let newer = latest.as_deref().is_some_and(|l| is_newer(l, &current));
    Ok(AppUpdate {
        message: if newer { format!("Pocket Mods {} is out.", latest.as_deref().unwrap_or("?")) } else { "You have the newest version.".into() },
        current, newer, notes: v["body"].as_str().map(|s| s.chars().take(4000).collect()), url: v["html_url"].as_str().map(str::to_owned), latest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1", "0.1.0"));
    }
}
