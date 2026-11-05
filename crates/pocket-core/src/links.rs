//! Previews of the links in mod and collection descriptions, the way chat apps show a link: the page's title,
//! summary, picture, site name and icon, read from its Open Graph and Twitter tags (or its <title>). YouTube answers
//! through oEmbed and Discord invites through Discord's invite API; Nexus mod pages are previewed from the Nexus API
//! by the caller (nexusmods.com pages sit behind a bot check). Only public web addresses are fetched: nothing on
//! this computer or the local network, also after a redirect.
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use url::Url;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    /// The link as asked for.
    pub url: String,
    /// Where it ended up after redirects.
    pub final_url: String,
    /// "page", "image", "video", "file", "invite" or "mod".
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub site: Option<String>,
    pub icon: Option<String>,
    pub author: Option<String>,
    /// A Nexus mod link: the mod, and its downloads and endorsements.
    pub mod_id: Option<u32>,
    pub downloads: Option<u64>,
    pub endorsements: Option<u64>,
    /// Why the page could not be read (the link still opens).
    pub error: Option<String>,
}

const MAX_BODY: usize = 768 * 1024;

/// A link the app may fetch: http or https, no user name or password, and a public host.
pub fn safe_url(raw: &str) -> Option<Url> {
    let u = Url::parse(raw.trim()).ok()?;
    (matches!(u.scheme(), "http" | "https") && u.username().is_empty() && u.password().is_none() && public_host(&u)).then_some(u)
}

fn public_host(u: &Url) -> bool {
    match u.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            !(d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local") || d.ends_with(".internal") || d.ends_with(".lan") || d.ends_with(".home.arpa") || !d.contains('.'))
        }
        Some(url::Host::Ipv4(ip)) => public_ip(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => public_ip(IpAddr::V6(ip)),
        None => false,
    }
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => !(v.is_private() || v.is_loopback() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || v.is_documentation() || v.octets()[0] == 100 && (v.octets()[1] & 0xc0) == 64 || v.octets()[0] >= 224),
        IpAddr::V6(v) => {
            if let Some(m) = v.to_ipv4_mapped() { return public_ip(IpAddr::V4(m)); }
            let s = v.segments()[0];
            !(v.is_loopback() || v.is_unspecified() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80 || (s & 0xff00) == 0xff00)
        }
    }
}

/// A Cyberpunk 2077 mod page on Nexus Mods: its mod id.
pub fn nexus_mod_id(u: &Url) -> Option<u32> {
    let host = u.host_str()?.to_ascii_lowercase();
    if !(host == "nexusmods.com" || host.ends_with(".nexusmods.com")) { return None; }
    let mut seg = u.path_segments()?;
    if seg.next()? != "cyberpunk2077" || seg.next()? != "mods" { return None; }
    seg.next()?.parse().ok()
}

