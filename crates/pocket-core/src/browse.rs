//! The in-app browser: one `mods()` call per page, cached by filter, sort and offset; derived filter counts in one
//! aliased call, cached too; detail fetched only when a mod is opened and refreshed when its `updatedAt` moves.

use crate::cyberpunk::classify::{classify, Classification};
use crate::db::{now, Db};
use crate::error::Result;
use crate::nexus::graphql::{facets_from, BrowseQuery, Facets, GAME_ID};
use crate::nexus::types::{ModCard, ModDetail, ModFile, ModPage};
use crate::nexus::NexusClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Mutex;

pub const PAGE_TTL: i64 = 10 * 60;
pub const COUNTS_TTL: i64 = 30 * 60;
pub const FILE_LIST_LIMIT: u32 = 400;

/// Filters Nexus cannot do, applied to this machine's data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalFilter {
    pub show_hidden: bool,
    /// Some(true): only mods downloaded here; Some(false): only ones not downloaded.
    pub downloaded: Option<bool>,
    /// Skip the derived filter counts (discovery shelves do not show them).
    pub skip_counts: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    #[serde(flatten)]
    pub card: ModCard,
    pub classification: Classification,
    pub hidden: bool,
    pub ignored: bool,
    pub downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResult {
    pub total: u64,
    pub offset: u32,
    pub count: u32,
    pub nodes: Vec<CardView>,
    /// Mods on this page left out by the local filters.
    pub filtered_on_page: u32,
    pub facets: Facets,
    /// "fw_<key>" and "type_<key>" → count.
    pub derived: Vec<(String, u64)>,
    pub from_cache: bool,
}

const CARDS_BY_ID: &str = "query ByIds($ids: [CompositeDomainWithIdInput!]!, $count: Int) { legacyModsByDomain(ids: $ids, count: $count) { totalCount nodesCount nodes { modId uid name summary author uploader { name memberId } thumbnailUrl downloads endorsements fileSize createdAt updatedAt version category modCategory { categoryId name } adultContent status tags { id name } modRequirements { nexusRequirements { totalCount nodes { modId modName externalRequirement } } } } } }";

pub async fn search(client: &NexusClient, db: &Mutex<Db>, q: &BrowseQuery, local: &LocalFilter) -> Result<BrowseResult> {
    let size = q.page_size();
    let (page, from_cache, derived) = if local.downloaded == Some(true) {
        // the "downloaded" view is a local list, looked up by ID in one request
        let ids: Vec<u32> = db.lock().unwrap().downloaded_mod_ids()?.into_iter().collect();
        let slice: Vec<u32> = ids.iter().copied().skip(q.offset as usize).take(size as usize).collect();
        let nodes = if slice.is_empty() { Vec::new() } else { cards_by_id(client, &slice).await? };
        (ModPage { total_count: ids.len() as u64, nodes_count: nodes.len() as u64, facets_data: None, nodes }, false, Vec::new())
    } else {
        let key = format!("page:{}", q.cache_key());
        let counts_key = format!("counts:{}", BrowseQuery { offset: 0, count: 0, sort: Default::default(), ascending: false, ..q.clone() }.cache_key());
        let (cached_page, cached_counts) = {
            let d = db.lock().unwrap();
            (d.cache_get(&key, PAGE_TTL, now())?, d.cache_get(&counts_key, COUNTS_TTL, now())?)
        };
        let page_fut = async {
            match cached_page.as_deref().and_then(|s| serde_json::from_str::<ModPage>(s).ok()) {
                Some(p) => Ok((p, true)),
                None => {
                    let p = client.mods_page(q).await?;
                    db.lock().unwrap().cache_put(&key, &serde_json::to_string(&p).unwrap_or_default(), now())?;
                    Ok::<_, crate::Error>((p, false))
                }
            }
        };
        let counts_fut = async {
            if local.skip_counts { return Vec::new(); }
            if let Some(c) = cached_counts.as_deref().and_then(|s| serde_json::from_str::<Vec<(String, u64)>>(s).ok()) { return c; }
            // counts are a nicety: a failure leaves the chips without numbers, the page still shows
            match client.derived_counts(q).await {
                Ok(c) => { let _ = db.lock().unwrap().cache_put(&counts_key, &serde_json::to_string(&c).unwrap_or_default(), now()); c }
                Err(_) => Vec::new(),
            }
        };
        let (page, derived) = tokio::join!(page_fut, counts_fut);
        let (page, from_cache) = page?;
        (page, from_cache, derived)
    };

    let (flags, downloaded) = { let d = db.lock().unwrap(); (d.flags()?, d.downloaded_mod_ids()?) };
    let mut nodes = Vec::with_capacity(page.nodes.len());
    let mut filtered = 0;
    for card in page.nodes {
        let hidden = flags.hidden.contains(&card.mod_id);
        let is_dl = downloaded.contains(&card.mod_id);
        if (hidden && !local.show_hidden) || (local.downloaded == Some(false) && is_dl) { filtered += 1; continue; }
        let files = db.lock().unwrap().file_lists_for(card.mod_id)?;
        let classification = classify(&card, &files);
        nodes.push(CardView { ignored: flags.ignored.contains(&card.mod_id), hidden, downloaded: is_dl, classification, card });
    }
    Ok(BrowseResult { total: page.total_count, offset: q.offset, count: size, nodes, filtered_on_page: filtered, facets: facets_from(&page.facets_data), derived, from_cache })
}

