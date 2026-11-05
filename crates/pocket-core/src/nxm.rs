//! nxm:// links, the "Mod Manager Download" handoff from the Nexus site.
//!
//! Shapes handled (the mod-file shape is the long-standing one Vortex and Mod Organizer parse):
//!   nxm://cyberpunk2077/mods/107/files/12345?key=abc&expires=1700000000&user_id=1
//!   nxm://cyberpunk2077/collections/abcdef/revisions/3
//! The `key` and `expires` pair is what lets a free account fetch a download link for that one file.

use crate::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NxmLink {
    #[serde(rename_all = "camelCase")]
    ModFile {
        game: String,
        mod_id: u64,
        file_id: u64,
        /// One-time download key. Never logged.
        #[serde(skip)]
        key: Option<String>,
        expires: Option<i64>,
        user_id: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Collection { game: String, slug: String, revision: Option<u64> },
}

impl NxmLink {
    pub fn parse(raw: &str) -> Result<NxmLink> {
        let raw = raw.trim().trim_matches('"');
        let url = url::Url::parse(raw).map_err(|e| Error::Invalid(format!("Not an nxm link: {e}")))?;
        if url.scheme() != "nxm" {
            return Err(Error::Invalid("Not an nxm link.".into()));
        }
        let game = url.host_str().unwrap_or_default().to_ascii_lowercase();
        if game.is_empty() {
            return Err(Error::Invalid("The nxm link names no game.".into()));
        }
        let parts: Vec<&str> = url.path().split('/').filter(|p| !p.is_empty()).collect();
        let q = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
        match parts.as_slice() {
            ["mods", mod_id, "files", file_id] => Ok(NxmLink::ModFile {
                game,
                mod_id: parse_id(mod_id)?,
                file_id: parse_id(file_id)?,
                key: q("key"),
                expires: q("expires").and_then(|v| v.parse().ok()),
                user_id: q("user_id").and_then(|v| v.parse().ok()),
            }),
            ["collections", slug, "revisions", rev] => Ok(NxmLink::Collection {
                game,
                slug: (*slug).to_string(),
                revision: rev.parse().ok(),
            }),
            ["collections", slug] => Ok(NxmLink::Collection { game, slug: (*slug).to_string(), revision: None }),
            _ => Err(Error::Invalid(format!("Unrecognised nxm link path: {}", url.path()))),
        }
    }

    pub fn game(&self) -> &str {
        match self {
            NxmLink::ModFile { game, .. } | NxmLink::Collection { game, .. } => game,
        }
    }

    /// True when the link's download key has passed its expiry time.
    pub fn expired(&self, now: i64) -> bool {
        matches!(self, NxmLink::ModFile { expires: Some(e), .. } if *e <= now)
    }
}

fn parse_id(s: &str) -> Result<u64> {
    s.parse().map_err(|_| Error::Invalid(format!("Bad id in nxm link: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mod_file_link() {
        let l = NxmLink::parse("nxm://cyberpunk2077/mods/107/files/12345?key=abc&expires=1700000000&user_id=9").unwrap();
        assert_eq!(
            l,
            NxmLink::ModFile {
                game: "cyberpunk2077".into(),
                mod_id: 107,
                file_id: 12345,
                key: Some("abc".into()),
                expires: Some(1700000000),
                user_id: Some(9)
            }
        );
        assert!(l.expired(1700000001));
        assert!(!l.expired(1600000000));
    }

    #[test]
    fn parses_quoted_and_cased() {
        let l = NxmLink::parse("\"nxm://Cyberpunk2077/mods/1/files/2\"").unwrap();
        assert_eq!(l.game(), "cyberpunk2077");
    }

    #[test]
    fn parses_collection_link() {
        let l = NxmLink::parse("nxm://cyberpunk2077/collections/qx7bdl/revisions/12").unwrap();
        assert_eq!(l, NxmLink::Collection { game: "cyberpunk2077".into(), slug: "qx7bdl".into(), revision: Some(12) });
    }

    #[test]
    fn rejects_junk() {
        assert!(NxmLink::parse("https://nexusmods.com").is_err());
        assert!(NxmLink::parse("nxm://cyberpunk2077/mods/x/files/2").is_err());
        assert!(NxmLink::parse("nxm://cyberpunk2077/whatever").is_err());
    }

    #[test]
    fn key_is_not_serialized() {
        let l = NxmLink::parse("nxm://cyberpunk2077/mods/1/files/2?key=secret&expires=5").unwrap();
        let json = serde_json::to_string(&l).unwrap();
        assert!(!json.contains("secret"));
    }
}
