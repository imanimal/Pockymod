//! Signing in with a Nexus Mods account: OAuth 2.0 with PKCE, as modding.wiki's OAuth guide describes it for public
//! apps like mod managers. The browser shows Nexus's approval page, Nexus sends the browser back to a one-time
//! listener on 127.0.0.1, and the app trades the code for tokens. There is no client secret.
//! UNVERIFIED: not yet run against the live service. Nexus issues the client ID on registration.

use super::rest::NexusUser;
use crate::credentials::Secret;
use crate::error::{Error, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const AUTHORIZE_URL: &str = "https://users.nexusmods.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://users.nexusmods.com/oauth/token";
pub const REVOKE_URL: &str = "https://users.nexusmods.com/oauth/revoke";
/// The client ID Nexus issues on registration. Empty until then, and the app only offers a personal API key.
const CLIENT_ID: &str = "";
/// The callback listener's port. Nexus registers the whole address, so it never changes.
pub const CALLBACK_PORT: u16 = 41477;
const CALLBACK_PATH: &str = "/callback";
/// How long the approval page may take before the attempt gives up.
pub const GIVE_UP: Duration = Duration::from_secs(10 * 60);

/// The callback port. `POCKYMOD_NEXUS_CALLBACK_PORT` changes it for testing with another client, such as the guide's
/// `public_test` on 8089.
pub fn callback_port() -> u16 {
    std::env::var("POCKYMOD_NEXUS_CALLBACK_PORT").ok().and_then(|p| p.trim().parse().ok()).unwrap_or(CALLBACK_PORT)
}

pub fn redirect_uri(port: u16) -> String { format!("http://127.0.0.1:{port}{CALLBACK_PATH}") }

/// The client ID, or None while Nexus hasn't issued one. `POCKYMOD_NEXUS_CLIENT_ID` overrides it for testing.
pub fn client_id() -> Option<String> {
    std::env::var("POCKYMOD_NEXUS_CLIENT_ID").ok().filter(|s| !s.trim().is_empty())
        .or_else(|| (!CLIENT_ID.is_empty()).then(|| CLIENT_ID.to_string()))
}

fn b64(b: &[u8]) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b) }

/// URL-safe text from `bytes` random bytes.
fn random(bytes: usize) -> Result<String> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).map_err(|e| Error::Invalid(format!("Couldn't start sign-in: {e}")))?;
    Ok(b64(&b))
}

/// PKCE: the verifier stays in the app, the challenge goes out with the approval page.
pub struct Pkce { pub verifier: String, pub challenge: String }

impl Pkce {
    /// A 64-character verifier (the spec asks for 43 to 128).
    pub fn new() -> Result<Pkce> { Ok(Pkce::from_verifier(random(48)?)) }
    pub fn from_verifier(verifier: String) -> Pkce {
        let challenge = b64(&Sha256::digest(verifier.as_bytes()));
        Pkce { verifier, challenge }
    }
}

/// The `state` that ties the browser's return to this attempt.
pub fn new_state() -> Result<String> { random(24) }

pub fn authorize_url(client_id: &str, redirect: &str, state: &str, challenge: &str) -> String {
    let mut u = url::Url::parse(AUTHORIZE_URL).expect("a valid constant");
    u.query_pairs_mut()
        .append_pair("client_id", client_id).append_pair("response_type", "code").append_pair("scope", "")
        .append_pair("redirect_uri", redirect).append_pair("state", state)
        .append_pair("code_challenge_method", "S256").append_pair("code_challenge", challenge);
    u.into()
}

/// The one-time listener the browser comes back to.
pub struct Callback { listener: TcpListener, pub port: u16 }

impl Callback {
    pub async fn bind(port: u16) -> Result<Callback> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrInUse => Error::Invalid(format!("Another program is using port {port}, which signing in needs. Close it and try again, or paste an API key")),
            _ => Error::Io(e),
        })?;
        let port = listener.local_addr()?.port();
        Ok(Callback { listener, port })
    }

    /// Wait for the browser to come back with this attempt's `state`, answer it with a short page, and return the
    /// code. Anything else (a favicon request, a tab left from an older attempt) is answered and ignored.
    pub async fn code(self, state: &str) -> Result<Secret> {
        loop {
            let (mut sock, _) = self.listener.accept().await?;
            let Ok(Ok(target)) = tokio::time::timeout(Duration::from_secs(5), request_target(&mut sock)).await else { continue };
            let Ok(u) = url::Url::parse(&format!("http://127.0.0.1{target}")) else { continue };
            if u.path() != CALLBACK_PATH { respond(&mut sock, "404 Not Found", "Nothing here.").await; continue; }
            let q: HashMap<String, String> = u.query_pairs().into_owned().collect();
            if q.get("state").map(String::as_str) != Some(state) {
                respond(&mut sock, "400 Bad Request", "This sign-in has expired. Start again from Pockymod.").await;
                continue;
            }
            if let Some(code) = q.get("code").filter(|c| !c.is_empty()) {
                respond(&mut sock, "200 OK", "You're signed in. Close this tab and go back to Pockymod.").await;
                return Ok(Secret::new(code.clone()));
            }
            respond(&mut sock, "200 OK", "Sign-in didn't finish. Go back to Pockymod to try again.").await;
            return Err(match q.get("error").map(String::as_str) {
                Some("access_denied") => Error::Cancelled,
                other => Error::Invalid(format!("Nexus sign-in failed: {}", q.get("error_description").map(String::as_str).or(other).unwrap_or("no reason given"))),
            });
        }
    }
}

