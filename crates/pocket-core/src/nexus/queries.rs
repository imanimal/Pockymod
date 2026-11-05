//! Request bodies and response parsers for the Nexus v1 REST and v2 GraphQL APIs.
//!
//! v1 shapes (users/validate, mods/{id}, files, download_link, updated, endorse, tracked_mods, games/{domain})
//! follow the published v1 docs.
//! UNVERIFIED: every v2 GraphQL name here (mods(), ModsFilter fields and operators, the sort keys, facets and
//! facetsData, legacyModsByDomain, modRequirements, collectionsV2, collectionRevision) comes from a schema
//! snapshot about 11 months old (spec section 4). The parsers are lenient: a missing field becomes a default
//! instead of an error, and `nexus::probe` logs the live shape so differences show up.

use super::types::*;
use crate::timefmt::parse_rfc3339;
use crate::{Error, Result, GAME_DOMAIN};
use serde_json::{json, Value};

/// UNVERIFIED unit: Mod.fileSize in v2 is taken to be kilobytes, like v1's size_kb.
pub const V2_FILE_SIZE_UNIT: u64 = 1024;

/// Only the fields a result card shows (engineering rule: one mods() call per page, card fields only).
pub const MODS_QUERY: &str = r#"query BrowseMods($filter: ModsFilter, $sort: [ModsSort!], $offset: Int, $count: Int, $facets: ModsFacet) {
  mods(filter: $filter, sort: $sort, offset: $offset, count: $count, facets: $facets) {
    totalCount
    facetsData
    nodes {
      modId name summary author
      uploader { name }
      thumbnailUrl pictureUrl
      downloads endorsements
      updatedAt createdAt
      fileSize category adultContent version
    }
  }
}"#;

fn op(field: &str, value: Value, op: &str) -> Value {
    json!({ field: [{ "value": value, "op": op }] })
}

/// The variables for MODS_QUERY. Only the remote part of the query is used.
pub fn mods_variables(q: &BrowseQuery) -> Value {
    let mut parts = vec![op("gameDomainName", json!(GAME_DOMAIN), "EQUALS")];
    if let Some(t) = q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        // search name and description together
        parts.push(json!({ "op": "OR", "filter": [
            op("nameStemmed", json!(t), "MATCHES"),
            op("description", json!(t), "MATCHES"),
        ]}));
    }
    if !q.categories.is_empty() {
        parts.push(json!({ "op": "OR", "filter": q.categories.iter().map(|c| op("categoryName", json!(c), "EQUALS")).collect::<Vec<_>>() }));
    }
    for t in &q.tags {
        parts.push(op("tag", json!(t), "EQUALS"));
    }
    if let Some(a) = q.author.as_deref().filter(|a| !a.is_empty()) {
        parts.push(op("author", json!(a), "EQUALS"));
    }
    let iso = |t: i64| json!(format!("{}T00:00:00Z", crate::timefmt::ymd(t)));
    if let Some(t) = q.updated_after { parts.push(op("updatedAt", iso(t), "GTE")); }
    if let Some(t) = q.updated_before { parts.push(op("updatedAt", iso(t), "LTE")); }
    if let Some(t) = q.created_after { parts.push(op("createdAt", iso(t), "GTE")); }
    if let Some(t) = q.created_before { parts.push(op("createdAt", iso(t), "LTE")); }
    if let Some(n) = q.min_downloads { parts.push(op("downloads", json!(n), "GTE")); }
    if let Some(n) = q.max_downloads { parts.push(op("downloads", json!(n), "LTE")); }
    if let Some(n) = q.min_endorsements { parts.push(op("endorsements", json!(n), "GTE")); }
    if let Some(n) = q.max_endorsements { parts.push(op("endorsements", json!(n), "LTE")); }
    if let Some(n) = q.min_size_kb { parts.push(op("fileSize", json!(n), "GTE")); }
    if let Some(n) = q.max_size_kb { parts.push(op("fileSize", json!(n), "LTE")); }
    if !q.include_adult {
        parts.push(op("adultContent", json!(false), "EQUALS"));
    }
    let dir = if q.ascending { "ASC" } else { "DESC" };
    let key = match q.sort {
        SortKey::Relevance if q.text.as_deref().is_some_and(|t| !t.trim().is_empty()) => "relevance",
        SortKey::Relevance => "endorsements", // relevance needs a search term
        SortKey::Downloads => "downloads",
        SortKey::Endorsements => "endorsements",
        SortKey::Updated => "updatedAt",
        SortKey::Created => "createdAt",
        SortKey::Size => "fileSize",
        SortKey::Name => "name",
    };
    json!({
        "filter": { "op": "AND", "filter": parts },
        "sort": [{ key: { "direction": dir } }],
        "offset": q.offset,
        "count": if q.count == 0 { 20 } else { q.count.min(50) },
        "facets": { "categoryName": [], "tag": [] },
    })
}