/// A YouTube video: its id.
pub fn youtube_id(u: &Url) -> Option<String> {
    let host = u.host_str()?.trim_start_matches("www.").trim_start_matches("m.").to_ascii_lowercase();
    let id = match host.as_str() {
        "youtu.be" => u.path_segments()?.next()?.to_string(),
        "youtube.com" | "youtube-nocookie.com" => {
            let mut seg = u.path_segments()?;
            match seg.next()? {
                "watch" => u.query_pairs().find(|(k, _)| k == "v")?.1.into_owned(),
                "embed" | "shorts" | "live" | "v" => seg.next()?.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    (id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(id)
}

fn discord_invite(u: &Url) -> Option<String> {
    let host = u.host_str()?.trim_start_matches("www.").to_ascii_lowercase();
    let mut seg = u.path_segments()?;
    let code = match host.as_str() {
        "discord.gg" => seg.next()?,
        "discord.com" | "discordapp.com" => { if seg.next()? != "invite" { return None; } seg.next()? }
        _ => return None,
    };
    (!code.is_empty() && code.len() <= 40 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then(|| code.to_string())
}

/// The client for previews: a time limit, a browser-like agent (many sites only send their tags to one), and
/// redirects followed only to public hosts. Made once.
pub fn client() -> &'static reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| reqwest::Client::builder()
        .user_agent(format!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 PocketMods/{}", crate::APP_VERSION))
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() >= 6 { a.error("too many redirects") }
            else if matches!(a.url().scheme(), "http" | "https") && public_host(a.url()) { a.follow() }
            else { a.stop() }
        }))
        .build()
        .unwrap_or_default())
}

/// Read a link's preview. A page that cannot be read gives a preview with `error` set, not an error.
pub async fn fetch(http: &reqwest::Client, url: &Url) -> LinkPreview {
    let base = LinkPreview { url: url.to_string(), final_url: url.to_string(), kind: "page".into(), site: url.host_str().map(|h| h.trim_start_matches("www.").to_string()), ..Default::default() };
    if let Some(id) = youtube_id(url) {
        if let Ok(p) = youtube(http, url, &id).await { return p; }
    }
    if let Some(code) = discord_invite(url) {
        if let Ok(p) = discord(http, url, &code).await { return p; }
    }
    match page(http, url).await {
        Ok(p) => p,
        Err(Error::Network(_)) => LinkPreview { error: Some("The site did not answer.".into()), ..base },
        Err(e) => LinkPreview { error: Some(e.to_string()), ..base },
    }
}

async fn youtube(http: &reqwest::Client, url: &Url, id: &str) -> Result<LinkPreview> {
    let watch = format!("https://www.youtube.com/watch?v={id}");
    let mut o = Url::parse("https://www.youtube.com/oembed?format=json").unwrap();
    o.query_pairs_mut().append_pair("url", &watch);
    let v: serde_json::Value = http.get(o).send().await?.error_for_status()?.json().await?;
    Ok(LinkPreview {
        url: url.to_string(), final_url: watch, kind: "video".into(),
        title: v["title"].as_str().map(str::to_owned),
        author: v["author_name"].as_str().map(str::to_owned),
        description: v["author_name"].as_str().map(|a| format!("A video by {a}")),
        image: Some(format!("https://img.youtube.com/vi/{id}/hqdefault.jpg")),
        site: Some("YouTube".into()), icon: Some("https://www.youtube.com/favicon.ico".into()),
        ..Default::default()
    })
}

async fn discord(http: &reqwest::Client, url: &Url, code: &str) -> Result<LinkPreview> {
    let v: serde_json::Value = http.get(format!("https://discord.com/api/v10/invites/{code}?with_counts=true")).send().await?.error_for_status()?.json().await?;
    let g = &v["guild"];
    let name = g["name"].as_str().ok_or_else(|| Error::Invalid("Not a server invite.".into()))?;
    let members = v["approximate_member_count"].as_u64();
    let online = v["approximate_presence_count"].as_u64();
    let counts = match (members, online) {
        (Some(m), Some(o)) => Some(format!("{m} members, {o} online")),
        (Some(m), None) => Some(format!("{m} members")),
        _ => None,
    };
    let about = g["description"].as_str().filter(|s| !s.is_empty());
    let icon = match (g["id"].as_str(), g["icon"].as_str()) { (Some(id), Some(ic)) => Some(format!("https://cdn.discordapp.com/icons/{id}/{ic}.png?size=128")), _ => None };
    let banner = match (g["id"].as_str(), g["splash"].as_str().or(g["banner"].as_str())) { (Some(id), Some(b)) => Some(format!("https://cdn.discordapp.com/splashes/{id}/{b}.jpg?size=480")), _ => None };
    Ok(LinkPreview {
        url: url.to_string(), final_url: url.to_string(), kind: "invite".into(),
        title: Some(name.to_string()),
        description: match (about, counts) { (Some(a), Some(c)) => Some(format!("{a} · {c}")), (Some(a), None) => Some(a.into()), (None, c) => c.map(|c| format!("Discord server · {c}")) },
        image: banner.or_else(|| icon.clone()), icon: icon.or_else(|| Some("https://discord.com/favicon.ico".into())),
        site: Some("Discord".into()),
        ..Default::default()
    })
}

async fn page(http: &reqwest::Client, url: &Url) -> Result<LinkPreview> {
    let mut r = http.get(url.clone()).header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5").header("Accept-Language", "en").send().await?;
    let status = r.status();
    let fin = r.url().clone();
    let ctype = r.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let size = r.content_length();
    let site = fin.host_str().map(|h| h.trim_start_matches("www.").to_string());
    let name = fin.path_segments().and_then(|mut s| s.next_back().map(str::to_owned)).filter(|s| !s.is_empty()).map(|s| percent_decode(&s));
    let base = LinkPreview { url: url.to_string(), final_url: fin.to_string(), kind: "page".into(), site: site.clone(), icon: favicon(&fin), ..Default::default() };
    if ctype.starts_with("image/") {
        return Ok(LinkPreview { kind: "image".into(), title: name, image: Some(fin.to_string()), description: Some(file_text(&ctype, size)), ..base });
    }
    if !ctype.is_empty() && !ctype.contains("html") && !ctype.contains("xml") {
        return Ok(LinkPreview { kind: "file".into(), title: name, description: Some(file_text(&ctype, size)), ..base });
    }
    let mut body = Vec::new();
    while let Some(c) = r.chunk().await? {
        body.extend_from_slice(&c);
        if body.len() >= MAX_BODY || find_ci(&body, b"</head>").is_some() { break; }
    }
    let html = String::from_utf8_lossy(&body);
    let mut p = parse(&html, &fin);
    p.url = url.to_string();
    p.final_url = fin.to_string();
    p.site = p.site.or(site);
    p.icon = p.icon.or(base.icon);
    if p.title.is_none() && !status.is_success() {
        p.error = Some(format!("The site answered {}.", status.as_u16()));
    }
    Ok(p)
}

fn file_text(ctype: &str, size: Option<u64>) -> String {
    let kind = ctype.split(';').next().unwrap_or("").trim();
    match size {
        Some(n) if n >= 1 << 20 => format!("{kind}, {:.1} MB", n as f64 / (1u64 << 20) as f64),
        Some(n) => format!("{kind}, {} KB", n.div_ceil(1024)),
        None => kind.to_string(),
    }
}

fn favicon(u: &Url) -> Option<String> { Some(format!("{}://{}/favicon.ico", u.scheme(), u.host_str()?)) }

fn find_ci(hay: &[u8], needle: &[u8]) -> Option<usize> { hay.windows(needle.len()).position(|w| w.eq_ignore_ascii_case(needle)) }

/// The preview from a page's <head>: Open Graph, then Twitter, then plain tags.
pub fn parse(html: &str, base: &Url) -> LinkPreview {
    let lower = html.to_ascii_lowercase();
    let head_end = lower.find("</head>").unwrap_or(lower.len());
    let (html, lower) = (&html[..head_end], &lower[..head_end]);
    let mut meta: Vec<(String, String)> = Vec::new();
    let mut icons: Vec<(String, String)> = Vec::new();
    let mut at = 0;
    while let Some(i) = lower[at..].find('<').map(|i| i + at) {
        let rest = &lower[i + 1..];
        let end = match html[i..].find('>') { Some(e) => i + e, None => break };
        if rest.starts_with("meta") || rest.starts_with("link") {
            let attrs = attributes(&html[i + 5..end]);
            let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            if rest.starts_with("meta") {
                if let (Some(k), Some(v)) = (get("property").or_else(|| get("name")).or_else(|| get("itemprop")), get("content")) {
                    meta.push((k.to_ascii_lowercase(), decode(&v)));
                }
            } else if let (Some(rel), Some(href)) = (get("rel"), get("href")) {
                icons.push((rel.to_ascii_lowercase(), decode(&href)));
            }
        }
        at = end + 1;
    }
    let first = |keys: &[&str]| keys.iter().find_map(|k| meta.iter().find(|(n, v)| n == k && !v.trim().is_empty()).map(|(_, v)| clean(v)));
    let title_tag = lower.find("<title").and_then(|s| {
        let open = s + lower[s..].find('>')? + 1;
        let close = open + lower[open..].find("</title")?;
        Some(clean(&decode(&html[open..close])))
    }).filter(|t| !t.is_empty());
    let abs = |v: String| base.join(v.trim()).ok().filter(|u| matches!(u.scheme(), "http" | "https")).map(|u| u.to_string());
    let icon = ["apple-touch-icon", "icon", "shortcut icon"].iter()
        .find_map(|want| icons.iter().find(|(rel, _)| rel.split_whitespace().any(|r| r == *want) || rel == want).map(|(_, h)| h.clone()))
        .and_then(abs);
    let kind = match first(&["og:type"]).as_deref() { Some(t) if t.starts_with("video") => "video", _ => "page" };
    LinkPreview {
        url: base.to_string(), final_url: base.to_string(), kind: kind.into(),
        title: first(&["og:title", "twitter:title"]).or(title_tag).map(|t| cut(&t, 200)),
        description: first(&["og:description", "twitter:description", "description"]).map(|t| cut(&t, 400)),
        image: first(&["og:image:secure_url", "og:image", "og:image:url", "twitter:image", "twitter:image:src", "image"]).and_then(abs),
        site: first(&["og:site_name", "application-name", "twitter:site"]).map(|s| cut(s.trim_start_matches('@'), 60)),
        icon,
        author: first(&["author", "article:author"]).filter(|a| !a.starts_with("http")),
        ..Default::default()
    }
}

/// `name="value"` pairs of a tag, names in lower case.
fn attributes(s: &str) -> Vec<(String, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') { i += 1; }
        let ns = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' { i += 1; }
        if ns == i { i += 1; continue; }
        let name = s[ns..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
            let value = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                i = vs;
                while i < b.len() && b[i] != q { i += 1; }
                let v = &s[vs..i.min(s.len())];
                i += 1;
                v
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() { i += 1; }
                &s[vs..i]
            };
            out.push((name, value.to_string()));
        } else {
            out.push((name, String::new()));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').filter(|&e| e <= 10);
        let ent = end.map(|e| &after[..e]);
        let ch = ent.and_then(|e| match e {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''), "nbsp" => Some(' '),
            "mdash" => Some('—'), "ndash" => Some('–'), "hellip" => Some('…'), "rsquo" => Some('’'), "lsquo" => Some('‘'), "ldquo" => Some('“'), "rdquo" => Some('”'),
            _ => e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")).and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok())).and_then(char::from_u32),
        });
        match (ch, end) {
            (Some(c), Some(e)) => { out.push(c); rest = &after[e + 1..]; }
            _ => { out.push('&'); rest = after; }
        }
    }
    out.push_str(rest);
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) { out.push((h * 16 + l) as u8); i += 3; continue; }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn clean(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") }

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n { return s.to_string(); }
    format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_open_graph_and_falls_back() {
        let base = Url::parse("https://example.com/docs/page").unwrap();
        let html = r#"<html><head><title>Plain title</title>
            <meta property="og:title" content="The &amp; title">
            <meta name='description' content='Short   summary'>
            <meta property=og:image content=/img/card.png>
            <link rel="shortcut icon" href="/fav.png">
            <meta property="og:site_name" content="Example"></head><body><meta property="og:title" content="ignored"></body>"#;
        let p = parse(html, &base);
        assert_eq!(p.title.as_deref(), Some("The & title"));
        assert_eq!(p.description.as_deref(), Some("Short summary"));
        assert_eq!(p.image.as_deref(), Some("https://example.com/img/card.png"));
        assert_eq!(p.icon.as_deref(), Some("https://example.com/fav.png"));
        assert_eq!(p.site.as_deref(), Some("Example"));
        let bare = parse("<head><title>Only &#39;this&#x27;</title></head>", &base);
        assert_eq!(bare.title.as_deref(), Some("Only 'this'"));
        assert!(bare.image.is_none());
    }

    #[test]
    fn only_public_addresses() {
        assert!(safe_url("https://github.com/x").is_some());
        assert!(safe_url("http://example.com").is_some());
        for bad in ["http://localhost:8080", "http://127.0.0.1/", "http://192.168.1.1/admin", "http://10.0.0.2", "http://[::1]/", "http://router/", "file:///C:/x", "https://user:pw@example.com", "http://169.254.169.254/latest", "javascript:alert(1)"] {
            assert!(safe_url(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn knows_nexus_youtube_and_discord() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert_eq!(nexus_mod_id(&u("https://www.nexusmods.com/cyberpunk2077/mods/4198?tab=files")), Some(4198));
        assert_eq!(nexus_mod_id(&u("https://nexusmods.com/cyberpunk2077/mods/107")), Some(107));
        assert_eq!(nexus_mod_id(&u("https://www.nexusmods.com/skyrim/mods/107")), None);
        assert_eq!(youtube_id(&u("https://youtu.be/dQw4w9WgXcQ?t=3")).as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id(&u("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=x")).as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id(&u("https://www.youtube.com/@channel")), None);
        assert_eq!(discord_invite(&u("https://discord.gg/redmodding")).as_deref(), Some("redmodding"));
        assert_eq!(discord_invite(&u("https://discord.com/invite/Epkq79kd96")).as_deref(), Some("Epkq79kd96"));
        assert_eq!(discord_invite(&u("https://discord.com/channels/1/2")), None);
    }

    #[test]
    fn file_names_are_decoded() {
        assert_eq!(percent_decode("My%20Mod%2Bfix.zip"), "My Mod+fix.zip");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