/// The path and query of an HTTP GET.
async fn request_target(sock: &mut TcpStream) -> std::io::Result<String> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = sock.read(&mut chunk).await?;
        if n == 0 { break; }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let line = head.lines().next().unwrap_or("");
    Ok(line.strip_prefix("GET ").and_then(|l| l.split(' ').next()).unwrap_or("").to_string())
}

async fn respond(sock: &mut TcpStream, status: &str, text: &str) {
    let body = format!("<!doctype html><meta charset=utf-8><title>Pockymod</title>\
        <body style=\"background:#111;color:#eee;font:16px system-ui,sans-serif;padding:3em\"><p>{text}</p></body>");
    let msg = format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = sock.write_all(msg.as_bytes()).await;
    let _ = sock.shutdown().await;
}

/// What Nexus hands back: an access token for API requests and a refresh token for the next one.
#[derive(Clone, Debug)]
pub struct Tokens { pub access: Secret, pub refresh: Option<Secret>, pub expires_at: i64 }

impl Tokens {
    /// A saved sign-in at start: only the refresh token is kept, so the first request fetches an access token.
    pub fn from_refresh(refresh: Secret) -> Tokens { Tokens { access: Secret::new(String::new()), refresh: Some(refresh), expires_at: 0 } }
    /// True when the access token runs out within a minute.
    pub fn expiring(&self, now: i64) -> bool { self.expires_at - 60 <= now }
}

#[derive(serde::Deserialize)]
struct TokenResponse { access_token: String, refresh_token: Option<String>, expires_in: Option<i64> }

/// Trade the code from the callback for tokens.
pub async fn exchange(http: &reqwest::Client, token_url: &str, client_id: &str, code: &Secret, verifier: &str, redirect: &str) -> Result<Tokens> {
    post_token(http, token_url, &[("grant_type", "authorization_code"), ("client_id", client_id), ("code", code.expose()),
        ("code_verifier", verifier), ("redirect_uri", redirect), ("scope", "")]).await
}