fn u64_of(v: &Value) -> u64 {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).or_else(|| v.as_f64().map(|f| f as u64)).unwrap_or(0)
}
fn time_of(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_str().and_then(parse_rfc3339)).unwrap_or(0)
}
fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(String::from).filter(|s| !s.is_empty())
}

pub fn card_from_v2(n: &Value) -> Option<ModCard> {
    let mod_id = u64_of(&n["modId"]);
    if mod_id == 0 {
        return None;
    }
    let size = n["fileSize"].as_u64().or_else(|| n["fileSize"].as_str().and_then(|s| s.parse().ok()));
    Some(ModCard {
        mod_id,
        name: str_of(&n["name"]).unwrap_or_else(|| format!("Mod {mod_id}")),
        summary: str_of(&n["summary"]).unwrap_or_default(),
        author: str_of(&n["author"]).or_else(|| str_of(&n["uploader"]["name"])).unwrap_or_default(),
        thumbnail_url: str_of(&n["thumbnailUrl"]),
        picture_url: str_of(&n["pictureUrl"]),
        downloads: u64_of(&n["downloads"]),
        endorsements: u64_of(&n["endorsements"]),
        updated_at: time_of(&n["updatedAt"]),
        created_at: time_of(&n["createdAt"]),
        size_bytes: size.map(|s| s * V2_FILE_SIZE_UNIT),
        category: str_of(&n["category"]).or_else(|| str_of(&n["category"]["name"])),
        adult: n["adultContent"].as_bool().unwrap_or(false),
        version: str_of(&n["version"]),
        derived: Default::default(),
        compat: crate::classify::Compat::Unknown,
        hidden: false,
        downloaded: false,
        installed: false,
    })
}