pub async fn cards_by_id(client: &NexusClient, ids: &[u32]) -> Result<Vec<ModCard>> {
    let list: Vec<_> = ids.iter().map(|id| json!({ "gameDomain": crate::GAME_DOMAIN, "modId": id })).collect();
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct D { legacy_mods_by_domain: ModPage }
    let d: D = client.graphql(CARDS_BY_ID, json!({ "ids": list, "count": ids.len() })).await?;
    Ok(d.legacy_mods_by_domain.nodes)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailView {
    pub detail: ModDetail,
    pub files: Vec<ModFile>,
    pub classification: Classification,
    /// The archive file list Nexus has for the main file, if indexed.
    pub file_list: Option<Vec<String>>,
    pub hidden: bool,
    pub ignored: bool,
    pub downloaded: bool,
    pub game_id: u32,
}

/// The newest MAIN file, else the newest current file.
pub fn main_file(files: &[ModFile]) -> Option<&ModFile> {
    files.iter().filter(|f| f.category == "MAIN").max_by_key(|f| f.date)
        .or_else(|| files.iter().filter(|f| f.is_current()).max_by_key(|f| f.date))
}

/// A file's "line": its name without version numbers and filler words, so "Body 4K v1.2" and "Body 4K 1.3" are
/// one line and "Body 2K" another. Updates follow a line; installing an optional file does not replace the main one.
pub fn file_line(name: &str) -> String {
    let lower = name.to_lowercase();
    let words: Vec<String> = lower.split(|c: char| !c.is_alphanumeric() && c != '.').filter(|w| !w.is_empty())
        .filter(|w| {
            let v = w.trim_start_matches('v');
            let version = !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.') && (v.contains('.') || w.starts_with('v') || v.len() >= 3);
            !version && !["main", "file", "files", "update", "version", "ver", "the", "latest", "new"].contains(w)
        })
        .map(|w| w.trim_matches('.').to_string()).filter(|w| !w.is_empty()).collect();
    words.join(" ")
}

/// The file that updates `old` (by its id, or its name when Nexus no longer lists it): the newest current file of
/// the same line; else, for a mod with a single main file, that file.
pub fn successor<'a>(files: &'a [ModFile], old_file_id: u32, old_name: Option<&str>) -> Option<&'a ModFile> {
    let old = files.iter().find(|f| f.file_id == old_file_id);
    let line = old.map(|f| file_line(&f.name)).or_else(|| old_name.map(file_line)).filter(|l| !l.is_empty());
    let current = || files.iter().filter(|f| f.is_current());
    if let Some(l) = &line {
        if let Some(f) = current().filter(|f| &file_line(&f.name) == l).max_by_key(|f| f.date) { return Some(f); }
    }
    let mains: Vec<&ModFile> = files.iter().filter(|f| f.category == "MAIN").collect();
    // an optional file with no newer copy of its own has no update
    if old.is_some_and(|f| f.category == "OPTIONAL" || f.category == "MISCELLANEOUS") { return None; }
    if mains.len() == 1 { return Some(mains[0]); }
    if old.is_none() && line.is_none() { return main_file(files); }
    None
}

