//! v1 REST calls: account, download links, endorse, track, updates. All need the API key.
//! UNVERIFIED live: v1 answers 401 without a key, so none of these shapes could be checked from the build machine.
//! Paths and fields follow the public v1 docs and Vortex's client. Fields are optional where the docs are unclear.

use super::NexusClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `GET /v1/users/validate.json`. The response also carries the key and an email; neither is kept.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct NexusUser {
    pub user_id: u64,
    pub name: String,
    #[serde(default)]
    pub is_premium: bool,
    #[serde(default)]
    pub is_supporter: bool,
    #[serde(default)]
    pub profile_url: Option<String>,
}

/// One CDN mirror from `download_link.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadLink {
    #[serde(rename = "URI")]
    pub uri: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub short_name: Option<String>,
}

/// `mods/updated.json`: IDs and timestamps only, which suits update badges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatedMod {
    pub mod_id: u32,
    pub latest_file_update: i64,
    pub latest_mod_activity: i64,
}

impl NexusClient {
    pub async fn validate(&self) -> Result<NexusUser> { self.v1_get("/users/validate.json").await }

    /// Premium: a direct link. Free accounts must pass the `key` and `expires` from an nxm:// link.
    pub async fn download_links(&self, mod_id: u32, file_id: u32, nxm: Option<(&str, i64)>) -> Result<Vec<DownloadLink>> {
        let mut path = format!("/games/{}/mods/{mod_id}/files/{file_id}/download_link.json", crate::GAME_DOMAIN);
        if let Some((key, expires)) = nxm {
            path.push_str(&format!("?key={}&expires={expires}", url::form_urlencoded::byte_serialize(key.as_bytes()).collect::<String>()));
        }
        self.v1_get(&path).await
    }

    /// period: "1d", "1w" or "1m".
    pub async fn updated_mods(&self, period: &str) -> Result<Vec<UpdatedMod>> {
        self.v1_get(&format!("/games/{}/mods/updated.json?period={period}", crate::GAME_DOMAIN)).await
    }

    pub async fn endorse(&self, mod_id: u32, version: &str) -> Result<serde_json::Value> {
        self.v1_post(&format!("/games/{}/mods/{mod_id}/endorse.json", crate::GAME_DOMAIN), &[("version", version)]).await
    }

    pub async fn abstain(&self, mod_id: u32, version: &str) -> Result<serde_json::Value> {
        self.v1_post(&format!("/games/{}/mods/{mod_id}/abstain.json", crate::GAME_DOMAIN), &[("version", version)]).await
    }

    pub async fn track(&self, mod_id: u32) -> Result<serde_json::Value> {
        let id = mod_id.to_string();
        self.v1_post(&format!("/user/tracked_mods.json?domain_name={}", crate::GAME_DOMAIN), &[("mod_id", id.as_str())]).await
    }

    pub async fn untrack(&self, mod_id: u32) -> Result<serde_json::Value> {
        let id = mod_id.to_string();
        self.v1_delete(&format!("/user/tracked_mods.json?domain_name={}", crate::GAME_DOMAIN), &[("mod_id", id.as_str())]).await
    }
}