/// A new access token. When Nexus doesn't send a new refresh token, the old one stays in use.
pub async fn refresh(http: &reqwest::Client, token_url: &str, client_id: &str, refresh: &Secret) -> Result<Tokens> {
    let mut t = post_token(http, token_url, &[("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh.expose())]).await?;
    if t.refresh.is_none() { t.refresh = Some(refresh.clone()); }
    Ok(t)
}

async fn post_token(http: &reqwest::Client, token_url: &str, form: &[(&str, &str)]) -> Result<Tokens> {
    let resp = http.post(token_url).form(form).send().await?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        let r: TokenResponse = resp.json().await.map_err(|e| Error::Decode(e.to_string()))?;
        return Ok(Tokens { access: Secret::new(r.access_token), refresh: r.refresh_token.map(Secret::new),
            expires_at: chrono::Utc::now().timestamp() + r.expires_in.unwrap_or(3600) });
    }
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    let error = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
    // RFC 6749: a code or refresh token that is no good is a 400 with invalid_grant
    if status == 401 || (status == 400 && error == "invalid_grant") { return Err(Error::SignedOut); }
    let message = body.get("error_description").and_then(|v| v.as_str()).unwrap_or(error);
    Err(Error::Http { status, message: if message.is_empty() { "Sign-in failed".into() } else { message.to_string() } })
}

/// Tell Nexus the sign-in is over. Best effort: signing out here doesn't wait on it.
pub async fn revoke(http: &reqwest::Client, client_id: &str, token: &Secret) {
    let _ = http.post(REVOKE_URL).form(&[("client_id", client_id), ("token", token.expose())]).send().await;
}

/// The account named in the access token, which Nexus issues as a JWT with `user.id`, `user.username` and
/// `user.membership_roles`. Only used for the name and Premium when the API doesn't answer; the signature isn't checked.
pub fn user_from_token(access: &Secret) -> Option<NexusUser> {
    let payload = access.expose().split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let u = v.get("user")?;
    let roles: Vec<&str> = u.get("membership_roles").and_then(|r| r.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
    Some(NexusUser {
        user_id: u.get("id")?.as_u64()?, name: u.get("username")?.as_str()?.to_string(),
        is_premium: roles.contains(&"premium"), is_supporter: roles.contains(&"supporter"), profile_url: None,
    })
}

/// A token endpoint that answers once with `reply` and hands back the request body.
#[cfg(test)]
pub(crate) async fn token_server(reply: &'static str, status: &'static str) -> (String, tokio::task::JoinHandle<String>) {
    let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("http://127.0.0.1:{}/oauth/token", l.local_addr().unwrap().port());
    let task = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // headers, then as much body as Content-Length says
        loop {
            let n = s.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(i) = text.find("\r\n\r\n") {
                let len: usize = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                if buf.len() >= i + 4 + len || n == 0 { break; }
            }
        }
        let msg = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
        s.write_all(msg.as_bytes()).await.unwrap();
        let text = String::from_utf8_lossy(&buf).to_string();
        text.split("\r\n\r\n").nth(1).unwrap_or("").to_string()
    });
    (url, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_the_sha256_of_the_verifier() {
        // the challenge as Python's hashlib and base64.urlsafe_b64encode work it out
        let p = Pkce::from_verifier("dBjftJeZ4CVP-mJ3NXRm6r3ZsTFbOXz-KiEO7pi8aWM".into());
        assert_eq!(p.challenge, "jNjZ31f8x2z3Khw8q4i4ODzDTuG7Qtg4Tjc1s8DzJwg");
        let fresh = Pkce::new().unwrap();
        assert_eq!(fresh.verifier.len(), 64);
        assert!(fresh.verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn authorize_url_carries_the_pkce_challenge() {
        let u = url::Url::parse(&authorize_url("pockymod", &redirect_uri(41477), "st8", "chal")).unwrap();
        let q: HashMap<String, String> = u.query_pairs().into_owned().collect();
        assert_eq!(u.host_str(), Some("users.nexusmods.com"));
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:41477/callback");
        assert_eq!((q["code_challenge_method"].as_str(), q["code_challenge"].as_str(), q["state"].as_str()), ("S256", "chal", "st8"));
    }

    async fn get(port: u16, path: &str) -> (u16, String) {
        let r = reqwest::get(format!("http://127.0.0.1:{port}{path}")).await.unwrap();
        (r.status().as_u16(), r.text().await.unwrap())
    }

    #[tokio::test]
    async fn callback_ignores_other_states_and_returns_the_code() {
        let cb = Callback::bind(0).await.unwrap();
        let port = cb.port;
        let wait = tokio::spawn(async move { cb.code("right").await });
        assert_eq!(get(port, "/favicon.ico").await.0, 404);
        let (status, page) = get(port, "/callback?code=old&state=wrong").await;
        assert!(status == 400 && page.contains("expired"));
        let (status, page) = get(port, "/callback?code=abc&state=right").await;
        assert!(status == 200 && page.contains("signed in"));
        assert_eq!(wait.await.unwrap().unwrap().expose(), "abc");
    }

    #[tokio::test]
    async fn callback_reports_a_refusal() {
        let cb = Callback::bind(0).await.unwrap();
        let port = cb.port;
        let wait = tokio::spawn(async move { cb.code("s").await });
        get(port, "/callback?error=access_denied&state=s").await;
        assert!(matches!(wait.await.unwrap(), Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn exchange_sends_the_verifier_and_reads_the_tokens() {
        let (url, server) = token_server(r#"{"access_token":"acc","refresh_token":"ref","expires_in":21600,"token_type":"Bearer"}"#, "200 OK").await;
        let t = exchange(&reqwest::Client::new(), &url, "pockymod", &Secret::new("the-code"), "the-verifier", "http://127.0.0.1:41477/callback").await.unwrap();
        let body = server.await.unwrap();
        for part in ["grant_type=authorization_code", "client_id=pockymod", "code=the-code", "code_verifier=the-verifier", "redirect_uri=http%3A%2F%2F127.0.0.1%3A41477%2Fcallback"] {
            assert!(body.contains(part), "{part} missing from {body}");
        }
        assert_eq!((t.access.expose(), t.refresh.unwrap().expose()), ("acc", "ref"));
        assert!(t.expires_at > chrono::Utc::now().timestamp() + 21000);
    }

    #[tokio::test]
    async fn refresh_keeps_the_old_refresh_token_and_spots_a_dead_one() {
        let (url, _) = token_server(r#"{"access_token":"acc2","expires_in":60}"#, "200 OK").await;
        let t = refresh(&reqwest::Client::new(), &url, "pockymod", &Secret::new("ref")).await.unwrap();
        assert_eq!(t.refresh.unwrap().expose(), "ref");
        let (url, _) = token_server(r#"{"error":"invalid_grant"}"#, "400 Bad Request").await;
        assert!(matches!(refresh(&reqwest::Client::new(), &url, "pockymod", &Secret::new("ref")).await, Err(Error::SignedOut)));
    }

    #[test]
    fn user_comes_from_the_token() {
        let payload = b64(br#"{"sub":"1","user":{"id":4242,"username":"vee","membership_roles":["member","premium"]}}"#);
        let u = user_from_token(&Secret::new(format!("e30.{payload}.sig"))).unwrap();
        assert_eq!((u.user_id, u.name.as_str(), u.is_premium, u.is_supporter), (4242, "vee", true, false));
        assert!(user_from_token(&Secret::new("not-a-jwt")).is_none());
    }
}
