//! For mod authors: a member's own Cyberpunk 2077 mods with their files, changelogs and requirements, and the
//! website pages that manage them.
//!
//! Verified live and anonymous on 2026-11-04 (docs/nexus/live-probe.md, "Author tools"):
//! - `mods(filter: { gameId, uploaderId: [{ value: "<memberId>", op: EQUALS }] })` lists one uploader's mods.
//!   `uploader` (the name, any case) works too; `author` is the free-text credit and also matches other people's mods.
//! - Anonymous requests see only published mods, with or without `viewUploaderHidden: true`.
//! - `user(id)` and `userByName(name)` answer anonymously, and so does `user.modAnalyticsByMonth` (totals for all
//!   games). `modAnalyticsForMonth` (per mod) is refused without the owner's key.
//! - `ModFile` has `groupId` (the update chain every version of a file shares), `totalDownloads`, `uniqueDownloads`,
//!   `scannedV2` (the virus scan) and `changelogText` (that version's changelog lines).
//! - `modRequirements.modsRequiringThisMod` counts and lists the mods that require this one.
//! - Several `modFiles` in one request, one alias per mod.
//! - v3 `GET /games/{domain}/mods/{id}` answers anonymously with `data.id`, the composite uid
//!   (gameId << 32 | modId, the same number as GraphQL `uid`), and `data.stats.unique_downloads`.
//!
//! UNVERIFIED (needs an author's key): hidden, unpublished and under-moderation mods showing to their owner with
//! `viewUploaderHidden` and a key; v3 `GET /mods/{uid}/files` and its shape; that its file ids (the Upload API's
//! "file id", formerly "file group id") are GraphQL's `groupId`. The website paths in `ModLinks` are unverified too:
//! the site sits behind a bot check that answers 403 to anything but a browser.

use crate::db::{now, Db};
use crate::error::{Error, Result};
use crate::nexus::graphql::GAME_ID;
use crate::nexus::types::{Requirement, Uploader};
use crate::nexus::NexusClient;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub const MY_MODS_TTL: i64 = 10 * 60;
pub const STATS_TTL: i64 = 60 * 60;
/// Mods per `mods()` page, and the most one author's list fetches.
const PAGE: u32 = 100;
const MAX_MODS: u32 = 500;
/// `modFiles` aliases per request.
const FILES_PER_REQUEST: usize = 10;

const MOD_FIELDS: &str = "modId uid name summary author version status category adultContent createdAt updatedAt downloads endorsements pictureUrl thumbnailUrl uploader { name memberId } modRequirements { nexusRequirements { totalCount } modsRequiringThisMod(count: 0) { totalCount } }";
const FILE_FIELDS: &str = "fileId name version category size sizeInBytes date primary uri description totalDownloads uniqueDownloads groupId scannedV2 changelogText";
const AUTHOR_FIELDS: &str = "memberId name avatar modCount uniqueModDownloads kudos recognizedAuthor joined modAnalyticsByMonth(pageSize: 1) { nodes { year month totalDownloads uniqueDownloads } }";

pub fn my_mods_query() -> String {
    format!("query MyMods($filter: ModsFilter, $offset: Int, $count: Int, $memberId: Int!) {{ mods(filter: $filter, sort: [{{ updatedAt: {{ direction: DESC }} }}], offset: $offset, count: $count, viewUploaderHidden: true) {{ totalCount nodes {{ {MOD_FIELDS} }} }} user(id: $memberId) {{ {AUTHOR_FIELDS} }} }}")
}

fn pinned_query() -> String {
    format!("query Pinned($ids: [CompositeDomainWithIdInput!]!, $count: Int) {{ legacyModsByDomain(ids: $ids, count: $count) {{ nodes {{ {MOD_FIELDS} }} }} }}")
}

pub const MEMBER_BY_NAME: &str = "query Member($name: String!) { userByName(name: $name) { memberId name } }";

pub const MANAGE_QUERY: &str = "query Manage($modId: ID!, $gameId: ID!) {
  mod(modId: $modId, gameId: $gameId) { modId uid name version status
    modRequirements {
      nexusRequirements(count: 50) { totalCount nodes { modId modName externalRequirement url notes } }
      dlcRequirements { notes gameExpansion { name } }
      modsRequiringThisMod(count: 50) { totalCount nodes { modId modName gameId externalRequirement notes } } } }
  modFiles(modId: $modId, gameId: $gameId) { fileId name version category size sizeInBytes date primary uri description totalDownloads uniqueDownloads groupId scannedV2 changelogText } }";

/// The mods of one uploader in Cyberpunk 2077. Needs `gameId` (or `gameDomainName`) beside `uploaderId`.
pub fn uploader_filter(member_id: u64) -> Value {
    json!({ "gameId": [{ "value": GAME_ID.to_string() }], "uploaderId": [{ "value": member_id.to_string(), "op": "EQUALS" }] })
}

pub fn my_mods_variables(member_id: u64, offset: u32) -> Value {
    json!({ "filter": uploader_filter(member_id), "offset": offset, "count": PAGE, "memberId": member_id })
}

