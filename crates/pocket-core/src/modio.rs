//! mod.io, read-only (spec section 6, a later-phase experiment behind a flag).
//! Checked here: the REST API at https://api.mod.io/v1 answers 401 without `api_key`, so browsing needs a key.
//! UNVERIFIED: whether Cyberpunk 2077's game ID and a key are open to third-party apps (the docs suggest keys are
//! tied to a game; ask mod.io's developer contact), and every field below, which follows mod.io's public v1 docs.
//! Installs are left to the game: mod.io mods are subscribed and installed from the game's own Mods menu.

use crate::credentials::{OsKeyring, Secret, SecretStore};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub const BASE: &str = "https://api.mod.io/v1";

pub fn key_store() -> OsKeyring { OsKeyring::named("mod.io-api-key") }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game { pub id: u64, pub name: String, #[serde(default)] pub name_id: String }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Logo { #[serde(default)] pub thumb_320x180: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User { #[serde(default)] pub username: String }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stats { #[serde(default)] pub downloads_total: u64, #[serde(default)] pub subscribers_total: u64 }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModFile { #[serde(default)] pub filesize: u64, #[serde(default)] pub version: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub profile_url: String,
    #[serde(default)]
    pub date_updated: i64,
    #[serde(default)]
    pub logo: Logo,
    #[serde(default)]
    pub submitted_by: User,
    #[serde(default)]
    pub stats: Stats,
    #[serde(default)]
    pub modfile: Option<ModFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> { pub data: Vec<T>, #[serde(default)] pub result_total: u64, #[serde(default)] pub result_offset: u64 }

pub struct Client { http: reqwest::Client, key: Secret }

impl Client {
    pub fn new(key: Secret) -> Result<Self> {
        let http = reqwest::Client::builder().user_agent(format!("PocketMods/{}", crate::APP_VERSION)).timeout(std::time::Duration::from_secs(30)).build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Client { http, key })
    }
    pub fn from_store() -> Result<Self> {
        let key = key_store().get()?.ok_or_else(|| Error::Invalid("Add a mod.io API key in Settings first.".into()))?;
        Self::new(key)
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let mut q: Vec<(&str, String)> = vec![("api_key", self.key.expose().to_string())];
        q.extend(query.iter().cloned());
        let resp = self.http.get(format!("{BASE}{path}")).query(&q).send().await?;
        let status = resp.status().as_u16();
        if status == 401 || status == 403 { return Err(Error::Invalid("mod.io did not accept the API key.".into())); }
        if !resp.status().is_success() { return Err(Error::Http { status, message: "mod.io request failed".into() }); }
        resp.json().await.map_err(Error::from)
    }

    /// Find Cyberpunk 2077 among mod.io's games.
    pub async fn find_game(&self) -> Result<Option<Game>> {
        let p: Page<Game> = self.get("/games", &[("_q", "Cyberpunk 2077".into())]).await?;
        Ok(p.data.into_iter().find(|g| g.name.to_lowercase().contains("cyberpunk")))
    }

    pub async fn mods(&self, game_id: u64, text: Option<&str>, offset: u32) -> Result<Page<Mod>> {
        let mut q = vec![("_limit", "30".to_string()), ("_offset", offset.to_string()), ("_sort", "-downloads_total".to_string())];
        if let Some(t) = text.filter(|t| !t.trim().is_empty()) { q.push(("_q", t.trim().to_string())); }
        self.get(&format!("/games/{game_id}/mods"), &q).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_documented_mod() {
        let m: Page<Mod> = serde_json::from_str(r#"{"data":[{"id":2,"name":"Rogue Knight","summary":"x","profile_url":"https://mod.io/g/x/m/y","date_updated":1499841487,
          "logo":{"thumb_320x180":"https://example/t.png"},"submitted_by":{"username":"XanT"},"stats":{"downloads_total":1230},"modfile":{"filesize":15181,"version":"1.3"}}],
          "result_count":1,"result_offset":0,"result_limit":100,"result_total":1}"#).unwrap();
        assert_eq!(m.data[0].stats.downloads_total, 1230);
        assert_eq!(m.result_total, 1);
    }
}
