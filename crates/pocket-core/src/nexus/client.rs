//! HTTP client for the Nexus API: v2 GraphQL for browsing, v1 REST for account, files and download links.
//! Every request carries `Application-Name` and `Application-Version`. Requests for an account carry either a
//! sign-in's access token (`Authorization: Bearer`, which v1 and v2 both take) or a personal key (`APIKEY`).
//! A request is never retried automatically: a 429 stops all requests until the reset.

use super::oauth::{self, Tokens};
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
/// Called with new tokens after a refresh, or None when the sign-in is over, so they can be saved.
pub type TokenSaver = Arc<dyn Fn(Option<&Tokens>) + Send + Sync>;

/// How requests prove the account.
#[derive(Clone)]
pub enum Auth { Key(Secret), SignIn(Tokens) }

pub struct NexusClient {
    http: reqwest::Client,
    /// For file downloads: no limit on the whole transfer (a big file on a free account takes many minutes),
    /// only on a stalled connection.
    dl: reqwest::Client,
    auth: RwLock<Option<Auth>>,
    /// Held while an access token is refreshed: a refresh token may only work once.
    refreshing: tokio::sync::Mutex<()>,
    saver: RwLock<Option<TokenSaver>>,
    token_url: String,
    client_id: Option<String>,
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
        Ok(NexusClient {
            http, dl, auth: RwLock::new(key.map(Auth::Key)), refreshing: Default::default(), saver: RwLock::new(None), token_url: oauth::TOKEN_URL.into(), client_id: oauth::client_id(),
            rate: Mutex::new(RateLimit::default()), observer: RwLock::new(None), v1_base: v1_base.into(), v2_url: v2_url.into(),
        })
    }

    /// Where sign-ins are refreshed, and as which app.
    pub fn with_token_endpoint(mut self, token_url: &str, client_id: Option<&str>) -> Self {
        self.token_url = token_url.into();
        self.client_id = client_id.map(Into::into);
        self
    }

    pub fn set_key(&self, key: Option<Secret>) { self.set_auth(key.map(Auth::Key)); }
    pub fn set_auth(&self, auth: Option<Auth>) { *self.auth.write().unwrap() = auth; }
    pub fn auth(&self) -> Option<Auth> { self.auth.read().unwrap().clone() }
    /// True with either a sign-in or a key.
    pub fn has_key(&self) -> bool { self.auth.read().unwrap().is_some() }
    pub fn set_token_saver(&self, f: TokenSaver) { *self.saver.write().unwrap() = Some(f); }
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
        let signed_in = matches!(self.auth(), Some(Auth::SignIn(_)));
        let req = match (self.auth_header().await?, needs_key) {
            (Some((name, value)), _) => req.header(name, value),
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
            401 => Err(if signed_in { Error::SignedOut } else { Error::BadKey }),
            403 => {
                let msg = message_of(resp).await;
                // UNVERIFIED: v1 answers 403 with a Premium message when a free account asks for a download link
                if msg.to_lowercase().contains("premium") { Err(Error::NeedsPremium) } else { Err(Error::Http { status, message: msg }) }
            }
            _ => Err(Error::Http { status, message: message_of(resp).await }),
        }
    }

    /// The header that proves the account. A sign-in's access token is refreshed first when it is about to run out.
    async fn auth_header(&self) -> Result<Option<(&'static str, String)>> {
        let now = Utc::now().timestamp();
        let bearer = |t: &Tokens| Some(("Authorization", format!("Bearer {}", t.access.expose())));
        match self.auth() {
            None => return Ok(None),
            Some(Auth::Key(k)) => return Ok(Some(("APIKEY", k.expose().to_owned()))),
            Some(Auth::SignIn(t)) if !t.expiring(now) => return Ok(bearer(&t)),
            Some(Auth::SignIn(_)) => {}
        }
        let _one = self.refreshing.lock().await;
        // another request may have refreshed it while this one waited
        let t = match self.auth() {
            Some(Auth::SignIn(t)) if t.expiring(now) => t,
            Some(Auth::SignIn(t)) => return Ok(bearer(&t)),
            Some(Auth::Key(k)) => return Ok(Some(("APIKEY", k.expose().to_owned()))),
            None => return Ok(None),
        };
        let saver = self.saver.read().unwrap().clone();
        let fresh = match (t.refresh.as_ref(), self.client_id.as_deref()) {
            (Some(r), Some(id)) => oauth::refresh(&self.http, &self.token_url, id, r).await,
            _ => Err(Error::SignedOut),
        };
        match fresh {
            Ok(fresh) => {
                self.set_auth(Some(Auth::SignIn(fresh.clone())));
                if let Some(s) = saver { s(Some(&fresh)); }
                Ok(bearer(&fresh))
            }
            Err(Error::SignedOut) => {
                self.set_auth(None);
                if let Some(s) = saver { s(None); }
                Err(Error::SignedOut)
            }
            // offline or Nexus is down: the next request tries again
            Err(e) => Err(e),
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
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if let Some(m) = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned)) { return m; }
    // an error page (Cloudflare, maintenance) or nothing at all: no use showing its markup
    if text.trim().is_empty() || text.trim_start().starts_with('<') {
        return if status.is_server_error() { "Nexus is having trouble right now. Try again in a few minutes".into() }
            else if status.as_u16() == 404 { "Not found on Nexus. It may have been removed or hidden".into() }
            else { status.canonical_reason().unwrap_or("No answer").into() };
    }
    text.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(c: &NexusClient) -> Arc<Mutex<Vec<Option<String>>>> {
        let log: Arc<Mutex<Vec<Option<String>>>> = Default::default();
        let l = log.clone();
        c.set_token_saver(Arc::new(move |t| l.lock().unwrap().push(t.and_then(|t| t.refresh.as_ref()).map(|r| r.expose().to_owned()))));
        log
    }

    #[tokio::test]
    async fn a_saved_sign_in_gets_its_access_token_once() {
        let (url, _) = oauth::token_server(r#"{"access_token":"acc2","refresh_token":"ref2","expires_in":3600}"#, "200 OK").await;
        let c = NexusClient::new(None).unwrap().with_token_endpoint(&url, Some("pockymod"));
        let log = saved(&c);
        c.set_auth(Some(Auth::SignIn(Tokens::from_refresh(Secret::new("ref")))));
        assert_eq!(c.auth_header().await.unwrap(), Some(("Authorization", "Bearer acc2".to_string())));
        // the new token lasts an hour, so this one doesn't go to the endpoint (it only answers once)
        assert_eq!(c.auth_header().await.unwrap(), Some(("Authorization", "Bearer acc2".to_string())));
        assert_eq!(*log.lock().unwrap(), vec![Some("ref2".to_string())]);
    }

    #[tokio::test]
    async fn a_dead_refresh_token_signs_out() {
        let (url, _) = oauth::token_server(r#"{"error":"invalid_grant"}"#, "400 Bad Request").await;
        let c = NexusClient::new(None).unwrap().with_token_endpoint(&url, Some("pockymod"));
        let log = saved(&c);
        c.set_auth(Some(Auth::SignIn(Tokens::from_refresh(Secret::new("ref")))));
        assert!(matches!(c.auth_header().await, Err(Error::SignedOut)));
        assert!(!c.has_key());
        assert_eq!(*log.lock().unwrap(), vec![None]);
    }

    #[tokio::test]
    async fn a_key_goes_in_the_apikey_header() {
        let c = NexusClient::new(Some(Secret::new("k"))).unwrap();
        assert_eq!(c.auth_header().await.unwrap(), Some(("APIKEY", "k".to_string())));
    }
}