/// One request with a `modFiles` alias per mod: `m4198: modFiles(modId: "4198", gameId: "3333") { … }`.
pub fn files_query(mod_ids: &[u32]) -> String {
    let parts: Vec<String> = mod_ids.iter().map(|id| format!("m{id}: modFiles(modId: \"{id}\", gameId: \"{GAME_ID}\") {{ {FILE_FIELDS} }}")).collect();
    format!("query Files {{ {} }}", parts.join(" "))
}

/// The v3 API's composite mod id: gameId << 32 | modId. Verified: it equals GraphQL `uid` and v3 `data.id`.
pub fn mod_uid(mod_id: u64) -> u64 {
    if mod_id >> 32 != 0 { mod_id } else { (u64::from(GAME_ID) << 32) | mod_id }
}

// ── what the UI gets ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub member_id: u64,
    pub name: String,
    #[serde(default)]
    pub avatar: Option<String>,
    /// Every game, not only Cyberpunk.
    #[serde(default)]
    pub mod_count: u64,
    /// Every game.
    #[serde(default)]
    pub unique_mod_downloads: u64,
    #[serde(default)]
    pub kudos: u64,
    #[serde(default)]
    pub recognized_author: bool,
    #[serde(default)]
    pub joined: Option<String>,
}

/// Downloads of all the member's mods in one month (every game).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Month { pub year: i32, pub month: u32, pub downloads: u64, pub unique_downloads: u64 }

/// One file version of a mod, with what an author needs: ids, downloads, scan state and the changelog lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorFile {
    pub file_id: u32,
    pub name: String,
    pub version: String,
    /// MAIN, UPDATE, OPTIONAL, OLD_VERSION, MISCELLANEOUS, REMOVED, ARCHIVED
    pub category: String,
    /// Kilobytes.
    pub size: u64,
    #[serde(default, deserialize_with = "opt_u64")]
    pub size_in_bytes: Option<u64>,
    /// Unix seconds.
    pub date: i64,
    #[serde(default)]
    pub primary: i32,
    pub uri: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub total_downloads: u64,
    #[serde(default)]
    pub unique_downloads: u64,
    /// The update chain all versions of this file share. Very likely the Upload API's "file id" (see module notes).
    #[serde(default, deserialize_with = "opt_u64")]
    pub group_id: Option<u64>,
    /// NOT_SCANNED, QUEUED, VERIFIED, QUARANTINED …
    #[serde(default)]
    pub scanned_v2: Option<String>,
    #[serde(default)]
    pub changelog_text: Vec<String>,
}

impl AuthorFile {
    pub fn is_current(&self) -> bool { matches!(self.category.as_str(), "MAIN" | "UPDATE" | "OPTIONAL" | "MISCELLANEOUS") }
}

/// Pages on the website for one mod. UNVERIFIED paths (see module notes); they open in the in-app Nexus window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModLinks {
    pub page: String,
    pub files: String,
    pub posts: String,
    pub bugs: String,
    pub logs: String,
    /// The mod's editor (details, media, files, requirements).
    pub edit: String,
    /// The editor's file step ("Manage files").
    pub edit_files: String,
}

pub fn mod_links(mod_id: u32) -> ModLinks {
    let page = format!("https://www.nexusmods.com/{}/mods/{mod_id}", crate::GAME_DOMAIN);
    let edit = format!("https://www.nexusmods.com/{}/mods/edit/?id={mod_id}", crate::GAME_DOMAIN);
    ModLinks {
        files: format!("{page}?tab=files"), posts: format!("{page}?tab=posts"), bugs: format!("{page}?tab=bugs"), logs: format!("{page}?tab=logs"),
        edit_files: format!("{edit}&step=files"), edit, page,
    }
}

/// A link that opens one file on the mod's Files tab.
pub fn file_link(mod_id: u32, file_id: u32) -> String { format!("https://www.nexusmods.com/{}/mods/{mod_id}?tab=files&file_id={file_id}", crate::GAME_DOMAIN) }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorLinks { pub profile: String, pub add_mod: String }

