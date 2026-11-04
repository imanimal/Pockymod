//! v2 GraphQL queries and the browse filter builder.
//!
//! Filter semantics checked against the live API on 2026-11-04 (docs/nexus/live-probe.md):
//! - several values in one field's list are ANDed (`categoryName: [A, B]` matches nothing);
//! - `filter: [{ op: OR, field: [A, B] }]` ORs them;
//! - date filters take Unix seconds as a string; ISO dates break the server's query parser;
//! - `modId` filters need `gameId`, not `gameDomainName`;
//! - aliases put several `mods()` counts in one request;
//! - free text: `nameStemmed` and `description` with `MATCHES`, sorted by relevance.

use super::types::*;
use crate::cyberpunk::frameworks::{self, Framework};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const GAME_ID: u32 = 3333;

const CARD_FIELDS: &str = "modId uid name summary author uploader { name memberId } thumbnailUrl downloads endorsements fileSize createdAt updatedAt version category modCategory { categoryId name } adultContent status tags { id name } modRequirements { nexusRequirements { totalCount nodes { modId modName externalRequirement } } }";

pub fn mods_page_query() -> String {
    format!("query Mods($filter: ModsFilter, $sort: [ModsSort!], $facets: ModsFacet, $offset: Int, $count: Int) {{ mods(filter: $filter, sort: $sort, facets: $facets, offset: $offset, count: $count) {{ totalCount nodesCount facetsData nodes {{ {CARD_FIELDS} }} }} }}")
}

pub const MOD_DETAIL_QUERY: &str = "query Detail($modId: ID!, $gameId: ID!) {
  mod(modId: $modId, gameId: $gameId) { modId uid name summary description version author uploader { name memberId } pictureUrl downloads endorsements fileSize createdAt updatedAt category adultContent status tags { id name }
    modRequirements { dlcRequirements { notes } nexusRequirements { totalCount nodes { modId modName externalRequirement url notes } } } }
  modFiles(modId: $modId, gameId: $gameId) { fileId name version category size sizeInBytes date primary uri description } }";

pub const FILE_CONTENTS_QUERY: &str = "query Contents($filter: ModFileContentSearchFilter, $count: Int, $offset: Int) {
  modFileContents(filter: $filter, count: $count, offset: $offset) { totalCount nodes { fileId modId filePath fileSize } } }";

/// Batch lookup by mod ID (library update checks): `legacyModsByDomain`.
pub const MODS_BY_ID_QUERY: &str = "query ByIds($ids: [CompositeDomainWithIdInput!]!, $count: Int) {
  legacyModsByDomain(ids: $ids, count: $count) { nodes { modId name version updatedAt thumbnailUrl } } }";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortKey { #[default] Downloads, Endorsements, Updated, Created, Relevance, Size, Name, Random }

/// Mod type: not a Nexus field, so the filter uses tags and text (heuristic; see `mod_type_group`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModTypeFilter { Redmod, Archive, Script, Framework }

/// Everything the browser can ask for. Sent from the UI as camelCase JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowseQuery {
    pub text: Option<String>,
    /// ORed.
    pub categories: Vec<String>,
    /// ANDed.
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub sort: SortKey,
    pub ascending: bool,
    /// Unix seconds.
    pub updated_after: Option<i64>,
    pub updated_before: Option<i64>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub downloads_min: Option<u64>,
    pub downloads_max: Option<u64>,
    pub endorsements_min: Option<u64>,
    /// Kilobytes, like the API's `fileSize`.
    pub size_min_kb: Option<u64>,
    pub size_max_kb: Option<u64>,
    pub include_adult: bool,
    /// Derived: each listed framework must be required (ANDed).
    pub frameworks: Vec<Framework>,
    pub mod_type: Option<ModTypeFilter>,
    /// Derived: updated on or after the installed patch's release (Unix seconds). Heuristic for "works with my game".
    pub compatible_since: Option<i64>,
    /// Seed for the random sort (the "Surprise me" shelf), so paging and caching stay stable.
    pub seed: Option<u32>,
    pub offset: u32,
    pub count: u32,
}

impl BrowseQuery {
    /// Stable key for the page cache: filter, sort, offset and count.
    pub fn cache_key(&self) -> String {
        let mut q = self.clone();
        q.text = q.text.map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty());
        q.categories.sort();
        q.tags.sort();
        q.exclude_tags.sort();
        q.frameworks.sort();
        serde_json::to_string(&q).unwrap_or_default()
    }

    pub fn page_size(&self) -> u32 { if self.count == 0 { 30 } else { self.count.min(100) } }
}

