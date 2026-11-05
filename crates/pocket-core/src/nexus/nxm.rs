//! nxm:// links, which the Nexus site's "Mod Manager Download" button produces.
//! Shape: `nxm://cyberpunk2077/mods/107/files/12345?key=...&expires=1700000000&user_id=1`.
//! UNVERIFIED live in this build (no account to click with); the shape follows Vortex's handler.

use crate::error::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NxmLink {
    pub game: String,
    pub mod_id: u32,
    pub file_id: u32,
    /// Free-user download key; secret-ish, never logged.
    #[serde(skip_serializing)]
    pub key: Option<String>,
    pub expires: Option<i64>,
    pub user_id: Option<u64>,
}

impl NxmLink {
    pub fn parse(s: &str) -> Result<Self> {
        let u = url::Url::parse(s.trim()).map_err(|e| Error::Invalid(format!("Not an nxm link: {e}")))?;
        if u.scheme() != "nxm" { return Err(Error::Invalid("Not an nxm link".into())); }
        let game = u.host_str().unwrap_or("").to_lowercase();
        let parts: Vec<&str> = u.path_segments().map(|p| p.filter(|s| !s.is_empty()).collect()).unwrap_or_default();
        let (mod_id, file_id) = match parts.as_slice() {
            ["mods", m, "files", f] => (m.parse().ok(), f.parse().ok()),
            _ => (None, None),
        };
        let (Some(mod_id), Some(file_id)) = (mod_id, file_id) else {
            return Err(Error::Invalid("Not a mod file link. Open collections from Collections".into()));
        };
        let mut link = NxmLink { game, mod_id, file_id, key: None, expires: None, user_id: None };
        for (k, v) in u.query_pairs() {
            match k.as_ref() {
                "key" => link.key = Some(v.into_owned()),
                "expires" => link.expires = v.parse().ok(),
                "user_id" => link.user_id = v.parse().ok(),
                _ => {}
            }
        }
        Ok(link)
    }

    pub fn is_for_this_game(&self) -> bool { self.game == crate::GAME_DOMAIN }

    pub fn expired(&self, now_unix: i64) -> bool { self.expires.is_some_and(|e| e <= now_unix) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_free_user_link() {
        let l = NxmLink::parse("nxm://cyberpunk2077/mods/107/files/98765?key=abc_DEF-1&expires=1793800000&user_id=42").unwrap();
        assert_eq!((l.mod_id, l.file_id), (107, 98765));
        assert_eq!(l.key.as_deref(), Some("abc_DEF-1"));
        assert_eq!(l.expires, Some(1793800000));
        assert!(l.is_for_this_game());
        assert!(l.expired(1793800001));
        assert!(!format!("{}", serde_json::to_string(&l).unwrap()).contains("abc_DEF"));
    }
    #[test]
    fn premium_link_without_key_and_other_games() {
        let l = NxmLink::parse("nxm://SkyrimSpecialEdition/mods/1/files/2").unwrap();
        assert!(!l.is_for_this_game());
        assert!(l.key.is_none());
    }
    #[test]
    fn rejects_non_file_links() {
        assert!(NxmLink::parse("https://nexusmods.com").is_err());
        assert!(NxmLink::parse("nxm://cyberpunk2077/collections/abc/revisions/3").is_err());
    }
}
