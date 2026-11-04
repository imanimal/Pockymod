//! Collections: browse Nexus collections, check one before installing, and make, export and import local ones.
//!
//! A collection is a manifest (mods, chosen files, rules), never the mod files. Checked live on 2026-11-04:
//! `collectionsV2` filters on `gameDomain` and facets on `gameVersion`; `collectionRevision(slug, domainName)` lists
//! `modFiles { fileId optional updatePolicy file { … mod { modId name } } }`; files can be OLD_VERSION or ARCHIVED.
//! UNVERIFIED and not applied: the installer choices (FOMOD options) and rules inside the revision's
//! `collection.json` download; only the file list is used. Publishing to Nexus is an experiment behind a flag that
//! stays off: it is unconfirmed that a third-party app may publish (spec section 4).

use crate::cyberpunk::frameworks;
use crate::db::{now, Db};
use crate::error::{Error, Result};
use crate::nexus::types::ModDetail;
use crate::nexus::NexusClient;
use crate::queue::{is_archive_name, ItemState, QueueItem};
use rusqlite::params;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

const BROWSE_QUERY: &str = "query Collections($filter: CollectionsSearchFilter, $sort: [CollectionsSearchSort!], $offset: Int, $count: Int) {
  collectionsV2(filter: $filter, sort: $sort, offset: $offset, count: $count, facets: { gameVersion: [] }) { totalCount facetsData
    nodes { id slug name summary endorsements totalDownloads adultContent user { name } tileImage { url thumbnailUrl(size: med) } category { name }
      latestPublishedRevision { revisionNumber modCount totalSize adultContent gameVersions { reference } } } } }";

const REVISION_QUERY: &str = "query Revision($slug: String!, $revision: Int) { collectionRevision(slug: $slug, revision: $revision, domainName: \"cyberpunk2077\") {
  revisionNumber modCount totalSize adultContent gameVersions { reference } collection { name slug summary user { name } }
  modFiles { fileId optional updatePolicy version file { fileId name version size sizeInBytes uri category mod { modId name } } } } }";