fn v(s: impl ToString) -> Value { json!({ "value": s.to_string() }) }
fn vop(s: impl ToString, op: &str) -> Value { json!({ "value": s.to_string(), "op": op }) }
fn iop(n: u64, op: &str) -> Value { json!({ "value": n, "op": op }) }
fn or_group(fields: Map<String, Value>) -> Value {
    let mut m = fields;
    m.insert("op".into(), json!("OR"));
    Value::Object(m)
}

/// Text that names a framework: its Nexus tag, or its words in the description or name.
fn framework_group(f: Framework) -> Value {
    let info = frameworks::info(f);
    let mut m = Map::new();
    if let Some(tag) = info.tag { m.insert("tag".into(), json!([v(tag)])); }
    m.insert("description".into(), Value::Array(info.words.iter().filter(|w| w.len() > 3).map(|w| vop(w, "MATCHES")).collect()));
    m.insert("name".into(), Value::Array(info.words.iter().filter(|w| w.len() > 3).map(|w| vop(w, "WILDCARD")).collect()));
    or_group(m)
}

/// HEURISTIC: Nexus has no mod-type field. The card's type is refined later from the archive's file list.
fn mod_type_group(t: ModTypeFilter) -> Value {
    let mut m = Map::new();
    match t {
        ModTypeFilter::Redmod => {
            m.insert("tag".into(), json!([v("REDMod")]));
            m.insert("description".into(), json!([vop("REDmod", "MATCHES")]));
        }
        ModTypeFilter::Script => {
            m.insert("categoryName".into(), json!([v("Scripts")]));
            m.insert("description".into(), json!([vop("redscript", "MATCHES"), vop("Cyber Engine Tweaks", "MATCHES")]));
        }
        ModTypeFilter::Framework => {
            m.insert("tag".into(), json!([v("Modder's Resource"), v("Utilities for Modders")]));
            m.insert("categoryName".into(), json!([v("Modders Resources")]));
        }
        ModTypeFilter::Archive => {
            m.insert("description".into(), json!([vop(".archive", "MATCHES"), vop("archive/pc/mod", "MATCHES")]));
        }
    }
    or_group(m)
}

/// The `ModsFilter` for a query, without paging.
pub fn build_filter(q: &BrowseQuery) -> Value {
    let mut f = Map::new();
    f.insert("gameDomainName".into(), json!([v(crate::GAME_DOMAIN)]));
    let mut groups: Vec<Value> = Vec::new();

    if let Some(text) = q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let mut m = Map::new();
        m.insert("nameStemmed".into(), json!([vop(text, "MATCHES")]));
        m.insert("description".into(), json!([vop(text, "MATCHES")]));
        m.insert("name".into(), json!([vop(text, "WILDCARD")]));
        groups.push(or_group(m));
    }
    if !q.categories.is_empty() {
        let mut m = Map::new();
        m.insert("categoryName".into(), Value::Array(q.categories.iter().map(v).collect()));
        groups.push(or_group(m));
    }
    let mut tags: Vec<Value> = q.tags.iter().map(v).collect();
    tags.extend(q.exclude_tags.iter().map(|t| vop(t, "NOT_EQUALS")));
    if !tags.is_empty() { f.insert("tag".into(), Value::Array(tags)); }

    let mut updated = Vec::new();
    if let Some(t) = q.updated_after.max(q.compatible_since) { updated.push(vop(t, "GTE")); }
    if let Some(t) = q.updated_before { updated.push(vop(t, "LTE")); }
    if !updated.is_empty() { f.insert("updatedAt".into(), Value::Array(updated)); }
    let mut created = Vec::new();
    if let Some(t) = q.created_after { created.push(vop(t, "GTE")); }
    if let Some(t) = q.created_before { created.push(vop(t, "LTE")); }
    if !created.is_empty() { f.insert("createdAt".into(), Value::Array(created)); }

    let mut range = |name: &str, lo: Option<u64>, hi: Option<u64>| {
        let mut r = Vec::new();
        if let Some(n) = lo { r.push(iop(n, "GTE")); }
        if let Some(n) = hi { r.push(iop(n, "LTE")); }
        if !r.is_empty() { f.insert(name.into(), Value::Array(r)); }
    };
    range("downloads", q.downloads_min, q.downloads_max);
    range("endorsements", q.endorsements_min, None);
    range("fileSize", q.size_min_kb, q.size_max_kb);

    if !q.include_adult { f.insert("adultContent".into(), json!([{ "value": false }])); }
    for fw in &q.frameworks { groups.push(framework_group(*fw)); }
    if let Some(t) = q.mod_type { groups.push(mod_type_group(t)); }
    if !groups.is_empty() { f.insert("filter".into(), Value::Array(groups)); }
    Value::Object(f)
}