/// Whether installing `new` should take out an install of `old_file_id` of the same mod: same line, or (for a
/// mod with a single main file) the old main file. An optional file never takes out the main one.
pub fn replaces(files: &[ModFile], new: &ModFile, old_file_id: u32, old_name: Option<&str>) -> bool {
    if new.file_id == old_file_id { return true; }
    if new.category == "OPTIONAL" || new.category == "MISCELLANEOUS" {
        let old_line = files.iter().find(|f| f.file_id == old_file_id).map(|f| file_line(&f.name)).or_else(|| old_name.map(file_line));
        return old_line.is_some_and(|l| l == file_line(&new.name));
    }
    successor(files, old_file_id, old_name).is_some_and(|f| f.file_id == new.file_id)
}

/// A mod and its file list, from the cache when it matches `updated_at` (or any cached copy when None), else one
/// request. No file-content lookups: for update checks and matching.
pub async fn mod_files(client: &NexusClient, db: &Mutex<Db>, mod_id: u32, updated_at: Option<&str>) -> Result<(ModDetail, Vec<ModFile>)> {
    let cached = db.lock().unwrap().mod_get(mod_id, updated_at)?;
    if let Some((d, f)) = cached {
        if let (Ok(d), Ok(f)) = (serde_json::from_str(&d), serde_json::from_str(&f)) { return Ok((d, f)); }
    }
    let (d, f) = client.mod_detail(mod_id).await?;
    db.lock().unwrap().mod_put(mod_id, &d.updated_at, &serde_json::to_string(&d).unwrap_or_default(), &serde_json::to_string(&f).unwrap_or_default(), now())?;
    Ok((d, f))
}

pub async fn detail(client: &NexusClient, db: &Mutex<Db>, mod_id: u32, updated_at: Option<&str>) -> Result<DetailView> {
    let cached = db.lock().unwrap().mod_get(mod_id, updated_at)?;
    let (detail, files) = match cached.and_then(|(d, f)| Some((serde_json::from_str::<ModDetail>(&d).ok()?, serde_json::from_str::<Vec<ModFile>>(&f).ok()?))) {
        Some(x) => x,
        None => {
            let (d, f) = client.mod_detail(mod_id).await?;
            db.lock().unwrap().mod_put(mod_id, &d.updated_at, &serde_json::to_string(&d).unwrap_or_default(), &serde_json::to_string(&f).unwrap_or_default(), now())?;
            (d, f)
        }
    };
    // the file list: the main file's, else whatever Nexus indexed for the mod (kept under file 0)
    let file_list = match main_file(&files).map(|f| f.file_id) {
        Some(fid) => {
            let have = db.lock().unwrap().file_list_get(mod_id, fid)?;
            let list = match have {
                Some(l) => l, // fetched before; may be empty when Nexus has not indexed it
                None => {
                    let (_, rows) = client.file_contents(mod_id, Some(fid), FILE_LIST_LIMIT).await.unwrap_or((0, Vec::new()));
                    let paths: Vec<String> = rows.into_iter().map(|r| r.file_path).collect();
                    db.lock().unwrap().file_list_put(mod_id, fid, &paths, now())?;
                    if paths.is_empty() {
                        // fall back to the newest file of the mod that is indexed, kept under file 0
                        let (_, any) = client.file_contents(mod_id, None, FILE_LIST_LIMIT).await.unwrap_or((0, Vec::new()));
                        if let Some(newest) = any.iter().map(|r| r.file_id).max() {
                            let older: Vec<String> = any.into_iter().filter(|r| r.file_id == newest).map(|r| r.file_path).collect();
                            db.lock().unwrap().file_list_put(mod_id, 0, &older, now())?;
                        }
                    }
                    paths
                }
            };
            if !list.is_empty() { Some(list) } else { db.lock().unwrap().file_list_get(mod_id, 0)?.filter(|l| !l.is_empty()) }
        }
        None => None,
    };
    let card = card_of(&detail);
    let classification = classify(&card, file_list.as_deref().unwrap_or(&[]));
    let (flags, dl) = { let d = db.lock().unwrap(); (d.flags()?, d.downloaded_mod_ids()?) };
    Ok(DetailView { hidden: flags.hidden.contains(&mod_id), ignored: flags.ignored.contains(&mod_id), downloaded: dl.contains(&mod_id), detail, files, classification, file_list, game_id: GAME_ID })
}

