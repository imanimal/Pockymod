//! Checking for a newer Pocket Mods release (GitHub's "latest release" endpoint for this repository).
//! The check only reports; downloading the installer is the person's choice.

use crate::{Result, APP_VERSION};
use serde::Serialize;
use serde_json::Value;

pub const RELEASES_URL: &str = "https://api.github.com/repos/imanimal/pocket-mods/releases/latest";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub url: String,
    pub notes: String,
    pub newer: bool,
}

/// "v1.2.3", "1.2", "1.2.3-beta.1" → (1, 2, 3). Pre-release tags sort below the release.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64, bool)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match s.split_once('-') {
        Some((c, _)) => (c, true),
        None => (s, false),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch, !pre))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

pub fn parse_release(v: &Value) -> Option<Release> {
    let tag = v["tag_name"].as_str()?;
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).into(),
        url: v["html_url"].as_str().unwrap_or_default().into(),
        notes: v["body"].as_str().unwrap_or_default().chars().take(4000).collect(),
        newer: is_newer(tag, APP_VERSION),
    })
}

pub async fn check(http: &reqwest::Client, url: &str) -> Result<Option<Release>> {
    let resp = http.get(url).header("Accept", "application/vnd.github+json").send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None); // no releases yet
    }
    if !resp.status().is_success() {
        return Err(crate::Error::Network(format!("The update check answered {}.", resp.status())));
    }
    Ok(parse_release(&resp.json().await?))
}

/// Checks this repository's latest release.
pub async fn check_latest() -> Result<Option<Release>> {
    let http = reqwest::Client::builder().user_agent(crate::nexus::user_agent()).build()?;
    check(&http, RELEASES_URL).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "1.0.0-beta.2"));
        assert!(!is_newer("1.0.0-beta.2", "1.0.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("junk", "0.1.0"));
        assert_eq!(parse_version("2.1"), Some((2, 1, 0, true)));
    }

    #[test]
    fn release_json() {
        let r = parse_release(&serde_json::json!({"tag_name": "v99.0.0", "html_url": "https://github.com/x", "body": "notes"})).unwrap();
        assert!(r.newer);
        assert_eq!(r.version, "99.0.0");
    }
}