pub fn build_sort(q: &BrowseQuery) -> Value {
    if q.sort == SortKey::Random {
        // verified live: `random: { seed }` sorts randomly and repeatably
        return json!([{ "random": { "seed": q.seed.unwrap_or(1) } }]);
    }
    let dir = if q.ascending { "ASC" } else { "DESC" };
    let key = match q.sort {
        SortKey::Random => unreachable!(),
        SortKey::Downloads => "downloads",
        SortKey::Endorsements => "endorsements",
        SortKey::Updated => "updatedAt",
        SortKey::Created => "createdAt",
        SortKey::Relevance => "relevance",
        SortKey::Size => "size",
        SortKey::Name => "name",
    };
    json!([{ key: { "direction": dir } }])
}

/// Variables for one page: filter, sort, paging, and the facets whose counts come back in the same response.
pub fn page_variables(q: &BrowseQuery) -> Value {
    json!({
        "filter": build_filter(q),
        "sort": build_sort(q),
        "facets": { "categoryName": [], "tag": [] },
        "offset": q.offset,
        "count": q.page_size(),
    })
}

/// One request, with an alias per option, that counts how many mods each derived filter would leave.
/// The API cannot facet on frameworks or mod types, so this is how those chips get their counts.
pub fn derived_counts_query(q: &BrowseQuery) -> (String, Value) {
    let mut base = q.clone();
    base.frameworks.clear();
    base.mod_type = None;
    let mut parts = Vec::new();
    let mut vars = Map::new();
    let mut decls = Vec::new();
    let mut add = |alias: String, query: BrowseQuery| {
        decls.push(format!("${alias}: ModsFilter"));
        parts.push(format!("{alias}: mods(filter: ${alias}, count: 0) {{ totalCount }}"));
        vars.insert(alias, build_filter(&query));
    };
    for info in frameworks::FRAMEWORKS {
        let mut x = base.clone();
        x.frameworks = q.frameworks.clone();
        if !x.frameworks.contains(&info.id) { x.frameworks.push(info.id); }
        x.mod_type = q.mod_type;
        add(format!("fw_{}", info.key), x);
    }
    for (key, t) in [("redmod", ModTypeFilter::Redmod), ("archive", ModTypeFilter::Archive), ("script", ModTypeFilter::Script), ("framework", ModTypeFilter::Framework)] {
        let mut x = base.clone();
        x.frameworks = q.frameworks.clone();
        x.mod_type = Some(t);
        add(format!("type_{key}"), x);
    }
    (format!("query Counts({}) {{ {} }}", decls.join(", "), parts.join(" ")), Value::Object(vars))
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Facets {
    /// value → count, from `facetsData`.
    pub categories: Vec<(String, u64)>,
    pub tags: Vec<(String, u64)>,
}

pub fn facets_from(data: &Option<Value>) -> Facets {
    let list = |k: &str| -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = data.as_ref().and_then(|d| d.get(k)).and_then(|o| o.as_object())
            .map(|o| o.iter().filter_map(|(name, n)| n.as_u64().map(|n| (name.clone(), n))).collect()).unwrap_or_default();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    };
    Facets { categories: list("categoryName"), tags: list("tag") }
}

#[derive(Deserialize)]
struct ModsData { mods: ModPage }

impl super::NexusClient {
    pub async fn mods_page(&self, q: &BrowseQuery) -> Result<ModPage> {
        let d: ModsData = self.graphql(&mods_page_query(), page_variables(q)).await?;
        Ok(d.mods)
    }

    pub async fn derived_counts(&self, q: &BrowseQuery) -> Result<Vec<(String, u64)>> {
        let (query, vars) = derived_counts_query(q);
        let (data, _) = self.graphql_raw(&query, vars).await?;
        Ok(data.as_object().map(|o| o.iter().filter_map(|(k, v)| v.get("totalCount").and_then(|n| n.as_u64()).map(|n| (k.clone(), n))).collect()).unwrap_or_default())
    }

    pub async fn mod_detail(&self, mod_id: u32) -> Result<(ModDetail, Vec<ModFile>)> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct D { #[serde(rename = "mod")] m: ModDetail, mod_files: Vec<ModFile> }
        let d: D = self.graphql(MOD_DETAIL_QUERY, json!({ "modId": mod_id.to_string(), "gameId": GAME_ID.to_string() })).await?;
        Ok((d.m, d.mod_files))
    }

    /// The archive file list Nexus has indexed for a mod (all its files, or one). Partial: may be empty.
    pub async fn file_contents(&self, mod_id: u32, file_id: Option<u32>, limit: u32) -> Result<(u64, Vec<FileContent>)> {
        let mut filter = json!({ "op": "AND", "gameId": [{ "value": GAME_ID, "op": "EQUALS" }], "modId": [{ "value": mod_id, "op": "EQUALS" }] });
        if let Some(fid) = file_id { filter["fileId"] = json!([{ "value": fid, "op": "EQUALS" }]); }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P { total_count: u64, nodes: Vec<FileContent> }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct D { mod_file_contents: P }
        let d: D = self.graphql(FILE_CONTENTS_QUERY, json!({ "filter": filter, "count": limit, "offset": 0 })).await?;
        Ok((d.mod_file_contents.total_count, d.mod_file_contents.nodes))
    }

