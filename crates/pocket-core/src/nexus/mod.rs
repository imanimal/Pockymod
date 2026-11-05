//! The Nexus Mods API client. Every request carries the `apikey`, `Application-Name` and `Application-Version`
//! headers, every response updates the rate-limit state, and a request is refused locally while throttled.
//!
//! The key is the person's own personal API key (fine for personal use, spec section 4). It is held in memory
//! only for the client's lifetime, is never logged, and never appears in a URL.

pub mod queries;
pub mod ratelimit;
pub mod types;

use crate::{Error, Result, APP_NAME, APP_VERSION, GAME_DOMAIN};
use ratelimit::{Pacing, RateLimit};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
pub use types::*;

pub const V1_BASE: &str = "https://api.nexusmods.com/v1";
pub const V2_GRAPHQL: &str = "https://api.nexusmods.com/v2/graphql";

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    key: Arc<String>,
    rate: Arc<Mutex<RateLimit>>,
    v1: String,
    v2: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").field("v1", &self.v1).field("key", &"<hidden>").finish()
    }
}

pub fn user_agent() -> String {
    format!("{APP_NAME}/{APP_VERSION} ({})", std::env::consts::OS)
}

impl Client {
    pub fn new(key: &str) -> Result<Client> {
        Client::with_base(key, V1_BASE, V2_GRAPHQL)
    }

    /// For tests and the probe: point the client at another server.
    pub fn with_base(key: &str, v1: &str, v2: &str) -> Result<Client> {
        let key = key.trim();
        if key.is_empty() {
            return Err(Error::NoKey);
        }
        let http = reqwest::Client::builder()
            .user_agent(user_agent())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        Ok(Client { http, key: Arc::new(key.to_string()), rate: Arc::new(Mutex::new(RateLimit::default())), v1: v1.trim_end_matches('/').into(), v2: v2.into() })
    }

    /// Shares rate-limit state across clients (the app rebuilds the client when the key changes).
    pub fn with_rate(mut self, rate: Arc<Mutex<RateLimit>>) -> Client {
        self.rate = rate;
        self
    }