fn bigint<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<u64, D::Error> {
    Ok(match Value::deserialize(d)? { Value::String(s) => s.parse().unwrap_or(0), Value::Number(n) => n.as_u64().unwrap_or(0), _ => 0 })
}
fn opt_bigint<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<u64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? { Some(Value::String(s)) => s.parse().ok(), Some(Value::Number(n)) => n.as_u64(), _ => None })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Named { pub name: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image { pub url: String, pub thumbnail_url: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersionRef { pub reference: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSummary {
    pub revision_number: u32,
    pub mod_count: u32,
    #[serde(deserialize_with = "bigint")]
    pub total_size: u64,
    pub adult_content: bool,
    #[serde(default)]
    pub game_versions: Vec<GameVersionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCard {
    pub id: u64,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub endorsements: u64,
    pub total_downloads: u64,
    pub adult_content: Option<bool>,
    pub user: Named,
    pub tile_image: Option<Image>,
    pub category: Option<Named>,
    pub latest_published_revision: Option<RevisionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionsPage { pub total_count: u64, #[serde(default)] pub facets_data: Option<Value>, pub nodes: Vec<CollectionCard> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionQuery {
    pub text: Option<String>,
    pub game_version: Option<String>,
    /// endorsements, downloads, updatedAt, createdAt, rating
    pub sort: Option<String>,
    pub include_adult: bool,
    pub offset: u32,
}

pub async fn browse(client: &NexusClient, db: &Mutex<Db>, q: &CollectionQuery) -> Result<CollectionsPage> {
    let mut filter = json!({ "gameDomain": [{ "value": crate::GAME_DOMAIN }] });
    if let Some(t) = q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) { filter["name"] = json!([{ "value": t, "op": "WILDCARD" }]); }
    if let Some(v) = &q.game_version { filter["gameVersion"] = json!([{ "value": v }]); }
    if !q.include_adult { filter["adultContent"] = json!([{ "value": false }]); }
    let key = match q.sort.as_deref() { Some(k @ ("downloads" | "updatedAt" | "createdAt" | "rating" | "endorsements")) => k, _ => "endorsements" };
    let vars = json!({ "filter": filter, "sort": [{ key: { "direction": "DESC" } }], "offset": q.offset, "count": 24 });
    let ck = format!("collections:{vars}");
    if let Some(s) = db.lock().unwrap().cache_get(&ck, 600, now())? {
        if let Ok(p) = serde_json::from_str(&s) { return Ok(p); }
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct D { collections_v2: CollectionsPage }
    let d: D = client.graphql(BROWSE_QUERY, vars).await?;
    db.lock().unwrap().cache_put(&ck, &serde_json::to_string(&d.collections_v2).unwrap_or_default(), now())?;
    Ok(d.collections_v2)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevModInfo { pub mod_id: u32, pub name: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevFile { pub file_id: u32, pub name: String, pub version: String, pub size: u64, #[serde(default, deserialize_with = "opt_bigint")] pub size_in_bytes: Option<u64>, pub uri: String, pub category: String, #[serde(rename = "mod")] pub m: RevModInfo }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevMod { pub file_id: u32, pub optional: bool, pub update_policy: String, pub version: String, pub file: Option<RevFile> }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevCollection { pub name: String, pub slug: String, pub summary: String, pub user: Named }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub revision_number: u32,
    pub mod_count: u32,
    #[serde(deserialize_with = "bigint")]
    pub total_size: u64,
    pub adult_content: bool,
    #[serde(default)]
    pub game_versions: Vec<GameVersionRef>,
    pub collection: RevCollection,
    pub mod_files: Vec<RevMod>,
}

pub async fn revision(client: &NexusClient, slug: &str, revision: Option<u32>) -> Result<Revision> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct D { collection_revision: Revision }
    let d: D = client.graphql(REVISION_QUERY, json!({ "slug": slug, "revision": revision })).await?;
    Ok(d.collection_revision)
}

// ── the local, shareable format ──

pub const FORMAT: &str = "pocket-mods.collection";

/// A local collection. Pins each mod's Nexus mod ID, file ID and MD5, and the dependency graph (`requires`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalCollection {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub game: String,
    pub game_version: Option<String>,
    pub created_at: String,
    pub mods: Vec<CollectionMod>,
    /// Mod IDs, first loads first.
    #[serde(default)]
    pub load_order: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMod {
    pub mod_id: u32,
    pub file_id: u32,
    pub name: String,
    #[serde(default)]
    pub file_name: String,
    pub version: String,
    /// MD5 of the archive, when it was downloaded through the app. Checked after download on install.
    pub md5: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub optional: bool,
    /// Nexus mod IDs this mod needs (in the collection or frameworks).
    #[serde(default)]
    pub requires: Vec<u32>,
    #[serde(default)]
    pub uri: String,
}

/// A local collection from the active profile's enabled mods (or the given installs).
pub fn create_local(db: &Db, name: &str, description: &str, game_version: Option<String>, only: Option<&[i64]>) -> Result<LocalCollection> {
    let active = crate::install::active_profile(db)?;
    let mut st = db.conn.prepare(
        "SELECT i.id, i.mod_id, i.file_id, i.name, i.version, d.md5, d.size, d.name FROM installs i
         JOIN profile_mods pm ON pm.install_id = i.id AND pm.profile_id = ?1
         LEFT JOIN downloads d ON d.mod_id = i.mod_id AND d.file_id = i.file_id
         WHERE pm.enabled = 1 AND i.mod_id < 900000000 ORDER BY pm.load_order")?;
    let rows: Vec<(i64, u32, u32, String, String, Option<String>, Option<i64>, Option<String>)> = st
        .query_map(params![active], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let rows: Vec<_> = rows.into_iter().filter(|r| only.is_none_or(|o| o.contains(&r.0))).collect();
    if rows.is_empty() { return Err(Error::Invalid("There are no Nexus mods enabled to put in a collection. Mods installed from a file cannot be shared.".into())); }
    let ids: BTreeSet<u32> = rows.iter().map(|r| r.1).collect();
    let mut mods = Vec::new();
    for (_, mod_id, file_id, mname, version, md5, size, file_name) in &rows {
        let mut requires: Vec<u32> = Vec::new();
        let mut uri = String::new();
        if let Some((d, f)) = db.mod_get(*mod_id, None)? {
            if let Ok(d) = serde_json::from_str::<ModDetail>(&d) {
                requires = d.mod_requirements.nexus_requirements.nodes.iter().map(|r| r.mod_id)
                    .filter(|id| ids.contains(id) || frameworks::by_mod_id(*id).is_some()).collect();
            }
            if let Ok(files) = serde_json::from_str::<Vec<crate::nexus::types::ModFile>>(&f) {
                if let Some(x) = files.iter().find(|x| x.file_id == *file_id) { uri = x.uri.clone(); }
            }
        }
        requires.sort();
        requires.dedup();
        mods.push(CollectionMod { mod_id: *mod_id, file_id: *file_id, name: mname.clone(), file_name: file_name.clone().unwrap_or_default(), version: version.clone(),
            md5: md5.clone(), size: size.unwrap_or(0) as u64, optional: false, requires, uri });
    }
    Ok(LocalCollection { format: FORMAT.into(), version: 1, name: name.trim().into(), description: description.trim().into(), game: crate::GAME_DOMAIN.into(),
        game_version, created_at: chrono::Utc::now().to_rfc3339(), load_order: rows.iter().map(|r| r.1).collect(), mods })
}

pub fn parse_local(text: &str) -> Result<LocalCollection> {
    if text.len() > 8 * 1024 * 1024 { return Err(Error::Invalid("That file is too large to be a collection.".into())); }
    let c: LocalCollection = serde_json::from_str(text).map_err(|e| Error::Invalid(format!("Not a Pocket Mods collection: {e}")))?;
    if c.format != FORMAT { return Err(Error::Invalid("Not a Pocket Mods collection file.".into())); }
    if c.version > 1 { return Err(Error::Invalid("This collection was made by a newer Pocket Mods. Update the app to open it.".into())); }
    if c.game != crate::GAME_DOMAIN { return Err(Error::Invalid(format!("This collection is for {}, not Cyberpunk 2077.", c.game))); }
    Ok(c)
}

/// Install order: requirements before the mods needing them (a topological sort of `requires`), ties in load order.
pub fn install_order(c: &LocalCollection) -> Vec<u32> {
    let pos: BTreeMap<u32, usize> = c.load_order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut ids: Vec<u32> = c.mods.iter().map(|m| m.mod_id).collect();
    ids.sort_by_key(|id| (frameworks::by_mod_id(*id).is_none(), pos.get(id).copied().unwrap_or(usize::MAX)));
    let reqs: BTreeMap<u32, Vec<u32>> = c.mods.iter().map(|m| (m.mod_id, m.requires.clone())).collect();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    fn visit(id: u32, reqs: &BTreeMap<u32, Vec<u32>>, seen: &mut BTreeSet<u32>, out: &mut Vec<u32>, depth: usize) {
        if depth > 64 || !seen.insert(id) { return; }
        for r in reqs.get(&id).into_iter().flatten() { if reqs.contains_key(r) { visit(*r, reqs, seen, out, depth + 1); } }
        out.push(id);
    }
    for id in ids { visit(id, &reqs, &mut seen, &mut out, 0); }
    out
}

// ── checks before installing ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub mod_id: u32,
    pub file_id: u32,
    pub name: String,
    pub file_name: String,
    pub version: String,
    pub size: u64,
    pub uri: String,
    pub optional: bool,
    pub md5: Option<String>,
    /// installed (this file), other (another file of the mod), missing, unavailable (removed or archived on Nexus)
    pub status: String,
    /// The file used instead when the pinned one is gone.
    pub replacement: Option<(u32, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    pub collection_versions: Vec<String>,
    pub current_patch: Option<String>,
    /// None when the collection's version cannot be compared (an executable version, or none given).
    pub game_version_ok: Option<bool>,
    pub entries: Vec<Entry>,
    pub missing: usize,
    pub installed: usize,
    pub unavailable: usize,
    pub needs_click: usize,
    pub download_bytes: u64,
}

/// "2.5.0.0" and "2.5" compare as patch 2.5; "3.0.80.51928" (an executable version) cannot be compared.
pub fn patch_of(reference: &str) -> Option<String> {
    let mut p = reference.trim().split('.');
    let major: u32 = p.next()?.parse().ok()?;
    if major != 2 { return None; }
    let minor = p.next().unwrap_or("0");
    let minor = minor.trim_end_matches('0');
    Some(format!("2.{}", if minor.is_empty() { "0" } else { minor }))
}

fn compare_versions(refs: &[String], current: Option<&str>) -> Option<bool> {
    let cur = current.and_then(patch_of)?;
    let comparable: Vec<String> = refs.iter().filter_map(|r| patch_of(r)).collect();
    if comparable.is_empty() { return None; }
    Some(comparable.contains(&cur))
}

fn summarize(name: String, versions: Vec<String>, patch: Option<String>, entries: Vec<Entry>, premium: bool) -> Check {
    let open: Vec<&Entry> = entries.iter().filter(|e| e.status != "installed").collect();
    let missing = open.iter().filter(|e| e.status != "unavailable" || e.replacement.is_some()).count();
    let unavailable = entries.iter().filter(|e| e.status == "unavailable" && e.replacement.is_none()).count();
    let needs_click = if premium { open.iter().filter(|e| !is_archive_name(&e.uri)).count() } else { missing };
    let download_bytes = open.iter().map(|e| e.size).sum();
    Check { name, game_version_ok: compare_versions(&versions, patch.as_deref()), collection_versions: versions, current_patch: patch, missing, installed: entries.len() - open.len(), unavailable, needs_click, download_bytes, entries }
}

fn installed_files(db: &Db) -> Result<BTreeSet<(u32, u32)>> {
    let mut st = db.conn.prepare("SELECT mod_id, file_id FROM installs")?;
    let s = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
    Ok(s)
}

pub fn check_revision(db: &Db, rev: &Revision, patch: Option<String>, premium: bool) -> Result<Check> {
    let have = installed_files(db)?;
    let have_mods: BTreeSet<u32> = have.iter().map(|x| x.0).collect();
    let entries = rev.mod_files.iter().filter_map(|m| {
        let f = m.file.as_ref()?;
        let gone = matches!(f.category.as_str(), "REMOVED" | "ARCHIVED");
        let status = if have.contains(&(f.m.mod_id, f.file_id)) { "installed" } else if gone { "unavailable" } else if have_mods.contains(&f.m.mod_id) { "other" } else { "missing" };
        Some(Entry { mod_id: f.m.mod_id, file_id: f.file_id, name: f.m.name.clone(), file_name: f.name.clone(), version: f.version.clone(),
            size: f.size_in_bytes.unwrap_or(f.size * 1024), uri: f.uri.clone(), optional: m.optional, md5: None, status: status.into(), replacement: None })
    }).collect();
    Ok(summarize(rev.collection.name.clone(), rev.game_versions.iter().map(|g| g.reference.clone()).collect(), patch, entries, premium))
}

pub fn check_local(db: &Db, c: &LocalCollection, patch: Option<String>, premium: bool) -> Result<Check> {
    let have = installed_files(db)?;
    let have_mods: BTreeSet<u32> = have.iter().map(|x| x.0).collect();
    let order = install_order(c);
    let mut entries: Vec<Entry> = c.mods.iter().map(|m| {
        let status = if have.contains(&(m.mod_id, m.file_id)) { "installed" } else if have_mods.contains(&m.mod_id) { "other" } else { "missing" };
        Entry { mod_id: m.mod_id, file_id: m.file_id, name: m.name.clone(), file_name: m.file_name.clone(), version: m.version.clone(), size: m.size,
            uri: m.uri.clone(), optional: m.optional, md5: m.md5.clone(), status: status.into(), replacement: None }
    }).collect();
    entries.sort_by_key(|e| order.iter().position(|id| *id == e.mod_id).unwrap_or(usize::MAX));
    Ok(summarize(c.name.clone(), c.game_version.clone().into_iter().collect(), patch, entries, premium))
}

/// For entries not installed, check the pinned file still exists on Nexus; if not, pick the mod's newest main file.
/// One detail request per mod checked, so it runs only for small lists or the entries the user is about to install.
pub async fn resolve_files(client: &NexusClient, db: &Mutex<Db>, check: &mut Check, limit: usize, premium: bool) -> Result<()> {
    let mut done = 0;
    for e in check.entries.iter_mut().filter(|e| e.status != "installed") {
        if done >= limit { break; }
        done += 1;
        let Ok(d) = crate::browse::detail(client, db, e.mod_id, None).await else { e.status = "unavailable".into(); continue };
        let pinned = d.files.iter().find(|f| f.file_id == e.file_id);
        match pinned {
            Some(f) if f.is_current() || f.category == "OLD_VERSION" => { if e.uri.is_empty() { e.uri = f.uri.clone(); } if e.size == 0 { e.size = f.bytes(); } }
            _ => {
                e.status = "unavailable".into();
                e.replacement = crate::browse::main_file(&d.files).map(|f| (f.file_id, f.version.clone()));
                if let Some(f) = crate::browse::main_file(&d.files) { e.uri = f.uri.clone(); e.size = f.bytes(); }
            }
        }
    }
    *check = summarize(check.name.clone(), check.collection_versions.clone(), check.current_patch.clone(), std::mem::take(&mut check.entries), premium);
    Ok(())
}

/// Queue items for a checked collection: missing entries (and optional ones if asked), pinned or replacement files.
pub fn queue_items(check: &Check, include_optional: bool, collection: &str) -> Vec<QueueItem> {
    check.entries.iter()
        .filter(|e| e.status != "installed" && (include_optional || !e.optional) && (e.status != "unavailable" || e.replacement.is_some()))
        .map(|e| {
            let (file_id, version) = e.replacement.clone().unwrap_or((e.file_id, e.version.clone()));
            QueueItem { id: 0, mod_id: e.mod_id, file_id, mod_name: e.name.clone(), file_name: e.file_name.clone(), version, uri: e.uri.clone(), size: e.size,
                required_by: Some(collection.to_string()), manual_only: !is_archive_name(&e.uri), state: ItemState::Waiting, nxm: None }
        }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local() -> LocalCollection {
        let m = |id: u32, req: &[u32]| CollectionMod { mod_id: id, file_id: id * 10, name: format!("m{id}"), file_name: String::new(), version: "1".into(), md5: Some("abc".into()), size: 100, optional: false, requires: req.to_vec(), uri: format!("m{id}.zip") };
        LocalCollection { format: FORMAT.into(), version: 1, name: "Mine".into(), description: String::new(), game: "cyberpunk2077".into(), game_version: Some("2.5".into()),
            created_at: "2026-11-04T00:00:00Z".into(), mods: vec![m(30, &[20]), m(20, &[4198]), m(4198, &[2380]), m(2380, &[])], load_order: vec![30, 20, 4198, 2380] }
    }

    #[test]
    fn requirements_install_first() {
        assert_eq!(install_order(&local()), [2380, 4198, 20, 30]);
    }

    #[test]
    fn roundtrip_and_validation() {
        let c = local();
        let text = serde_json::to_string_pretty(&c).unwrap();
        assert_eq!(parse_local(&text).unwrap(), c);
        assert!(parse_local(&text.replace("cyberpunk2077", "skyrim")).is_err());
        assert!(parse_local(&text.replace(FORMAT, "x")).is_err());
        assert!(parse_local("{}").is_err());
    }

    #[test]
    fn game_versions() {
        assert_eq!(patch_of("2.5.0.0").as_deref(), Some("2.5"));
        assert_eq!(patch_of("2.31").as_deref(), Some("2.31"));
        assert_eq!(patch_of("2.3.1.0").as_deref(), Some("2.3"));
        assert_eq!(patch_of("3.0.80.51928"), None);
        assert_eq!(compare_versions(&["2.5.0.0".into()], Some("2.5")), Some(true));
        assert_eq!(compare_versions(&["2.3.1.0".into()], Some("2.5")), Some(false));
        assert_eq!(compare_versions(&["3.0.80.51928".into()], Some("2.5")), None);
    }

    #[test]
    fn local_check_counts_clicks_and_skips_installed() {
        let db = Db::memory().unwrap();
        db.conn.execute("INSERT INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (2380, 23800, 'RED4ext', '1', 'framework', '[]', '', 0)", []).unwrap();
        let c = check_local(&db, &local(), Some("2.5".into()), false).unwrap();
        assert_eq!((c.installed, c.missing, c.needs_click, c.download_bytes), (1, 3, 3, 300));
        assert_eq!(c.game_version_ok, Some(true));
        let premium = check_local(&db, &local(), Some("2.5".into()), true).unwrap();
        assert_eq!(premium.needs_click, 0);
        let q = queue_items(&c, false, "Mine");
        assert_eq!(q.iter().map(|i| i.mod_id).collect::<Vec<_>>(), [4198, 20, 30]);
    }

    #[test]
    fn a_live_revision_parses_and_archived_files_are_unavailable() {
        let rev: Revision = serde_json::from_value(json!({ "revisionNumber": 483, "modCount": 2, "totalSize": "491562403", "adultContent": false,
            "gameVersions": [{ "reference": "2.5.0.0" }], "collection": { "name": "Welcome to Night City 2.5", "slug": "iszwwe", "summary": "", "user": { "name": "z9r" } },
            "modFiles": [
                { "fileId": 169009, "optional": false, "updatePolicy": "exact", "version": "2.70.1", "file": { "fileId": 169009, "name": "Unified Mod Settings", "version": "2.70.1", "size": 267, "sizeInBytes": "273562", "uri": "4c/ec/e3/4cece3d6", "category": "OLD_VERSION", "mod": { "modId": 28308, "name": "Unified Mod Settings" } } },
                { "fileId": 1, "optional": true, "updatePolicy": "exact", "version": "1", "file": { "fileId": 1, "name": "Old", "version": "1", "size": 5, "sizeInBytes": "5638", "uri": "x.zip", "category": "ARCHIVED", "mod": { "modId": 9, "name": "Gone" } } }
            ] })).unwrap();
        let db = Db::memory().unwrap();
        let c = check_revision(&db, &rev, Some("2.5".into()), false).unwrap();
        assert_eq!((c.missing, c.unavailable), (1, 1));
        assert_eq!(c.entries[0].size, 273562);
        assert!(queue_items(&c, true, "x").len() == 1, "archived without a replacement is left out");
    }
}