pub fn author_links(member_id: u64) -> AuthorLinks {
    AuthorLinks { profile: format!("https://www.nexusmods.com/users/{member_id}"), add_mod: format!("https://www.nexusmods.com/{}/mods/add", crate::GAME_DOMAIN) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MyMod {
    pub mod_id: u32,
    pub uid: String,
    pub name: String,
    pub summary: String,
    pub author: Option<String>,
    pub uploader: Option<Uploader>,
    pub version: String,
    /// published, hidden, not_published, under_moderation, removed … as Nexus sends it.
    pub status: String,
    pub category: String,
    pub adult_content: bool,
    pub created_at: String,
    pub updated_at: String,
    pub downloads: u64,
    pub endorsements: u64,
    /// From the v3 API (one request per mod, cached); None when it could not be read.
    pub unique_downloads: Option<u64>,
    pub picture_url: Option<String>,
    pub thumbnail_url: Option<String>,
    /// Mods this one requires, and mods that require it.
    pub requirements: u32,
    pub required_by: u32,
    /// Current files (main, update, optional, miscellaneous), newest first.
    pub files: Vec<AuthorFile>,
    /// Old, archived and removed versions, not listed here.
    pub old_files: u32,
    /// Added by hand (a new page, or a mod the member co-authors).
    pub pinned: bool,
    pub links: ModLinks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals { pub mods: u64, pub downloads: u64, pub unique_downloads: u64, pub endorsements: u64, pub files: u64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MyMods {
    pub author: Author,
    pub mods: Vec<MyMod>,
    pub totals: Totals,
    /// Every game.
    pub this_month: Option<Month>,
    /// Signed in as this member: with the key Nexus may also list hidden and unpublished mods (UNVERIFIED).
    pub own: bool,
    /// Pinned mods Nexus did not return (not published yet, or removed), with their pages.
    pub missing_pinned: Vec<Missing>,
    pub links: AuthorLinks,
    pub fetched_at: i64,
    #[serde(default)]
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Missing { pub mod_id: u32, pub links: ModLinks }

/// A version's changelog, from the lines Nexus keeps on its files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry { pub version: String, pub date: i64, pub lines: Vec<String> }

/// What the manage view of one mod needs, in one request (plus the cached unique downloads).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModManage {
    pub mod_id: u32,
    pub uid: String,
    pub name: String,
    pub version: String,
    pub status: String,
    pub unique_downloads: Option<u64>,
    /// Every file version, newest first.
    pub files: Vec<AuthorFile>,
    pub changelog: Vec<ChangelogEntry>,
    pub requirements: Vec<Requirement>,
    pub requirements_total: u32,
    /// DLC the mod needs, with the author's notes.
    pub dlc: Vec<String>,
    pub required_by: Vec<Requirement>,
    pub required_by_total: u32,
    pub links: ModLinks,
}

/// A file of a mod as the Upload API lists it: the id a new version goes to. UNVERIFIED shape (needs a key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadTarget {
    pub id: String,
    pub name: String,
    pub versions_count: Option<u64>,
    pub last_uploaded_at: Option<String>,
    pub is_active: Option<bool>,
    /// The GraphQL update chain this matches, by id or else by name.
    pub group_id: Option<u64>,
}

// ── parsing ──

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMod {
    mod_id: u32,
    uid: String,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    adult_content: Option<bool>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    endorsements: u64,
    #[serde(default)]
    picture_url: Option<String>,
    #[serde(default)]
    thumbnail_url: Option<String>,
    #[serde(default)]
    uploader: Option<Uploader>,
    #[serde(default)]
    mod_requirements: Option<RawCounts>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawCounts { #[serde(default)] nexus_requirements: Option<Count>, #[serde(default)] mods_requiring_this_mod: Option<Count> }
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Count { total_count: u32 }

/// One page of the author query: the mods, their total and the member.
pub struct AuthorPage { mods: Vec<RawMod>, pub total: u64, pub author: Option<Author>, pub month: Option<Month> }

pub fn parse_author_page(data: &Value) -> Result<AuthorPage> {
    let page = data.get("mods").ok_or_else(|| Error::Decode("no mods in the answer".into()))?;
    let mods: Vec<RawMod> = serde_json::from_value(page.get("nodes").cloned().unwrap_or(Value::Array(vec![]))).map_err(|e| Error::Decode(e.to_string()))?;
    let total = page.get("totalCount").and_then(Value::as_u64).unwrap_or(mods.len() as u64);
    let user = data.get("user").filter(|u| !u.is_null());
    let author = user.map(|u| serde_json::from_value::<Author>(u.clone())).transpose().map_err(|e| Error::Decode(e.to_string()))?;
    let month = user.and_then(|u| u.pointer("/modAnalyticsByMonth/nodes/0")).and_then(|n| Some(Month {
        year: n.get("year")?.as_i64()? as i32,
        month: n.get("month")?.as_u64()? as u32,
        downloads: num(n.get("totalDownloads")?)?,
        unique_downloads: num(n.get("uniqueDownloads")?)?,
    }));
    Ok(AuthorPage { mods, total, author, month })
}

fn parse_pinned(data: &Value) -> Result<Vec<RawMod>> {
    serde_json::from_value(data.pointer("/legacyModsByDomain/nodes").cloned().unwrap_or(Value::Array(vec![]))).map_err(|e| Error::Decode(e.to_string()))
}

/// The answer to `files_query`: mod id → its files, newest first.
pub fn parse_files(data: &Value) -> Result<BTreeMap<u32, Vec<AuthorFile>>> {
    let mut out = BTreeMap::new();
    for (alias, list) in data.as_object().into_iter().flatten() {
        let Some(id) = alias.strip_prefix('m').and_then(|s| s.parse::<u32>().ok()) else { continue };
        let mut files: Vec<AuthorFile> = serde_json::from_value(list.clone()).map_err(|e| Error::Decode(format!("files of {id}: {e}")))?;
        files.sort_by(|a, b| b.date.cmp(&a.date).then(b.file_id.cmp(&a.file_id)));
        out.insert(id, files);
    }
    Ok(out)
}

/// Unique downloads from a v3 `GET /games/{domain}/mods/{id}` answer.
pub fn parse_v3_unique(v: &Value) -> Option<u64> { v.pointer("/data/stats/unique_downloads").and_then(num) }

/// The files of a mod from v3 `GET /mods/{uid}/files`. Accepts `data.mod_files` or a bare `data` list, since the
/// shape could not be checked without a key.
pub fn parse_upload_targets(v: &Value) -> Vec<UploadTarget> {
    let list = v.pointer("/data/mod_files").or_else(|| v.get("data").filter(|d| d.is_array())).or_else(|| v.get("mod_files"))
        .and_then(Value::as_array).cloned().unwrap_or_default();
    list.iter().filter_map(|f| Some(UploadTarget {
        id: id_text(f.get("id")?)?,
        name: f.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
        versions_count: f.get("versions_count").and_then(num),
        last_uploaded_at: f.get("last_file_uploaded_at").and_then(Value::as_str).map(str::to_owned),
        is_active: f.get("is_active").and_then(Value::as_bool),
        group_id: None,
    })).collect()
}

/// Tie each Upload API file to a GraphQL update chain: the same id (bare or composite), else the same file name.
pub fn match_upload_targets(targets: &mut [UploadTarget], files: &[AuthorFile]) {
    let groups: BTreeMap<u64, &AuthorFile> = files.iter().filter_map(|f| f.group_id.map(|g| (g, f))).collect();
    for t in targets.iter_mut() {
        let id = t.id.parse::<u64>().ok();
        t.group_id = id.and_then(|id| groups.keys().copied().find(|g| *g == id || mod_uid(*g) == id))
            .or_else(|| {
                let same: BTreeSet<u64> = files.iter().filter(|f| f.name.eq_ignore_ascii_case(t.name.trim())).filter_map(|f| f.group_id).collect();
                (same.len() == 1).then(|| *same.iter().next().unwrap())
            });
    }
}

/// Each version's changelog lines, newest first, from the files that carry them.
pub fn changelog(files: &[AuthorFile]) -> Vec<ChangelogEntry> {
    let mut by_version: BTreeMap<String, ChangelogEntry> = BTreeMap::new();
    for f in files.iter().filter(|f| !f.changelog_text.is_empty()) {
        let e = by_version.entry(f.version.clone()).or_insert_with(|| ChangelogEntry { version: f.version.clone(), date: f.date, lines: Vec::new() });
        e.date = e.date.min(f.date);
        for l in &f.changelog_text {
            let l = l.trim();
            if !l.is_empty() && !e.lines.iter().any(|x| x == l) { e.lines.push(l.to_owned()); }
        }
    }
    let mut out: Vec<ChangelogEntry> = by_version.into_values().filter(|e| !e.lines.is_empty()).collect();
    out.sort_by(|a, b| b.date.cmp(&a.date));
    out
}

fn my_mod(m: RawMod, files: Option<&Vec<AuthorFile>>, unique: Option<u64>, pinned: bool) -> MyMod {
    let all = files.cloned().unwrap_or_default();
    let old = all.iter().filter(|f| !f.is_current()).count() as u32;
    let counts = m.mod_requirements.unwrap_or_default();
    MyMod {
        links: mod_links(m.mod_id),
        mod_id: m.mod_id, uid: m.uid, name: m.name, summary: m.summary.unwrap_or_default(), author: m.author, uploader: m.uploader, version: m.version,
        status: m.status, category: m.category, adult_content: m.adult_content.unwrap_or(false), created_at: m.created_at, updated_at: m.updated_at,
        downloads: m.downloads, endorsements: m.endorsements, unique_downloads: unique, picture_url: m.picture_url, thumbnail_url: m.thumbnail_url,
        requirements: counts.nexus_requirements.map(|c| c.total_count).unwrap_or(0),
        required_by: counts.mods_requiring_this_mod.map(|c| c.total_count).unwrap_or(0),
        files: all.into_iter().filter(AuthorFile::is_current).collect(), old_files: old, pinned,
    }
}

/// Put the parts together: the uploader's mods, then pinned ones not already there.
pub fn assemble(author: Author, month: Option<Month>, mods: Vec<RawModList>, files: &BTreeMap<u32, Vec<AuthorFile>>, unique: &BTreeMap<u32, u64>,
    pinned: &[u32], own: bool) -> MyMods {
    let mut seen = BTreeSet::new();
    let mut list = Vec::new();
    for RawModList(m, is_pin) in mods {
        if !seen.insert(m.mod_id) { continue; }
        let id = m.mod_id;
        list.push(my_mod(m, files.get(&id), unique.get(&id).copied(), is_pin || pinned.contains(&id)));
    }
    let missing_pinned = pinned.iter().copied().filter(|id| !seen.contains(id)).map(|mod_id| Missing { mod_id, links: mod_links(mod_id) }).collect();
    let totals = Totals {
        mods: list.len() as u64,
        downloads: list.iter().map(|m| m.downloads).sum(),
        unique_downloads: list.iter().filter_map(|m| m.unique_downloads).sum(),
        endorsements: list.iter().map(|m| m.endorsements).sum(),
        files: list.iter().map(|m| m.files.len() as u64).sum(),
    };
    let links = author_links(author.member_id);
    MyMods { author, mods: list, totals, this_month: month, own, missing_pinned, links, fetched_at: now(), from_cache: false }
}

/// A mod as returned by Nexus, and whether it came from the pinned list.
pub struct RawModList(RawMod, bool);

pub fn manage_from(data: &Value, unique: Option<u64>) -> Result<ModManage> {
    let m = data.get("mod").filter(|m| !m.is_null()).ok_or_else(|| Error::Invalid("Nexus has no such mod for Cyberpunk 2077, or it is not public yet.".into()))?;
    let mut files: Vec<AuthorFile> = serde_json::from_value(data.get("modFiles").cloned().unwrap_or(Value::Array(vec![]))).map_err(|e| Error::Decode(e.to_string()))?;
    files.sort_by(|a, b| b.date.cmp(&a.date).then(b.file_id.cmp(&a.file_id)));
    let reqs = |p: &str| -> Result<(Vec<Requirement>, u32)> {
        let page = m.pointer(p);
        let nodes = page.and_then(|x| x.get("nodes")).cloned().unwrap_or(Value::Array(vec![]));
        let total = page.and_then(|x| x.get("totalCount")).and_then(Value::as_u64).unwrap_or(0) as u32;
        let list: Vec<Requirement> = serde_json::from_value(nodes).map_err(|e| Error::Decode(e.to_string()))?;
        Ok((list, total))
    };
    let (requirements, requirements_total) = reqs("/modRequirements/nexusRequirements")?;
    let (required_by, required_by_total) = reqs("/modRequirements/modsRequiringThisMod")?;
    let dlc = m.pointer("/modRequirements/dlcRequirements").and_then(Value::as_array).into_iter().flatten().map(|d| {
        let name = d.pointer("/gameExpansion/name").and_then(Value::as_str).unwrap_or("A DLC");
        match d.get("notes").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()) { Some(n) => format!("{name} ({n})"), None => name.to_owned() }
    }).collect();
    let mod_id = m.get("modId").and_then(Value::as_u64).ok_or_else(|| Error::Decode("no modId".into()))? as u32;
    let text = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    Ok(ModManage {
        mod_id, uid: text("uid"), name: text("name"), version: text("version"), status: text("status"), unique_downloads: unique,
        changelog: changelog(&files), files, requirements, requirements_total, dlc, required_by, required_by_total, links: mod_links(mod_id),
    })
}

// ── what the user types ──

/// A member as typed: a member id, a profile link, or a user name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberRef { Id(u64), Name(String) }

pub fn member_ref(input: &str) -> Option<MemberRef> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() { return None; }
    if let Ok(n) = s.parse::<u64>() { return Some(MemberRef::Id(n)); }
    if let Ok(u) = url::Url::parse(s) {
        if !u.host_str().is_some_and(|h| h.ends_with("nexusmods.com")) { return None; }
        let seg: Vec<&str> = u.path_segments().map(|p| p.filter(|x| !x.is_empty()).collect()).unwrap_or_default();
        return match seg.as_slice() {
            ["users", id, ..] => id.parse().ok().map(MemberRef::Id),
            ["profile", name, ..] => Some(MemberRef::Name(name.to_string())),
            _ => None,
        };
    }
    Some(MemberRef::Name(s.trim_start_matches('@').to_owned()))
}

/// A Cyberpunk mod as typed: its id or a link to its page (any tab).
pub fn mod_ref(input: &str) -> Option<u32> {
    let s = input.trim();
    if let Ok(n) = s.parse::<u32>() { return (n > 0).then_some(n); }
    let u = url::Url::parse(s).ok()?;
    if !u.host_str().is_some_and(|h| h.ends_with("nexusmods.com")) { return None; }
    let seg: Vec<&str> = u.path_segments()?.filter(|x| !x.is_empty()).collect();
    match seg.as_slice() {
        // the editor: /cyberpunk2077/mods/edit/?id=123
        [game, "mods", "edit", ..] if *game == crate::GAME_DOMAIN => u.query_pairs().find(|(k, _)| k == "id").and_then(|(_, v)| v.parse().ok()),
        [game, "mods", id, ..] if *game == crate::GAME_DOMAIN => id.parse().ok(),
        _ => None,
    }
}

// ── live calls ──

pub async fn resolve_member(client: &NexusClient, input: &str) -> Result<u64> {
    match member_ref(input) {
        Some(MemberRef::Id(n)) => Ok(n),
        Some(MemberRef::Name(name)) => {
            let (d, _) = client.graphql_raw(MEMBER_BY_NAME, json!({ "name": name })).await?;
            d.pointer("/userByName/memberId").and_then(Value::as_u64)
                .ok_or_else(|| Error::Invalid(format!("Nexus has no member called {name}. Check the spelling, or paste the link to your profile.")))
        }
        None => Err(Error::Invalid("Type a Nexus user name, a member ID, or a link to a profile.".into())),
    }
}

/// The raw `data` of every author page, up to `MAX_MODS` mods.
pub async fn fetch_author_pages(client: &NexusClient, member_id: u64) -> Result<Vec<Value>> {
    let mut pages = Vec::new();
    let mut offset = 0;
    loop {
        let (d, _) = client.graphql_raw(&my_mods_query(), my_mods_variables(member_id, offset)).await?;
        let got = d.pointer("/mods/nodes").and_then(Value::as_array).map(|a| a.len() as u32).unwrap_or(0);
        let total = d.pointer("/mods/totalCount").and_then(Value::as_u64).unwrap_or(0) as u32;
        pages.push(d);
        offset += got;
        if got == 0 || offset >= total || offset >= MAX_MODS { break; }
    }
    Ok(pages)
}

/// The raw `data` of the files requests for these mods (several aliases per request).
pub async fn fetch_files(client: &NexusClient, mod_ids: &[u32]) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for chunk in mod_ids.chunks(FILES_PER_REQUEST) {
        let (d, _) = client.graphql_raw(&files_query(chunk), json!({})).await?;
        out.push(d);
    }
    Ok(out)
}

/// The v3 mod record, anonymous (it carries unique downloads, which GraphQL does not).
pub async fn fetch_v3_mod(client: &NexusClient, mod_id: u32) -> Result<Value> {
    let url = format!("{}/games/{}/mods/{mod_id}", super::UPLOAD_BASE, crate::GAME_DOMAIN);
    let r = client.http().get(url).header("Accept", "application/json").send().await?;
    let status = r.status().as_u16();
    if !r.status().is_success() { return Err(Error::Http { status, message: "v3 mod".into() }); }
    Ok(r.json().await?)
}

/// Unique downloads per mod, cached for an hour; mods whose number cannot be read are left out.
pub async fn unique_downloads(client: &NexusClient, db: &Mutex<Db>, mod_ids: &[u32], refresh: bool) -> BTreeMap<u32, u64> {
    let mut out = BTreeMap::new();
    let mut todo = Vec::new();
    for id in mod_ids {
        let hit = if refresh { None } else { db.lock().unwrap().cache_get(&format!("v3mod:{id}"), STATS_TTL, now()).ok().flatten() };
        match hit.and_then(|s| s.parse::<u64>().ok()) { Some(n) => { out.insert(*id, n); } None => todo.push(*id) }
    }
    for chunk in todo.chunks(6) {
        let got = futures_util::future::join_all(chunk.iter().map(|id| async move { (*id, fetch_v3_mod(client, *id).await) })).await;
        for (id, r) in got {
            if let Some(n) = r.ok().as_ref().and_then(parse_v3_unique) {
                let _ = db.lock().unwrap().cache_put(&format!("v3mod:{id}"), &n.to_string(), now());
                out.insert(id, n);
            }
        }
    }
    out
}

/// A member's Cyberpunk mods with their current files, plus the pinned ones. Cached for ten minutes.
pub async fn my_mods(client: &NexusClient, db: &Mutex<Db>, member_id: u64, pinned: &[u32], own: bool, refresh: bool) -> Result<MyMods> {
    let key = format!("creator:mods:{member_id}:{own}:{}", pinned.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
    if !refresh {
        if let Some(mut hit) = db.lock().unwrap().cache_get(&key, MY_MODS_TTL, now())?.and_then(|s| serde_json::from_str::<MyMods>(&s).ok()) {
            hit.from_cache = true;
            return Ok(hit);
        }
    }
    let pages = fetch_author_pages(client, member_id).await?;
    let mut author = None;
    let mut month = None;
    let mut mods: Vec<RawModList> = Vec::new();
    for p in &pages {
        let page = parse_author_page(p)?;
        author = author.or(page.author);
        month = month.or(page.month);
        mods.extend(page.mods.into_iter().map(|m| RawModList(m, false)));
    }
    let have: BTreeSet<u32> = mods.iter().map(|m| m.0.mod_id).collect();
    let extra: Vec<u32> = pinned.iter().copied().filter(|id| !have.contains(id)).collect();
    if !extra.is_empty() {
        let ids: Vec<Value> = extra.iter().map(|id| json!({ "gameDomain": crate::GAME_DOMAIN, "modId": id })).collect();
        let (d, _) = client.graphql_raw(&pinned_query(), json!({ "ids": ids, "count": extra.len() })).await?;
        mods.extend(parse_pinned(&d)?.into_iter().map(|m| RawModList(m, true)));
    }
    let author = author.ok_or_else(|| Error::Invalid(format!("Nexus has no member {member_id}.")))?;
    let ids: Vec<u32> = mods.iter().map(|m| m.0.mod_id).collect();
    let mut files = BTreeMap::new();
    for d in fetch_files(client, &ids).await? { files.extend(parse_files(&d)?); }
    let unique = unique_downloads(client, db, &ids, refresh).await;
    let out = assemble(author, month, mods, &files, &unique, pinned, own);
    db.lock().unwrap().cache_put(&key, &serde_json::to_string(&out).unwrap_or_default(), now())?;
    Ok(out)
}

/// Files, changelog and requirements of one mod, fresh each time (an author checks right after a change).
pub async fn manage(client: &NexusClient, db: &Mutex<Db>, mod_id: u32) -> Result<ModManage> {
    let (d, _) = client.graphql_raw(MANAGE_QUERY, json!({ "modId": mod_id.to_string(), "gameId": GAME_ID.to_string() })).await?;
    let unique = unique_downloads(client, db, &[mod_id], true).await.get(&mod_id).copied();
    manage_from(&d, unique)
}

/// The files the Upload API knows for a mod, matched to its update chains. Needs the author's key.
pub async fn upload_targets(client: &NexusClient, key: &str, mod_id: u32) -> Result<Vec<UploadTarget>> {
    let url = format!("{}/mods/{}/files", super::UPLOAD_BASE, mod_uid(u64::from(mod_id)));
    let r = client.http().get(url).header("apikey", key).header("Accept", "application/json").send().await?;
    let status = r.status().as_u16();
    if status == 401 || status == 403 { return Err(Error::Http { status, message: "Nexus did not list this mod's files for your key. Only the mod's owner and team can upload to it.".into() }); }
    if !r.status().is_success() { return Err(Error::Http { status, message: r.text().await.unwrap_or_default().chars().take(200).collect() }); }
    let v: Value = r.json().await?;
    let mut targets = parse_upload_targets(&v);
    let (d, _) = client.graphql_raw(&files_query(&[mod_id]), json!({})).await?;
    let files = parse_files(&d)?.remove(&mod_id).unwrap_or_default();
    match_upload_targets(&mut targets, &files);
    Ok(targets)
}

fn num(v: &Value) -> Option<u64> { v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())) }

fn id_text(v: &Value) -> Option<String> {
    match v { Value::String(s) if !s.is_empty() => Some(s.clone()), Value::Number(n) => Some(n.to_string()), _ => None }
}

fn opt_u64<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<u64>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.as_ref().and_then(num))
}