    /// Current version and update time for many mods in one request.
    pub async fn mods_by_id(&self, ids: &[u32]) -> Result<Vec<Value>> {
        let list: Vec<Value> = ids.iter().map(|id| json!({ "gameDomain": crate::GAME_DOMAIN, "modId": id })).collect();
        let (data, _) = self.graphql_raw(MODS_BY_ID_QUERY, json!({ "ids": list, "count": ids.len() })).await?;
        Ok(data.pointer("/legacyModsByDomain/nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_filter_is_cyberpunk_without_adult() {
        let f = build_filter(&BrowseQuery::default());
        assert_eq!(f["gameDomainName"][0]["value"], "cyberpunk2077");
        assert_eq!(f["adultContent"][0]["value"], false);
        assert!(f.get("filter").is_none());
    }

    #[test]
    fn categories_are_ored_tags_anded() {
        let q = BrowseQuery { categories: vec!["Gameplay".into(), "Weapons".into()], tags: vec!["Gameplay".into(), "Weapons".into()], exclude_tags: vec!["Sexualised".into()], ..Default::default() };
        let f = build_filter(&q);
        assert_eq!(f["filter"][0]["op"], "OR");
        assert_eq!(f["filter"][0]["categoryName"].as_array().unwrap().len(), 2);
        assert_eq!(f["tag"].as_array().unwrap().len(), 3);
        assert_eq!(f["tag"][2]["op"], "NOT_EQUALS");
    }

    #[test]
    fn dates_are_unix_strings_and_compat_tightens_updated() {
        let q = BrowseQuery { updated_after: Some(100), compatible_since: Some(200), created_before: Some(300), ..Default::default() };
        let f = build_filter(&q);
        assert_eq!(f["updatedAt"][0], json!({ "value": "200", "op": "GTE" }));
        assert_eq!(f["createdAt"][0], json!({ "value": "300", "op": "LTE" }));
    }

    #[test]
    fn text_search_and_frameworks_become_or_groups() {
        let q = BrowseQuery { text: Some("  better vehicle ".into()), frameworks: vec![Framework::Cet, Framework::ArchiveXl], ..Default::default() };
        let f = build_filter(&q);
        let groups = f["filter"].as_array().unwrap();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0]["nameStemmed"][0]["value"], "better vehicle");
        assert_eq!(groups[1]["tag"][0]["value"], "Cyber Engine Tweaks");
        assert!(groups[2].get("tag").is_none());
        assert_eq!(groups[2]["description"][0]["value"], "ArchiveXL");
    }

    #[test]
    fn sort_and_paging() {
        let q = BrowseQuery { sort: SortKey::Updated, ascending: true, offset: 60, count: 500, ..Default::default() };
        let v = page_variables(&q);
        assert_eq!(v["sort"][0]["updatedAt"]["direction"], "ASC");
        assert_eq!(v["count"], 100);
        assert_eq!(v["offset"], 60);
    }

    #[test]
    fn random_sort_uses_the_seed() {
        let v = page_variables(&BrowseQuery { sort: SortKey::Random, seed: Some(7), ..Default::default() });
        assert_eq!(v["sort"][0]["random"]["seed"], 7);
    }

    #[test]
    fn cache_key_ignores_order_and_case() {
        let a = BrowseQuery { text: Some("CET ".into()), tags: vec!["b".into(), "a".into()], ..Default::default() };
        let b = BrowseQuery { text: Some("cet".into()), tags: vec!["a".into(), "b".into()], ..Default::default() };
        assert_eq!(a.cache_key(), b.cache_key());
        let c = BrowseQuery { offset: 30, ..b.clone() };
        assert_ne!(b.cache_key(), c.cache_key());
    }

    #[test]
    fn derived_counts_is_one_query_with_an_alias_per_option() {
        let (q, vars) = derived_counts_query(&BrowseQuery { frameworks: vec![Framework::Red4ext], ..Default::default() });
        assert!(q.starts_with("query Counts("));
        assert_eq!(vars.as_object().unwrap().len(), frameworks::FRAMEWORKS.len() + 4);
        // the already-applied framework stays in every count
        assert!(vars["fw_cet"]["filter"].as_array().unwrap().len() == 2);
    }

    #[test]
    fn facets_parse_and_sort() {
        let f = facets_from(&Some(json!({ "tag": { "A": 3, "B": 9 }, "categoryName": { "Gameplay": 4604 } })));
        assert_eq!(f.tags[0], ("B".into(), 9));
        assert_eq!(f.categories[0].1, 4604);
    }
}
