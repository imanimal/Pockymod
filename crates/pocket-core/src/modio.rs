//! mod.io, read-only (spec section 6, a later-phase experiment behind a flag).
//!
//! Browsing and downloading need an API key and the game's mod.io ID. Whether Cyberpunk's key and game ID are
//! open to third parties is unconfirmed; the person enters both in Settings. Installs are left to the game.
//! UNVERIFIED for Cyberpunk: the response fields below follow mod.io's public REST docs (v1).

use crate::{Error, Result};
use serde::Serialize;
use serde_json::Value;

pub const API_BASE: &str = "https://api.mod.io/v1";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModioMod {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub logo_url: Option<String>,
    pub downloads: u64,
    pub updated_at: i64,
    pub size_bytes: u64,
    pub profile_url: Option<String>,
}

pub fn parse_mods(v: &Value) -> Result<(Vec<ModioMod>, u64)> {
    if let Some(msg) = v["error"]["message"].as_str() {
        return Err(Error::BadResponse { what: "mod.io".into(), detail: msg.into() });
    }
    let mods = v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    Some(ModioMod {
                        id: m["id"].as_u64()?,
                        name: m["name"].as_str().unwrap_or_default().into(),
                        summary: m["summary"].as_str().unwrap_or_default().into(),
                        author: m["submitted_by"]["username"].as_str().unwrap_or_default().into(),
                        logo_url: m["logo"]["thumb_320x180"].as_str().map(String::from),
                        downloads: m["stats"]["downloads_total"].as_u64().unwrap_or(0),
                        updated_at: m["date_updated"].as_i64().unwrap_or(0),
                        size_bytes: m["modfile"]["filesize"].as_u64().unwrap_or(0),
                        profile_url: m["profile_url"].as_str().map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((mods, v["result_total"].as_u64().unwrap_or(0)))
}

/// Searches a game's mods. The key goes in the query string (mod.io's read-only auth); it is never logged.
pub async fn search(http: &reqwest::Client, api_key: &str, game_id: u64, text: &str, offset: u32, limit: u32) -> Result<(Vec<ModioMod>, u64)> {
    if api_key.trim().is_empty() {
        return Err(Error::Invalid("Add a mod.io API key in Settings to browse mod.io.".into()));
    }
    let mut q: Vec<(&str, String)> = vec![("api_key", api_key.trim().into()), ("_offset", offset.to_string()), ("_limit", limit.clamp(1, 100).to_string()), ("_sort", "-popular".into())];
    if !text.trim().is_empty() {
        q.push(("_q", text.trim().into()));
    }
    let resp = http.get(format!("{API_BASE}/games/{game_id}/mods")).query(&q).header("Accept", "application/json").send().await?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = v["error"]["message"].as_str().unwrap_or("request failed").to_string();
        return Err(Error::BadResponse { what: format!("mod.io (HTTP {status})"), detail: msg });
    }
    parse_mods(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_mod_list() {
        let v = json!({"data": [{"id": 1, "name": "M", "summary": "s", "submitted_by": {"username": "u"}, "logo": {"thumb_320x180": "https://i"},
            "stats": {"downloads_total": 5}, "date_updated": 10, "modfile": {"filesize": 99}, "profile_url": "https://mod.io/m"}], "result_total": 1});
        let (m, t) = parse_mods(&v).unwrap();
        assert_eq!(t, 1);
        assert_eq!(m[0].author, "u");
        assert_eq!(m[0].size_bytes, 99);
        assert!(parse_mods(&json!({"error": {"message": "bad key"}})).is_err());
    }
}