/// For the fixture recorder: keep only the newest `keep` files of each mod in a files answer.
pub fn trim_files_answer(data: &Value, keep: usize) -> Value {
    let mut out = Map::new();
    for (alias, list) in data.as_object().into_iter().flatten() {
        let mut files = list.as_array().cloned().unwrap_or_default();
        files.sort_by_key(|f| std::cmp::Reverse(f.get("date").and_then(Value::as_i64).unwrap_or(0)));
        // keep every current file, then the newest old ones
        let (cur, old): (Vec<Value>, Vec<Value>) = files.into_iter().partition(|f| matches!(f.get("category").and_then(Value::as_str), Some("MAIN" | "UPDATE" | "OPTIONAL" | "MISCELLANEOUS")));
        let mut kept = cur;
        kept.extend(old.into_iter().take(keep.saturating_sub(kept.len()).max(2)));
        out.insert(alias.clone(), Value::Array(kept));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Recorded live, anonymous, 2026-11-04 by `cargo run -p pocket-core --example creator_fixture`.
    const AUTHOR: &str = include_str!("../../tests/fixtures/creator/author-page.json");
    const FILES: &str = include_str!("../../tests/fixtures/creator/files.json");
    const MANAGE: &str = include_str!("../../tests/fixtures/creator/manage.json");
    const V3_MOD: &str = include_str!("../../tests/fixtures/creator/v3-mod.json");

    fn v(s: &str) -> Value { serde_json::from_str(s).unwrap() }

    #[test]
    fn parses_a_recorded_author_page() {
        let p = parse_author_page(&v(AUTHOR)).unwrap();
        let a = p.author.unwrap();
        assert_eq!(a.member_id, 108159138);
        assert!(a.unique_mod_downloads > 0 && a.mod_count > 0);
        assert!(p.month.as_ref().is_some_and(|m| m.downloads >= m.unique_downloads));
        assert_eq!(p.total as usize, p.mods.len());
        assert!(p.mods.iter().all(|m| m.uploader.as_ref().is_some_and(|u| u.member_id == 108159138)), "every mod is the uploader's");
        assert!(p.mods.iter().any(|m| m.mod_requirements.as_ref().and_then(|r| r.mods_requiring_this_mod.as_ref()).is_some_and(|c| c.total_count > 0)));
        // newest update first, as asked
        assert!(p.mods.windows(2).all(|w| w[0].updated_at >= w[1].updated_at));
    }

    #[test]
    fn parses_recorded_files_and_splits_current_from_old() {
        let files = parse_files(&v(FILES)).unwrap();
        assert!(!files.is_empty());
        let (_, list) = files.iter().find(|(_, l)| l.iter().any(|f| !f.is_current())).expect("a mod with old versions");
        assert!(list.windows(2).all(|w| w[0].date >= w[1].date), "newest first");
        assert!(list.iter().all(|f| f.group_id.is_some()), "every file has its update chain");
        assert!(list.iter().any(|f| f.size_in_bytes.is_some() && f.unique_downloads <= f.total_downloads));
    }

    #[test]
    fn assembles_cards_and_totals() {
        let p = parse_author_page(&v(AUTHOR)).unwrap();
        let files = parse_files(&v(FILES)).unwrap();
        let first = p.mods[0].mod_id;
        let unique = BTreeMap::from([(first, 1234u64)]);
        let mods = p.mods.into_iter().map(|m| RawModList(m, false)).collect();
        let my = assemble(p.author.unwrap(), p.month, mods, &files, &unique, &[first, 99_999_999], true);
        assert_eq!(my.totals.mods as usize, my.mods.len());
        assert_eq!(my.totals.downloads, my.mods.iter().map(|m| m.downloads).sum::<u64>());
        assert_eq!(my.totals.unique_downloads, 1234);
        assert_eq!(my.missing_pinned.iter().map(|m| m.mod_id).collect::<Vec<_>>(), vec![99_999_999], "a pinned mod Nexus did not return");
        let m = &my.mods[0];
        assert!(m.pinned && m.unique_downloads == Some(1234));
        assert!(m.files.iter().all(AuthorFile::is_current));
        assert!(my.mods.iter().any(|m| m.old_files > 0));
        assert_eq!(m.links.edit, format!("https://www.nexusmods.com/cyberpunk2077/mods/edit/?id={}", m.mod_id));
        assert!(m.links.files.ends_with("?tab=files"));
    }

    #[test]
    fn manage_view_has_changelog_and_both_requirement_lists() {
        let m = manage_from(&v(MANAGE), Some(5)).unwrap();
        assert_eq!(m.mod_id, 18839);
        assert!(m.requirements_total >= m.requirements.len() as u32 && !m.requirements.is_empty());
        assert!(m.required_by_total > 0 && !m.required_by.is_empty());
        assert!(!m.changelog.is_empty());
        assert!(m.changelog.windows(2).all(|w| w[0].date >= w[1].date), "newest version first");
        // one entry per version, lines not repeated (two files of 1.3.1 carry the same lines)
        let versions: BTreeSet<&str> = m.changelog.iter().map(|e| e.version.as_str()).collect();
        assert_eq!(versions.len(), m.changelog.len());
    }

    #[test]
    fn v3_mod_gives_uid_and_unique_downloads() {
        let x = v(V3_MOD);
        assert!(parse_v3_unique(&x).is_some_and(|n| n > 0));
        let id: u64 = x["data"]["id"].as_str().unwrap().parse().unwrap();
        let scoped: u64 = x["data"]["game_scoped_id"].as_str().unwrap().parse().unwrap();
        assert_eq!(mod_uid(scoped), id, "the composite id is gameId << 32 | modId");
        assert_eq!(mod_uid(id), id, "an id that is already composite stays");
    }

    #[test]
    fn upload_targets_parse_and_match_by_id_or_name() {
        let files = vec![file(1, "Main", 2_828_359), file(2, "Optional textures", 2_900_000)];
        let mut t = parse_upload_targets(&json!({ "data": { "mod_files": [
            { "id": "2828359", "name": "Main", "versions_count": 14, "is_active": true },
            { "id": "f6a1c2", "name": " optional TEXTURES" },
            { "id": "777", "name": "Something new" },
            { "id": "14315128897568", "name": "Renamed on the site" } ] } }));
        match_upload_targets(&mut t, &files);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].group_id, Some(2_828_359), "the same id");
        assert_eq!(t[0].versions_count, Some(14));
        assert_eq!(t[1].group_id, Some(2_900_000), "matched by name");
        assert_eq!(t[2].group_id, None);
        assert_eq!(t[3].group_id, Some(2_900_000), "a composite id");
        // a bare list in data works too
        assert_eq!(parse_upload_targets(&json!({ "data": [{ "id": 5, "name": "x" }] }))[0].id, "5");
    }

    fn file(id: u32, name: &str, group: u64) -> AuthorFile {
        AuthorFile { file_id: id, name: name.into(), version: "1".into(), category: "MAIN".into(), size: 1, size_in_bytes: None, date: 0, primary: 0, uri: String::new(),
            description: None, total_downloads: 0, unique_downloads: 0, group_id: Some(group), scanned_v2: None, changelog_text: vec![] }
    }

    #[test]
    fn reads_members_and_mods_as_typed() {
        assert_eq!(member_ref(" 108159138 "), Some(MemberRef::Id(108159138)));
        assert_eq!(member_ref("@psiberx"), Some(MemberRef::Name("psiberx".into())));
        assert_eq!(member_ref("https://www.nexusmods.com/users/108159138?tab=user+files"), Some(MemberRef::Id(108159138)));
        assert_eq!(member_ref("https://www.nexusmods.com/profile/psiberx/mods"), Some(MemberRef::Name("psiberx".into())));
        assert_eq!(member_ref("https://example.com/users/1"), None);
        assert_eq!(member_ref("  "), None);
        assert_eq!(mod_ref("4198"), Some(4198));
        assert_eq!(mod_ref("https://www.nexusmods.com/cyberpunk2077/mods/4198?tab=files"), Some(4198));
        assert_eq!(mod_ref("https://www.nexusmods.com/cyberpunk2077/mods/edit/?id=4198&step=files"), Some(4198));
        assert_eq!(mod_ref("https://www.nexusmods.com/skyrimspecialedition/mods/4198"), None);
        assert_eq!(mod_ref("0"), None);
    }

    #[test]
    fn queries_have_the_filter_and_one_alias_per_mod() {
        let f = uploader_filter(42);
        assert_eq!(f["gameId"][0]["value"], "3333");
        assert_eq!(f["uploaderId"][0], json!({ "value": "42", "op": "EQUALS" }));
        let q = files_query(&[1, 22]);
        assert!(q.contains("m1: modFiles(modId: \"1\", gameId: \"3333\")") && q.contains("m22: modFiles"));
        assert!(my_mods_query().contains("viewUploaderHidden: true"));
    }

    #[test]
    fn trimmed_fixture_keeps_current_files() {
        let d = json!({ "m1": [
            { "fileId": 1, "category": "OLD_VERSION", "date": 1 }, { "fileId": 2, "category": "OLD_VERSION", "date": 2 },
            { "fileId": 3, "category": "OLD_VERSION", "date": 3 }, { "fileId": 4, "category": "MAIN", "date": 4 } ] });
        let t = trim_files_answer(&d, 3);
        let ids: Vec<i64> = t["m1"].as_array().unwrap().iter().map(|f| f["fileId"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![4, 3, 2]);
    }
}
