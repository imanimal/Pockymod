//! HTTP client for the Nexus API: v2 GraphQL for browsing, v1 REST for account, files and download links.
//! Every request carries `Application-Name` and `Application-Version`; keyed requests carry `APIKEY`.
//! A request is never retried automatically: a 429 stops all requests until the reset.

use super::ratelimit::{Gate, RateDisplay, RateLimit};
use crate::credentials::Secret;
use crate::error::{Error, Result};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};

pub const V1_BASE: &str = "https://api.nexusmods.com/v1";
pub const V2_GRAPHQL: &str = "https://api.nexusmods.com/v2/graphql";

pub type RateObserver = Arc<dyn Fn(RateDisplay) + Send + Sync>;

pub struct NexusClient {
    http: reqwest::Client,
    /// For file downloads: no limit on the whole transfer (a big file on a free account takes many minutes),
    /// only on a stalled connection.
    dl: reqwest::Client,
    key: RwLock<Option<Secret>>,
    rate: Mutex<RateLimit>,
    observer: RwLock<Option<RateObserver>>,
    v1_base: String,
    v2_url: String,
}

/// Response headers kept for the live probe (names and the rate-limit values; nothing identifying).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HeaderSnapshot {
    pub status: u16,
    pub names: Vec<String>,
    pub rate_limit: Vec<(String, String)>,
}

impl NexusClient {
    pub fn new(key: Option<Secret>) -> Result<Self> {
        Self::with_endpoints(key, V1_BASE, V2_GRAPHQL)
    }

    pub fn with_endpoints(key: Option<Secret>, v1_base: &str, v2_url: &str) -> Result<Self> {
        let mut h = HeaderMap::new();
        h.insert("Application-Name", HeaderValue::from_static(crate::APP_NAME));
        h.insert("Application-Version", HeaderValue::from_static(crate::APP_VERSION));
        // the same name as Application-Name, so Nexus sees one app
        let ua = format!("{}/{} ({})", crate::APP_NAME, crate::APP_VERSION, std::env::consts::OS);
        let http = reqwest::Client::builder()
            .user_agent(ua.clone())
            .default_headers(h.clone())
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        let dl = reqwest::Client::builder()
            .user_agent(ua)
            .default_headers(h)
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(NexusClient { http, dl, key: RwLock::new(key), rate: Mutex::new(RateLimit::default()), observer: RwLock::new(None), v1_base: v1_base.into(), v2_url: v2_url.into() })
    }

    pub fn set_key(&self, key: Option<Secret>) { *self.key.write().unwrap() = key; }
    pub fn has_key(&self) -> bool { self.key.read().unwrap().is_some() }
    pub fn http(&self) -> &reqwest::Client { &self.http }
    /// The client for downloading files (no overall time limit).
    pub fn download_http(&self) -> &reqwest::Client { &self.dl }

    /// Called after every response with the new rate-limit state (the UI's LCD listens to this).
    pub fn set_observer(&self, f: RateObserver) { *self.observer.write().unwrap() = Some(f); }

    pub fn rate_limit(&self) -> RateDisplay { self.rate.lock().unwrap().display(Utc::now()) }

    async fn send(&self, req: reqwest::RequestBuilder, needs_key: bool) -> Result<(reqwest::Response, HeaderSnapshot)> {
        // slow down early, stop when out of budget; never queue a blind retry
        let gate = self.rate.lock().unwrap().gate(Utc::now());
        match gate {
            Gate::Go => {}
            Gate::Wait(d) => tokio::time::sleep(d).await,
            Gate::Stop(until) => return Err(Error::Throttled { until: until.format("%H:%M UTC").to_string() }),
        }
        let key = self.key.read().unwrap().clone();
        let req = match (&key, needs_key) {
            (Some(k), _) => req.header("APIKEY", k.expose()),
            (None, true) => return Err(Error::NoKey),
            (None, false) => req,
        };
        let resp = req.send().await?;
        let status = resp.status().as_u16();
        let snap = snapshot(status, resp.headers());
        let display = {
            let mut r = self.rate.lock().unwrap();
            r.observe(status, |n| resp.headers().get(n).and_then(|v| v.to_str().ok()).map(str::to_owned), Utc::now());
            r.display(Utc::now())
        };
        if let Some(obs) = self.observer.read().unwrap().clone() { obs(display.clone()); }
        match status {
            200..=299 => Ok((resp, snap)),
            429 => Err(Error::Throttled { until: display.throttled_until.map(|t| t.format("%H:%M UTC").to_string()).unwrap_or_else(|| "the next hour".into()) }),
            401 => Err(Error::BadKey),
            403 => {
                let msg = message_of(resp).await;
                // UNVERIFIED: v1 answers 403 with a Premium message when a free account asks for a download link
                if msg.to_lowercase().contains("premium") { Err(Error::NeedsPremium) } else { Err(Error::Http { status, message: msg }) }
            }
            _ => Err(Error::Http { status, message: message_of(resp).await }),
        }
    }

    /// A GraphQL v2 call. The key is sent when there is one; browsing works without it (verified live, see
    /// docs/nexus/live-probe.md).
    pub async fn graphql<T: DeserializeOwned>(&self, query: &str, variables: Value) -> Result<T> {
        let (v, _) = self.graphql_raw(query, variables).await?;
        serde_json::from_value(v).map_err(|e| Error::Decode(e.to_string()))
    }

    /// The `data` object as JSON, plus the response headers.
    pub async fn graphql_raw(&self, query: &str, variables: Value) -> Result<(Value, HeaderSnapshot)> {
        let req = self.http.post(&self.v2_url).json(&serde_json::json!({ "query": query, "variables": variables }));
        let (resp, snap) = self.send(req, false).await?;
        let body: GqlResponse = resp.json().await?;
        if let Some(errs) = body.errors.filter(|e| !e.is_empty()) {
            return Err(Error::GraphQl(errs.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; ")));
        }
        Ok((body.data.unwrap_or(Value::Null), snap))
    }

    pub async fn v1_get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let (resp, _) = self.send(self.http.get(format!("{}{}", self.v1_base, path)), true).await?;
        resp.json().await.map_err(Error::from)
    }

    pub async fn v1_post<T: DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<T> {
        let (resp, _) = self.send(self.http.post(format!("{}{}", self.v1_base, path)).form(form), true).await?;
        resp.json().await.map_err(Error::from)
    }

    pub async fn v1_delete<T: DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<T> {
        let (resp, _) = self.send(self.http.delete(format!("{}{}", self.v1_base, path)).form(form), true).await?;
        resp.json().await.map_err(Error::from)
    }
}

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<Value>,
    errors: Option<Vec<GqlError>>,
}
#[derive(Deserialize)]
struct GqlError { message: String }

fn snapshot(status: u16, h: &HeaderMap) -> HeaderSnapshot {
    let mut names: Vec<String> = h.keys().map(|k| k.as_str().to_owned()).filter(|n| n != "set-cookie").collect();
    names.sort();
    names.dedup();
    let rate_limit = h.iter()
        .filter(|(k, _)| k.as_str().starts_with("x-rl-") || k.as_str() == "retry-after")
        .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    HeaderSnapshot { status, names, rate_limit }
}

async fn message_of(resp: reqwest::Response) -> String {
    let text = resp.text().await.unwrap_or_default();
    serde_json::from_str::<Value>(&text).ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
        .unwrap_or_else(|| text.chars().take(200).collect())
}