/// facetsData in either of the shapes it might come in:
/// `{"categoryName": {"Gameplay": 3}}` or `[{"facet":"categoryName","value":"Gameplay","count":3}]`
/// or `{"categoryName": [{"value":"Gameplay","count":3}]}`.
pub fn parse_facets(v: &Value) -> (Vec<FacetCount>, Vec<FacetCount>) {
    let mut cats = Vec::new();
    let mut tags = Vec::new();
    let mut add = |facet: &str, value: String, count: u64| {
        let fc = FacetCount { value, count };
        match facet {
            "categoryName" | "category" => cats.push(fc),
            "tag" | "tags" => tags.push(fc),
            _ => {}
        }
    };
    let v = if let Some(s) = v.as_str() { serde_json::from_str(s).unwrap_or(Value::Null) } else { v.clone() };
    match &v {
        Value::Object(map) => {
            for (facet, inner) in map {
                match inner {
                    Value::Object(vals) => vals.iter().for_each(|(k, c)| add(facet, k.clone(), u64_of(c))),
                    Value::Array(items) => items.iter().for_each(|i| {
                        if let Some(val) = str_of(&i["value"]).or_else(|| str_of(&i["name"])) {
                            add(facet, val, u64_of(&i["count"]))
                        }
                    }),
                    _ => {}
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| {
            if let (Some(f), Some(val)) = (str_of(&i["facet"]), str_of(&i["value"])) {
                add(&f, val, u64_of(&i["count"]))
            }
        }),
        _ => {}
    }
    cats.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
    tags.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
    (cats, tags)
}

/// Unwraps a GraphQL response: errors become Error::BadResponse, data comes back.
pub fn graphql_data(body: &Value) -> Result<&Value> {
    if let Some(errs) = body["errors"].as_array().filter(|e| !e.is_empty()) {
        let msg = errs.iter().filter_map(|e| e["message"].as_str()).collect::<Vec<_>>().join("; ");
        if body["data"].is_null() {
            return Err(Error::BadResponse { what: "Nexus GraphQL".into(), detail: msg });
        }
        log::warn!("Nexus GraphQL returned partial data with errors: {msg}");
    }
    Ok(&body["data"])
}

pub fn parse_mods_page(body: &Value, offset: u32) -> Result<ModPage> {
    let data = graphql_data(body)?;
    let mods = &data["mods"];
    let nodes = mods["nodes"].as_array().ok_or_else(|| Error::BadResponse { what: "mods()".into(), detail: "no nodes".into() })?;
    let (categories, tags) = parse_facets(&mods["facetsData"]);
    Ok(ModPage {
        mods: nodes.iter().filter_map(card_from_v2).collect(),
        total: u64_of(&mods["totalCount"]),
        offset,
        facets: Facets { categories, tags, ..Default::default() },
        cached: false,
        filtered_out: 0,
    })
}

/// v1 GET /v1/games/{game}/mods/{id}.json → the card part of a detail view.
pub fn card_from_v1(v: &Value) -> Option<ModCard> {
    let mod_id = u64_of(&v["mod_id"]);
    if mod_id == 0 {
        return None;
    }
    Some(ModCard {
        mod_id,
        name: str_of(&v["name"]).unwrap_or_else(|| format!("Mod {mod_id}")),
        summary: str_of(&v["summary"]).unwrap_or_default(),
        author: str_of(&v["author"]).or_else(|| str_of(&v["uploaded_by"])).unwrap_or_default(),
        thumbnail_url: str_of(&v["picture_url"]),
        picture_url: str_of(&v["picture_url"]),
        downloads: u64_of(&v["mod_downloads"]),
        endorsements: u64_of(&v["endorsement_count"]),
        updated_at: v["updated_timestamp"].as_i64().unwrap_or_else(|| time_of(&v["updated_time"])),
        created_at: v["created_timestamp"].as_i64().unwrap_or_else(|| time_of(&v["created_time"])),
        size_bytes: None,
        category: v["category_id"].as_u64().map(|c| c.to_string()),
        adult: v["contains_adult_content"].as_bool().unwrap_or(false),
        version: str_of(&v["version"]),
        derived: Default::default(),
        compat: crate::classify::Compat::Unknown,
        hidden: false,
        downloaded: false,
        installed: false,
    })
}

/// v1 GET /v1/games/{game}/mods/{id}/files.json
pub fn parse_files(v: &Value) -> Vec<ModFile> {
    let mut files: Vec<ModFile> = v["files"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let file_id = u64_of(&f["file_id"]);
                    (file_id != 0).then(|| ModFile {
                        file_id,
                        name: str_of(&f["name"]).unwrap_or_default(),
                        version: str_of(&f["version"]).or_else(|| str_of(&f["mod_version"])),
                        category: str_of(&f["category_name"]).unwrap_or_else(|| "MAIN".into()),
                        size_bytes: f["size_in_bytes"].as_u64().unwrap_or_else(|| u64_of(&f["size_kb"]) * 1024),
                        file_name: str_of(&f["file_name"]).unwrap_or_default(),
                        uploaded_at: f["uploaded_timestamp"].as_i64().unwrap_or(0),
                        is_primary: f["is_primary"].as_bool().unwrap_or(false),
                        description: str_of(&f["description"]),
                        content_preview_url: str_of(&f["content_preview_link"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|f| std::cmp::Reverse(f.uploaded_at));
    files
}

/// The file an Install key picks: the primary MAIN file, else the newest MAIN file, else the newest non-archived one.
pub fn recommended_file(files: &[ModFile]) -> Option<u64> {
    let main = |f: &&ModFile| f.category.eq_ignore_ascii_case("MAIN");
    files
        .iter()
        .filter(main)
        .find(|f| f.is_primary)
        .or_else(|| files.iter().filter(main).max_by_key(|f| f.uploaded_at))
        .or_else(|| {
            files.iter().filter(|f| !["OLD_VERSION", "ARCHIVED"].contains(&f.category.to_ascii_uppercase().as_str())).max_by_key(|f| f.uploaded_at)
        })
        .map(|f| f.file_id)
}

/// Nexus content preview JSON (a tree of {name, path, type, children}) → file paths.
pub fn parse_content_preview(v: &Value) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        if let Some(children) = v["children"].as_array() {
            for c in children {
                walk(c, out);
            }
        }
        let is_file = v["type"].as_str().map(|t| t.eq_ignore_ascii_case("file")).unwrap_or(false);
        if is_file {
            if let Some(p) = str_of(&v["path"]).or_else(|| str_of(&v["name"])) {
                out.push(p.replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// UNVERIFIED: requirement links from v2.
pub const REQUIREMENTS_QUERY: &str = r#"query Requirements($ids: [CompositeDomainWithIdInput!]!) {
  legacyModsByDomain(ids: $ids) {
    nodes {
      modId
      modRequirements {
        nexusRequirements { nodes { modId name url notes externalRequirement } }
      }
    }
  }
}"#;

pub fn requirements_variables(mod_id: u64) -> Value {
    json!({ "ids": [{ "gameDomain": GAME_DOMAIN, "modId": mod_id }] })
}

pub fn parse_requirements(body: &Value) -> Result<Vec<Requirement>> {
    let data = graphql_data(body)?;
    let nodes = data["legacyModsByDomain"]["nodes"].as_array().cloned().unwrap_or_default();
    Ok(nodes
        .iter()
        .flat_map(|n| n["modRequirements"]["nexusRequirements"]["nodes"].as_array().cloned().unwrap_or_default())
        .map(|r| {
            let id = u64_of(&r["modId"]);
            Requirement {
                mod_id: (id != 0).then_some(id),
                name: str_of(&r["name"]).unwrap_or_else(|| "Unnamed requirement".into()),
                url: str_of(&r["url"]),
                notes: str_of(&r["notes"]),
                external: r["externalRequirement"].as_bool().unwrap_or(false),
            }
        })
        .collect())
}

pub fn parse_account(v: &Value) -> Result<Account> {
    let user_id = u64_of(&v["user_id"]);
    if user_id == 0 {
        return Err(Error::BadResponse { what: "users/validate".into(), detail: "no user_id".into() });
    }
    // the response also echoes the key; it is deliberately not read
    Ok(Account {
        user_id,
        name: str_of(&v["name"]).unwrap_or_default(),
        is_premium: v["is_premium"].as_bool().or_else(|| v["is_premium?"].as_bool()).unwrap_or(false),
        is_supporter: v["is_supporter"].as_bool().or_else(|| v["is_supporter?"].as_bool()).unwrap_or(false),
        profile_url: str_of(&v["profile_url"]),
    })
}

/// v1 download_link.json → URLs, preferred CDN first.
pub fn parse_download_links(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|l| str_of(&l["URI"])).collect())
        .unwrap_or_default()
}

pub fn parse_updated(v: &Value) -> Vec<UpdatedMod> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|u| UpdatedMod {
                    mod_id: u64_of(&u["mod_id"]),
                    latest_file_update: u["latest_file_update"].as_i64().unwrap_or(0),
                    latest_mod_activity: u["latest_mod_activity"].as_i64().unwrap_or(0),
                })
                .filter(|u| u.mod_id != 0)
                .collect()
        })
        .unwrap_or_default()
}

/// v1 games/{domain}.json → categories.
pub fn parse_categories(v: &Value) -> Vec<Category> {
    v["categories"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let id = u64_of(&c["category_id"]);
                    (id != 0).then(|| Category {
                        id,
                        name: str_of(&c["name"]).unwrap_or_default(),
                        parent: c["parent_category"].as_u64().filter(|p| *p != 0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The detail view's "must download by hand" flag: v1 has no such field, so this reads the cues Nexus pages use.
/// UNVERIFIED: heuristic. Mods without a MAIN or OPTIONAL file, or whose description asks for a manual install.
pub fn looks_manual_only(files: &[ModFile], description: &str) -> bool {
    let has_installable = files.iter().any(|f| ["MAIN", "OPTIONAL", "UPDATE"].contains(&f.category.to_ascii_uppercase().as_str()));
    let d = description.to_ascii_lowercase();
    !has_installable || d.contains("manual download only") || d.contains("do not use a mod manager") || d.contains("don't use a mod manager")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_build_filters_and_sort() {
        let q = BrowseQuery { text: Some("jacket".into()), categories: vec!["Clothing".into()], sort: SortKey::Downloads, min_downloads: Some(100), count: 20, ..Default::default() };
        let v = mods_variables(&q);
        assert_eq!(v["filter"]["op"], "AND");
        let parts = v["filter"]["filter"].as_array().unwrap();
        assert_eq!(parts[0]["gameDomainName"][0]["value"], "cyberpunk2077");
        assert_eq!(parts[1]["op"], "OR");
        assert!(parts.iter().any(|p| p["downloads"][0]["op"] == "GTE"));
        assert!(parts.iter().any(|p| p["adultContent"][0]["value"] == false));
        assert_eq!(v["sort"][0]["downloads"]["direction"], "DESC");
        assert_eq!(v["count"], 20);
        // relevance without text falls back
        let v = mods_variables(&BrowseQuery::default());
        assert!(v["sort"][0].get("endorsements").is_some());
        assert_eq!(v["count"], 20);
    }

    #[test]
    fn remote_key_ignores_local_filters() {
        let a = BrowseQuery { text: Some("x".into()), ..Default::default() };
        let mut b = a.clone();
        b.compatible_only = true;
        b.frameworks.push(crate::frameworks::Framework::Cet);
        assert_eq!(a.remote_key(), b.remote_key());
        b.offset = 20;
        assert_ne!(a.remote_key(), b.remote_key());
    }

    #[test]
    fn parses_a_mods_page() {
        let body = json!({"data": {"mods": {
            "totalCount": 2,
            "facetsData": {"categoryName": {"Gameplay": 5, "Clothing": 9}},
            "nodes": [
                {"modId": 1, "name": "A", "summary": "s", "author": "me", "downloads": 10, "endorsements": "3",
                 "updatedAt": "2024-05-01T12:30:00Z", "createdAt": "2024-01-01T00:00:00Z", "fileSize": 2, "category": "Gameplay", "adultContent": false},
                {"modId": 0, "name": "broken"}
            ]
        }}});
        let p = parse_mods_page(&body, 0).unwrap();
        assert_eq!(p.total, 2);
        assert_eq!(p.mods.len(), 1);
        assert_eq!(p.mods[0].endorsements, 3);
        assert_eq!(p.mods[0].updated_at, 1714566600);
        assert_eq!(p.mods[0].size_bytes, Some(2048));
        assert_eq!(p.facets.categories[0], FacetCount { value: "Clothing".into(), count: 9 });
    }

    #[test]
    fn facets_in_other_shapes() {
        let (c, _) = parse_facets(&json!([{"facet": "categoryName", "value": "X", "count": 2}]));
        assert_eq!(c[0].value, "X");
        let (c, t) = parse_facets(&json!({"categoryName": [{"value": "Y", "count": 1}], "tag": {"Lore": 4}}));
        assert_eq!(c[0].value, "Y");
        assert_eq!(t[0].count, 4);
        let (c, _) = parse_facets(&json!("{\"categoryName\":{\"Z\":7}}"));
        assert_eq!(c[0].count, 7);
        assert!(parse_facets(&Value::Null).0.is_empty());
    }

    #[test]
    fn graphql_errors_surface() {
        let e = parse_mods_page(&json!({"errors": [{"message": "Field 'x' doesn't exist"}], "data": null}), 0).unwrap_err();
        assert!(e.to_string().contains("doesn't exist"));
    }

    #[test]
    fn files_and_recommendation() {
        let v = json!({"files": [
            {"file_id": 1, "name": "Old", "category_name": "OLD_VERSION", "size_kb": 10, "uploaded_timestamp": 100},
            {"file_id": 2, "name": "Main", "category_name": "MAIN", "size_in_bytes": 5000, "uploaded_timestamp": 200},
            {"file_id": 3, "name": "Main newer", "category_name": "MAIN", "size_in_bytes": 6000, "uploaded_timestamp": 300},
            {"file_id": 4, "name": "Optional", "category_name": "OPTIONAL", "uploaded_timestamp": 400}
        ]});
        let f = parse_files(&v);
        assert_eq!(f[0].file_id, 4, "newest first");
        assert_eq!(f.iter().find(|x| x.file_id == 1).unwrap().size_bytes, 10240);
        assert_eq!(recommended_file(&f), Some(3));
        let mut f2 = f.clone();
        f2.iter_mut().find(|x| x.file_id == 2).unwrap().is_primary = true;
        assert_eq!(recommended_file(&f2), Some(2));
        assert!(!looks_manual_only(&f, ""));
        assert!(looks_manual_only(&f[2..3].iter().filter(|x| x.category == "OLD_VERSION").cloned().collect::<Vec<_>>(), ""));
    }

    #[test]
    fn content_preview_tree() {
        let v = json!({"children": [{"type": "directory", "name": "archive", "path": "archive", "children": [
            {"type": "file", "name": "a.archive", "path": "archive\\pc\\mod\\a.archive"}]}]});
        assert_eq!(parse_content_preview(&v), vec!["archive/pc/mod/a.archive"]);
    }

    #[test]
    fn account_ignores_the_key() {
        let a = parse_account(&json!({"user_id": 5, "key": "SECRET", "name": "V", "is_premium": true, "is_supporter": false})).unwrap();
        assert!(a.is_premium);
        assert!(!serde_json::to_string(&a).unwrap().contains("SECRET"));
        assert!(parse_account(&json!({})).is_err());
    }

    #[test]
    fn small_v1_parsers() {
        assert_eq!(parse_download_links(&json!([{"name": "CDN", "URI": "https://x/y.zip"}])), vec!["https://x/y.zip"]);
        let u = parse_updated(&json!([{"mod_id": 7, "latest_file_update": 10, "latest_mod_activity": 11}]));
        assert_eq!(u[0].mod_id, 7);
        let c = parse_categories(&json!({"categories": [{"category_id": 2, "name": "Gameplay", "parent_category": false}]}));
        assert_eq!(c[0].name, "Gameplay");
        assert_eq!(c[0].parent, None);
        let r = parse_requirements(&json!({"data": {"legacyModsByDomain": {"nodes": [{"modRequirements": {"nexusRequirements": {"nodes": [{"modId": "107", "name": "CET"}]}}}]}}})).unwrap();
        assert_eq!(r[0].mod_id, Some(107));
    }
}