    pub fn rate(&self) -> RateLimit {
        self.rate.lock().map(|r| r.clone()).unwrap_or_default()
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    fn v1_url(&self, path: &str) -> String {
        format!("{}/{}", self.v1, path.trim_start_matches('/'))
    }

    async fn send(&self, req: RequestBuilder, what: &str) -> Result<Value> {
        let now = crate::timefmt::now();
        let pacing = self.rate.lock().map(|r| r.pacing(now)).unwrap_or(Pacing::Go);
        match pacing {
            Pacing::Go => {}
            Pacing::Wait(d) => tokio::time::sleep(d).await,
            Pacing::Stop => {
                let resume = self.rate.lock().map(|r| r.resume_text()).unwrap_or_default();
                return Err(Error::RateLimited { resume });
            }
        }
        let resp = req
            .header("apikey", self.key.as_str())
            .header("Application-Name", APP_NAME)
            .header("Application-Version", APP_VERSION)
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        if let Ok(mut r) = self.rate.lock() {
            r.update_from(|n| headers.get(n).and_then(|v| v.to_str().ok()));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry = headers.get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
            let resume = self
                .rate
                .lock()
                .map(|mut r| {
                    r.mark_throttled(crate::timefmt::now(), retry);
                    r.resume_text()
                })
                .unwrap_or_default();
            return Err(Error::RateLimited { resume });
        }
        let text = resp.text().await?;
        if !status.is_success() {
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["message"].as_str().or(v["error"].as_str()).map(String::from))
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(match status {
                StatusCode::UNAUTHORIZED => Error::Nexus { status: 401, message: "Your API key was refused. Check it in Settings, or make a new one on your Nexus account page.".into() },
                _ => Error::Nexus { status: status.as_u16(), message: if msg.is_empty() { what.to_string() } else { msg } },
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| Error::BadResponse { what: what.into(), detail: e.to_string() })
    }

    async fn v1_get(&self, path: &str) -> Result<Value> {
        self.send(self.http.get(self.v1_url(path)), path).await
    }

    async fn v1_form(&self, method: Method, path: &str, form: &[(&str, String)]) -> Result<Value> {
        self.send(self.http.request(method, self.v1_url(path)).form(form), path).await
    }

    pub async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        self.send(self.http.post(&self.v2).json(&json!({ "query": query, "variables": variables })), "graphql").await
    }

    /// GET users/validate.json: checks the key and says whether the account is Premium.
    pub async fn validate(&self) -> Result<Account> {
        queries::parse_account(&self.v1_get("users/validate.json").await?)
    }

    /// One page of Browse results: one mods() call, card fields only.
    pub async fn mods(&self, q: &BrowseQuery) -> Result<ModPage> {
        let body = self.graphql(queries::MODS_QUERY, queries::mods_variables(q)).await?;
        queries::parse_mods_page(&body, q.offset)
    }

    pub async fn mod_v1(&self, mod_id: u64) -> Result<Value> {
        self.v1_get(&format!("games/{GAME_DOMAIN}/mods/{mod_id}.json")).await
    }

    pub async fn files(&self, mod_id: u64) -> Result<Vec<ModFile>> {
        Ok(queries::parse_files(&self.v1_get(&format!("games/{GAME_DOMAIN}/mods/{mod_id}/files.json")).await?))
    }

    /// UNVERIFIED query; a failure here is not fatal for the detail view.
    pub async fn requirements(&self, mod_id: u64) -> Result<Vec<Requirement>> {
        let body = self.graphql(queries::REQUIREMENTS_QUERY, queries::requirements_variables(mod_id)).await?;
        queries::parse_requirements(&body)
    }

    /// The archive's file list from Nexus's content preview (a static JSON file; not an API call, so no key is sent).
    pub async fn content_preview(&self, url: &str) -> Result<Vec<String>> {
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        Ok(queries::parse_content_preview(&resp.json::<Value>().await?))
    }

    /// Download URLs for one file. Premium accounts call this without `nxm`; free accounts need the key and
    /// expiry from an nxm link (a click on "Mod Manager Download").
    pub async fn download_links(&self, mod_id: u64, file_id: u64, nxm: Option<(&str, i64)>) -> Result<Vec<String>> {
        let path = format!("games/{GAME_DOMAIN}/mods/{mod_id}/files/{file_id}/download_link.json");
        let mut req = self.http.get(self.v1_url(&path));
        if let Some((key, expires)) = nxm {
            req = req.query(&[("key", key.to_string()), ("expires", expires.to_string())]);
        }
        match self.send(req, "download_link").await {
            Ok(v) => Ok(queries::parse_download_links(&v)),
            Err(Error::Nexus { status: 403, .. }) if nxm.is_none() => Err(Error::NeedsPremium),
            Err(e) => Err(e),
        }
    }

    /// Mods updated in the last day ("1d"), week ("1w") or month ("1m"): IDs and times only.
    pub async fn updated(&self, period: &str) -> Result<Vec<UpdatedMod>> {
        Ok(queries::parse_updated(&self.v1_get(&format!("games/{GAME_DOMAIN}/mods/updated.json?period={period}")).await?))
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        Ok(queries::parse_categories(&self.v1_get(&format!("games/{GAME_DOMAIN}.json")).await?))
    }

    /// Endorse (true) or abstain (false).
    pub async fn endorse(&self, mod_id: u64, version: &str, endorse: bool) -> Result<()> {
        let verb = if endorse { "endorse" } else { "abstain" };
        self.v1_form(Method::POST, &format!("games/{GAME_DOMAIN}/mods/{mod_id}/{verb}.json"), &[("version", version.into())]).await?;
        Ok(())
    }

    /// The account's endorsements on this game: mod ID and status ("Endorsed", "Abstained", "Undecided").
    pub async fn endorsements(&self) -> Result<Vec<(u64, String)>> {
        let v = self.v1_get("user/endorsements.json").await?;
        Ok(v.as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| e["domain_name"].as_str() == Some(GAME_DOMAIN))
                    .filter_map(|e| Some((e["mod_id"].as_u64()?, e["status"].as_str().unwrap_or("").to_string())))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn tracked(&self) -> Result<Vec<u64>> {
        let v = self.v1_get("user/tracked_mods.json").await?;
        Ok(v.as_array()
            .map(|a| a.iter().filter(|t| t["domain_name"].as_str() == Some(GAME_DOMAIN)).filter_map(|t| t["mod_id"].as_u64()).collect())
            .unwrap_or_default())
    }

    pub async fn set_tracked(&self, mod_id: u64, track: bool) -> Result<()> {
        let method = if track { Method::POST } else { Method::DELETE };
        self.v1_form(method, &format!("user/tracked_mods.json?domain_name={GAME_DOMAIN}"), &[("mod_id", mod_id.to_string())]).await?;
        Ok(())
    }

    /// GET on a v1 path given by another response (collections hand back such paths).
    pub async fn v1_path(&self, path: &str) -> Result<Value> {
        let path = path.trim_start_matches("/v1/").trim_start_matches("v1/");
        self.v1_get(path).await
    }

    /// The live check for milestone 2: what mods() and the Mod type really look like right now.
    /// Logs and returns the shapes so they can be compared with section 4's unconfirmed list.
    pub async fn probe(&self) -> Result<Value> {
        let introspect = r#"query { mod: __type(name: "Mod") { fields { name } } filter: __type(name: "ModsFilter") { inputFields { name } } sort: __type(name: "ModsSort") { inputFields { name } } }"#;
        let types = self.graphql(introspect, json!({})).await.unwrap_or_else(|e| json!({ "error": e.to_string() }));
        let names = |v: &Value, k: &str| -> Vec<String> {
            v[k].as_array().map(|a| a.iter().filter_map(|f| f["name"].as_str().map(String::from)).collect()).unwrap_or_default()
        };
        let mod_fields = names(&types["data"]["mod"], "fields");
        let filter_fields = names(&types["data"]["filter"], "inputFields");
        let sort_fields = names(&types["data"]["sort"], "inputFields");
        let q = BrowseQuery { count: 2, ..Default::default() };
        let raw = self.graphql(queries::MODS_QUERY, queries::mods_variables(&q)).await;
        let (raw_value, parsed) = match raw {
            Ok(v) => {
                let parsed = queries::parse_mods_page(&v, 0).map(|p| json!({ "total": p.total, "cards": p.mods.len(), "categories": p.facets.categories.len() }));
                (v, parsed.unwrap_or_else(|e| json!({ "parseError": e.to_string() })))
            }
            Err(e) => (json!({ "error": e.to_string() }), Value::Null),
        };
        let requested = ["modId", "name", "summary", "author", "uploader", "thumbnailUrl", "pictureUrl", "downloads", "endorsements", "updatedAt", "createdAt", "fileSize", "category", "adultContent", "version"];
        let missing: Vec<&str> = if mod_fields.is_empty() { vec![] } else { requested.iter().copied().filter(|f| !mod_fields.iter().any(|m| m == f)).collect() };
        let report = json!({
            "modTypeFields": mod_fields,
            "modsFilterFields": filter_fields,
            "modsSortFields": sort_fields,
            "requestedButMissing": missing,
            "sampleResponse": raw_value,
            "parsed": parsed,
            "rateLimit": self.rate(),
        });
        log::info!("Nexus probe: {}", serde_json::to_string(&report).unwrap_or_default());
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A one-shot HTTP server: answers one request with `response`, hands back the request it got.
    async fn serve_once(response: String) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let h = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 16384];
            let n = s.read(&mut buf).await.unwrap();
            s.write_all(response.as_bytes()).await.unwrap();
            s.shutdown().await.ok();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (format!("http://{addr}"), h)
    }

    fn http_response(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut r = format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
        for (k, v) in headers {
            r.push_str(&format!("{k}: {v}\r\n"));
        }
        r.push_str("\r\n");
        r.push_str(body);
        r
    }

    #[tokio::test]
    async fn sends_required_headers_and_reads_rate_limits() {
        let body = r#"{"user_id": 1, "name": "V", "is_premium": false, "key": "x"}"#;
        let (base, h) = serve_once(http_response("200 OK", &[("x-rl-daily-remaining", "19999"), ("x-rl-hourly-remaining", "499"), ("x-rl-daily-limit", "20000")], body)).await;
        let c = Client::with_base("my-secret-key", &base, &base).unwrap();
        let acct = c.validate().await.unwrap();
        assert_eq!(acct.name, "V");
        let req = h.await.unwrap().to_ascii_lowercase();
        assert!(req.starts_with("get /users/validate.json"), "{req}");
        assert!(req.contains("apikey: my-secret-key"));
        assert!(req.contains("application-name: pocket mods"));
        assert!(req.contains(&format!("application-version: {APP_VERSION}")));
        assert_eq!(c.rate().daily_remaining, Some(19999));
        assert!(!format!("{c:?}").contains("my-secret-key"));
    }

    #[tokio::test]
    async fn throttles_on_429_and_stops_sending() {
        let (base, h) = serve_once(http_response("429 Too Many Requests", &[("retry-after", "120")], "{}")).await;
        let c = Client::with_base("k", &base, &base).unwrap();
        assert!(matches!(c.validate().await, Err(Error::RateLimited { .. })));
        h.await.unwrap();
        // the next call is refused locally, without a request (nothing is listening any more)
        assert!(matches!(c.validate().await, Err(Error::RateLimited { .. })));
        assert_eq!(c.rate().level(crate::timefmt::now()), ratelimit::Level::Throttled);
    }

    #[tokio::test]
    async fn unauthorized_is_explained() {
        let (base, _h) = serve_once(http_response("401 Unauthorized", &[], r#"{"message":"Please provide a valid API Key"}"#)).await;
        let c = Client::with_base("k", &base, &base).unwrap();
        let e = c.validate().await.unwrap_err();
        assert!(e.to_string().contains("API key was refused"));
    }

    #[tokio::test]
    async fn free_accounts_get_needs_premium() {
        let (base, _h) = serve_once(http_response("403 Forbidden", &[], r#"{"message":"You don't have permission"}"#)).await;
        let c = Client::with_base("k", &base, &base).unwrap();
        assert!(matches!(c.download_links(1, 2, None).await, Err(Error::NeedsPremium)));
    }

    #[tokio::test]
    async fn nxm_key_goes_in_the_query() {
        let (base, h) = serve_once(http_response("200 OK", &[], r#"[{"URI":"https://cdn/x.zip"}]"#)).await;
        let c = Client::with_base("k", &base, &base).unwrap();
        let links = c.download_links(107, 9, Some(("abc", 1700))).await.unwrap();
        assert_eq!(links, vec!["https://cdn/x.zip"]);
        let req = h.await.unwrap();
        assert!(req.contains("/games/cyberpunk2077/mods/107/files/9/download_link.json?key=abc&expires=1700"), "{req}");
    }

    #[test]
    fn empty_key_is_no_key() {
        assert!(matches!(Client::new("  "), Err(Error::NoKey)));
    }
}