/// The card view of a detail record, for classification.
pub fn card_of(d: &ModDetail) -> ModCard {
    ModCard {
        mod_id: d.mod_id, uid: d.uid.clone(), name: d.name.clone(), summary: d.summary.clone(), author: d.author.clone(), uploader: d.uploader.clone(),
        thumbnail_url: d.picture_url.clone(), downloads: d.downloads, endorsements: d.endorsements, file_size: d.file_size, created_at: d.created_at.clone(),
        updated_at: d.updated_at.clone(), version: d.version.clone(), category: d.category.clone(), mod_category: None, adult_content: d.adult_content,
        status: d.status.clone(), tags: d.tags.clone(), mod_requirements: Some(d.mod_requirements.clone()),
    }
}

// ── media: the community's screenshots and videos for the game ──

const MEDIA_QUERY: &str = "query Media($filter: MediaSearchFilter, $sort: [MediaSearchSort!], $offset: Int, $count: Int) {
  media(filter: $filter, sort: $sort, offset: $offset, count: $count) { totalCount
    nodes { __typename ... on Image { id name title url thumbnailUrl rating views createdAt owner { name } }
                       ... on Video { id title link thumbnailUrl rating views createdAt owner { name } } } } }";

/// One page of images or videos (checked live: `media` filters on gameId and type, sorts by rating, views, createdAt,
/// random). There is no per-mod filter, so a mod's own pictures come from its description instead.
pub async fn media(client: &NexusClient, db: &Mutex<Db>, kind: &str, sort: &str, offset: u32) -> Result<serde_json::Value> {
    let kind = if kind == "video" { "video" } else { "image" };
    let sort = match sort { "views" | "createdAt" => sort, _ => "rating" };
    let vars = json!({ "filter": { "gameId": [{ "value": GAME_ID.to_string() }], "type": [{ "value": kind }], "adultContent": [{ "value": false }] },
        "sort": [{ sort: { "direction": "DESC" } }], "offset": offset, "count": 30 });
    let key = format!("media:{vars}");
    if let Some(s) = db.lock().unwrap().cache_get(&key, PAGE_TTL, now())? {
        if let Ok(v) = serde_json::from_str(&s) { return Ok(v); }
    }
    let (data, _) = client.graphql_raw(MEDIA_QUERY, vars).await?;
    let page = data.get("media").cloned().unwrap_or(serde_json::Value::Null);
    db.lock().unwrap().cache_put(&key, &page.to_string(), now())?;
    Ok(page)
}

#[cfg(test)]
mod line_tests {
    use super::*;
    fn f(id: u32, name: &str, cat: &str, date: i64) -> ModFile {
        ModFile { file_id: id, name: name.into(), version: "1".into(), category: cat.into(), size: 1, size_in_bytes: None, date, primary: 0, uri: format!("{id}.zip"), description: None }
    }
    #[test]
    fn updates_follow_the_variant_and_optionals_stay() {
        assert_eq!(file_line("Body Mod 4K v1.2.3"), "body mod 4k");
        assert_eq!(file_line("Body Mod 4K - 1.3"), "body mod 4k");
        assert_ne!(file_line("Body Mod 2K 1.3"), file_line("Body Mod 4K 1.3"));
        let files = vec![f(1, "Body 2K 1.0", "OLD_VERSION", 1), f(2, "Body 4K 1.0", "OLD_VERSION", 1), f(3, "Body 2K 1.1", "MAIN", 2), f(4, "Body 4K 1.1", "MAIN", 3),
                         f(5, "Extra colours", "OPTIONAL", 2)];
        assert_eq!(successor(&files, 1, None).map(|x| x.file_id), Some(3), "the 2K user stays on 2K");
        assert_eq!(successor(&files, 2, None).map(|x| x.file_id), Some(4));
        assert!(!replaces(&files, &files[4], 3, None), "an optional file does not take out the main one");
        assert!(replaces(&files, &files[2], 1, None));
        assert!(!replaces(&files, &files[3], 1, None), "4K does not replace 2K");
        // a single-main mod that renamed its file between versions
        let one = vec![f(10, "TweakXL", "OLD_VERSION", 1), f(11, "TweakXL Release", "MAIN", 2)];
        assert_eq!(successor(&one, 10, None).map(|x| x.file_id), Some(11));
        // a file Nexus no longer lists, known only by an old name
        assert_eq!(successor(&files, 999, Some("Body 4K 0.9")).map(|x| x.file_id), Some(4));
    }
}
