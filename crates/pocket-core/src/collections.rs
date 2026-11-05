//! Collections: Nexus collections (browse and install) and local ones (create and export as a shareable file).
//!
//! A collection is a manifest, never the mod files: mod IDs, file choices, hashes and rules. The manifest shape
//! follows Vortex's open-source ICollection (collection.json); our exports use the same shape so they can be
//! read back, plus a `pocketMods` block with the load order.
//! UNVERIFIED: the GraphQL names (collectionsV2, collectionRevision, downloadLink, modFiles) and that the
//! revision download is a .7z holding collection.json. Creating or publishing on Nexus from a third-party app is
//! unconfirmed; it sits behind an experiment flag and is not implemented against the API.

use crate::library::InstalledMod;
use crate::queue::NewItem;
use crate::{Error, Result, GAME_DOMAIN};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionInfo {
    pub author: String,
    pub author_url: String,
    pub name: String,
    pub description: String,
    pub install_instructions: Option<String>,
    pub domain_name: String,
    pub game_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceInfo {
    /// "nexus", "direct", "browse", "manual" or "bundle"
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub instructions: Option<String>,
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    /// "exact", "latest" or "prefer"
    pub update_policy: Option<String>,
    pub md5: Option<String>,
    pub file_size: Option<u64>,
    pub logical_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionMod {
    pub name: String,
    pub version: String,
    pub optional: bool,
    pub domain_name: String,
    pub source: SourceInfo,
    pub author: Option<String>,
    pub instructions: Option<String>,
    pub phase: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ModReference {
    pub file_md5: Option<String>,
    pub logical_file_name: Option<String>,
    pub file_expression: Option<String>,
    pub id_hint: Option<String>,
    pub repo: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ModRule {
    pub source: ModReference,
    /// "before", "after", "requires", "conflicts", "recommends", "provides"
    #[serde(rename = "type")]
    pub kind: String,
    pub reference: ModReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Collection {
    pub info: CollectionInfo,
    pub mods: Vec<CollectionMod>,
    pub mod_rules: Vec<ModRule>,
    /// Our extension: REDmod load order and the tool that wrote it. Ignored by other readers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pocket_mods: Option<Value>,
}

pub fn parse_manifest(text: &str) -> Result<Collection> {
    let c: Collection = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| Error::Invalid(format!("That isn't a collection file: {e}")))?;
    if c.mods.is_empty() {
        return Err(Error::Invalid("The collection lists no mods.".into()));
    }
    if !c.info.domain_name.is_empty() && !c.info.domain_name.eq_ignore_ascii_case(GAME_DOMAIN) {
        return Err(Error::Invalid(format!("This collection is for {}, not Cyberpunk 2077.", c.info.domain_name)));
    }
    Ok(c)
}

/// Extracts an archive (a Nexus revision download) and reads the collection.json inside.
pub fn read_manifest_from_archive(archive: &Path, scratch: &Path) -> Result<Collection> {
    let files = crate::archive::extract(archive, scratch)?;
    let found = files
        .iter()
        .find(|f| f.eq_ignore_ascii_case("collection.json") || f.to_ascii_lowercase().ends_with("/collection.json"))
        .ok_or_else(|| Error::Invalid("The collection download holds no collection.json.".into()))?;
    let text = std::fs::read_to_string(crate::frameworks::game_path(scratch, found))?;
    parse_manifest(&text)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingMod {
    pub name: String,
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    pub optional: bool,
    /// Not on Nexus: a direct link, a web page, or a manual step.
    pub external: bool,
    pub url: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreCheck {
    pub name: String,
    pub total: usize,
    pub optional: usize,
    pub installed: usize,
    pub missing: Vec<MissingMod>,
    /// Nexus downloads that will need a click (free account) or that are manual anyway.
    pub manual_clicks: usize,
    pub external: usize,
    pub bytes_to_download: u64,
    /// None when the collection names no game version.
    pub game_version_ok: Option<bool>,
    pub game_versions: Vec<String>,
}

fn is_installed(m: &CollectionMod, installed: &[InstalledMod]) -> bool {
    installed.iter().any(|i| {
        (m.source.mod_id.is_some() && i.mod_id == m.source.mod_id && (m.source.file_id.is_none() || i.file_id == m.source.file_id || m.source.update_policy.as_deref() == Some("latest")))
            || (m.source.md5.is_some() && i.md5.is_some() && i.md5.as_deref().map(str::to_ascii_lowercase) == m.source.md5.as_deref().map(str::to_ascii_lowercase))
    })
}

/// Pre-install checks: game version, what's missing, how many clicks it will cost.
pub fn pre_check(c: &Collection, installed: &[InstalledMod], premium: bool, game_version: Option<&str>, include_optional: bool) -> PreCheck {
    let wanted: Vec<&CollectionMod> = c.mods.iter().filter(|m| include_optional || !m.optional).collect();
    let missing: Vec<MissingMod> = wanted
        .iter()
        .filter(|m| !is_installed(m, installed))
        .map(|m| MissingMod {
            name: m.name.clone(),
            mod_id: m.source.mod_id,
            file_id: m.source.file_id,
            optional: m.optional,
            external: m.source.kind != "nexus",
            url: m.source.url.clone(),
            size_bytes: m.source.file_size.unwrap_or(0),
        })
        .collect();
    let external = missing.iter().filter(|m| m.external).count();
    let nexus_missing = missing.len() - external;
    let game_version_ok = match (c.info.game_versions.is_empty(), game_version) {
        (true, _) | (_, None) => None,
        (false, Some(v)) => Some(c.info.game_versions.iter().any(|g| versions_match(g, v))),
    };
    PreCheck {
        name: c.info.name.clone(),
        total: c.mods.len(),
        optional: c.mods.iter().filter(|m| m.optional).count(),
        installed: wanted.len() - missing.len(),
        manual_clicks: if premium { external } else { nexus_missing + external },
        external,
        bytes_to_download: missing.iter().map(|m| m.size_bytes).sum(),
        missing,
        game_version_ok,
        game_versions: c.info.game_versions.clone(),
    }
}

/// "2.21" matches "2.21", "2.21.1" and "v2.21"; exe-style versions match on their leading parts.
pub fn versions_match(wanted: &str, have: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches(['v', 'V']).to_string();
    let (w, h) = (norm(wanted), norm(have));
    w == h || h.starts_with(&format!("{w}.")) || w.starts_with(&format!("{h}."))
}

/// Queue items for the missing Nexus mods (external ones are listed for the person to handle).
pub fn queue_items(c: &Collection, check: &PreCheck, slug: &str) -> Vec<NewItem> {
    check
        .missing
        .iter()
        .filter(|m| !m.external)
        .filter_map(|m| {
            Some(NewItem {
                mod_id: m.mod_id?,
                file_id: m.file_id?,
                mod_name: m.name.clone(),
                file_name: c.mods.iter().find(|cm| cm.source.mod_id == m.mod_id).and_then(|cm| cm.source.logical_filename.clone()).unwrap_or_else(|| m.name.clone()),
                version: c.mods.iter().find(|cm| cm.source.mod_id == m.mod_id).map(|cm| cm.version.clone()).filter(|v| !v.is_empty()),
                size_bytes: m.size_bytes,
                manual_only: false,
                origin: format!("collection:{slug}"),
            })
        })
        .collect()
}

/// Builds a shareable collection from installed mods: pins mod ID, file ID and MD5, and records the
/// dependency graph (requires rules) and the REDmod load order.
pub fn export(name: &str, author: &str, description: &str, mods: &[InstalledMod], load_order: &[String], game_version: Option<&str>) -> Result<Collection> {
    if mods.is_empty() {
        return Err(Error::Invalid("Pick at least one mod for the collection.".into()));
    }
    let reference = |m: &InstalledMod| ModReference {
        file_md5: m.md5.clone(),
        logical_file_name: Some(m.file_name.clone()),
        id_hint: Some(m.key.clone()),
        repo: m.mod_id.map(|id| json!({ "repository": "nexus", "gameId": GAME_DOMAIN, "modId": id.to_string(), "fileId": m.file_id.map(|f| f.to_string()) })),
        ..Default::default()
    };
    let in_set: HashSet<u64> = mods.iter().filter_map(|m| m.mod_id).collect();
    let mut rules = Vec::new();
    for m in mods {
        for req in &m.requirements {
            if let Some(dep) = mods.iter().find(|d| d.mod_id == Some(*req)) {
                rules.push(ModRule { source: reference(m), kind: "requires".into(), reference: reference(dep) });
            }
        }
        // frameworks the mod needs that are in the set count as requirements too
        for fw in &m.frameworks {
            let fid = crate::frameworks::info(*fw).nexus_mod_id;
            if in_set.contains(&fid) && !m.requirements.contains(&fid) && m.mod_id != Some(fid) {
                if let Some(dep) = mods.iter().find(|d| d.mod_id == Some(fid)) {
                    rules.push(ModRule { source: reference(m), kind: "requires".into(), reference: reference(dep) });
                }
            }
        }
    }
    let out_mods = mods
        .iter()
        .map(|m| CollectionMod {
            name: m.name.clone(),
            version: m.version.clone().unwrap_or_default(),
            optional: false,
            domain_name: GAME_DOMAIN.into(),
            source: SourceInfo {
                kind: if m.mod_id.is_some() { "nexus".into() } else { "manual".into() },
                mod_id: m.mod_id,
                file_id: m.file_id,
                update_policy: Some("exact".into()),
                md5: m.md5.clone(),
                file_size: Some(m.size),
                logical_filename: Some(m.file_name.clone()),
                instructions: if m.mod_id.is_none() { Some("Not from Nexus: get this file from wherever the author shared it.".into()) } else { None },
                ..Default::default()
            },
            author: m.author.clone(),
            ..Default::default()
        })
        .collect();
    let redmods: HashSet<&str> = mods.iter().flat_map(|m| m.redmods.iter().map(String::as_str)).collect();
    Ok(Collection {
        info: CollectionInfo {
            author: author.into(),
            name: name.trim().into(),
            description: description.into(),
            domain_name: GAME_DOMAIN.into(),
            game_versions: game_version.map(|v| vec![v.to_string()]).unwrap_or_default(),
            ..Default::default()
        },
        mods: out_mods,
        mod_rules: rules,
        pocket_mods: Some(json!({
            "format": 1,
            "loadOrder": load_order.iter().filter(|f| redmods.contains(f.as_str())).collect::<Vec<_>>(),
        })),
    })
}

// Nexus collections over GraphQL (UNVERIFIED names) ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCard {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
    pub endorsements: u64,
    pub downloads: u64,
    pub revision: Option<u64>,
    pub mod_count: u64,
    pub size_bytes: u64,
    pub adult: bool,
    pub game_versions: Vec<String>,
    pub category: Option<String>,
}

pub const COLLECTIONS_QUERY: &str = r#"query BrowseCollections($filter: CollectionsSearchFilter, $sort: [CollectionsSearchSort!], $offset: Int, $count: Int) {
  collectionsV2(filter: $filter, sort: $sort, offset: $offset, count: $count) {
    totalCount
    nodes {
      slug name summary endorsements totalDownloads
      user { name }
      tileImage { thumbnailUrl }
      category { name }
      latestPublishedRevision { revisionNumber modCount totalSize adultContent gameVersions { reference } }
    }
  }
}"#;

pub fn collections_variables(text: Option<&str>, game_version: Option<&str>, sort: &str, offset: u32, count: u32) -> Value {
    let mut filter = json!({ "gameDomain": [{ "value": GAME_DOMAIN, "op": "EQUALS" }] });
    if let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) {
        filter["name"] = json!([{ "value": t, "op": "WILDCARD" }]);
    }
    if let Some(v) = game_version.filter(|v| !v.is_empty()) {
        filter["gameVersion"] = json!([{ "value": v, "op": "EQUALS" }]);
    }
    let key = match sort {
        "downloads" => "downloads",
        "updated" => "updatedAt",
        "created" => "createdAt",
        _ => "endorsements",
    };
    json!({ "filter": filter, "sort": [{ key: { "direction": "DESC" } }], "offset": offset, "count": count.clamp(1, 50) })
}

pub fn parse_collections(body: &Value) -> Result<(Vec<CollectionCard>, u64)> {
    let data = crate::nexus::queries::graphql_data(body)?;
    let c = &data["collectionsV2"];
    let s = |v: &Value| v.as_str().map(String::from);
    let n = |v: &Value| v.as_u64().or_else(|| v.as_str().and_then(|x| x.parse().ok())).unwrap_or(0);
    let cards = c["nodes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let rev = &x["latestPublishedRevision"];
                    Some(CollectionCard {
                        slug: s(&x["slug"])?,
                        name: s(&x["name"]).unwrap_or_default(),
                        summary: s(&x["summary"]).unwrap_or_default(),
                        author: s(&x["user"]["name"]).unwrap_or_default(),
                        thumbnail_url: s(&x["tileImage"]["thumbnailUrl"]),
                        endorsements: n(&x["endorsements"]),
                        downloads: n(&x["totalDownloads"]),
                        revision: rev["revisionNumber"].as_u64(),
                        mod_count: n(&rev["modCount"]),
                        size_bytes: n(&rev["totalSize"]),
                        adult: rev["adultContent"].as_bool().unwrap_or(false),
                        game_versions: rev["gameVersions"].as_array().map(|g| g.iter().filter_map(|v| s(&v["reference"])).collect()).unwrap_or_default(),
                        category: s(&x["category"]["name"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((cards, n(&c["totalCount"])))
}

pub const REVISION_QUERY: &str = r#"query Revision($slug: String!, $revision: Int, $domain: String) {
  collectionRevision(slug: $slug, revision: $revision, domainName: $domain, viewAdultContent: true) {
    revisionNumber
    downloadLink
  }
}"#;

pub fn revision_variables(slug: &str, revision: Option<u64>) -> Value {
    json!({ "slug": slug, "revision": revision, "domain": GAME_DOMAIN })
}

/// The revision's download path, e.g. "/v1/collections/<slug>/revisions/<n>/download_link".
pub fn parse_revision_link(body: &Value) -> Result<(Option<u64>, String)> {
    let data = crate::nexus::queries::graphql_data(body)?;
    let r = &data["collectionRevision"];
    let link = r["downloadLink"].as_str().ok_or_else(|| Error::BadResponse { what: "collectionRevision".into(), detail: "no downloadLink".into() })?;
    Ok((r["revisionNumber"].as_u64(), link.to_string()))
}

/// The download_link response: `{"download_links": [{"URI": ...}]}`, or a bare list like mod files.
pub fn parse_collection_download(v: &Value) -> Vec<String> {
    let list = if v["download_links"].is_array() { &v["download_links"] } else { v };
    crate::nexus::queries::parse_download_links(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::sample_mod;

    const MANIFEST: &str = r#"{
      "info": {"author": "A", "authorUrl": "", "name": "Night City Plus", "description": "d", "domainName": "cyberpunk2077", "gameVersions": ["2.21"]},
      "mods": [
        {"name": "CET", "version": "1.34", "optional": false, "domainName": "cyberpunk2077",
         "source": {"type": "nexus", "modId": 107, "fileId": 1000, "md5": "AA", "fileSize": 100, "updatePolicy": "exact", "logicalFilename": "CET"}},
        {"name": "Jacket", "version": "1", "optional": false, "domainName": "cyberpunk2077",
         "source": {"type": "nexus", "modId": 5, "fileId": 50, "fileSize": 300}},
        {"name": "Extra", "version": "", "optional": true, "domainName": "cyberpunk2077", "source": {"type": "nexus", "modId": 6, "fileId": 60}},
        {"name": "Off-site", "version": "", "optional": false, "domainName": "cyberpunk2077", "source": {"type": "browse", "url": "https://example.com"}}
      ],
      "modRules": []
    }"#;

    #[test]
    fn parses_and_checks() {
        let c = parse_manifest(MANIFEST).unwrap();
        assert_eq!(c.mods.len(), 4);
        let mut cet = sample_mod("107-1000", "CET");
        cet.md5 = Some("aa".into());
        let pc = pre_check(&c, &[cet], false, Some("2.21.1"), false);
        assert_eq!(pc.installed, 1);
        assert_eq!(pc.missing.len(), 2, "jacket and the off-site one; the optional one is left out");
        assert_eq!(pc.manual_clicks, 2);
        assert_eq!(pc.external, 1);
        assert_eq!(pc.bytes_to_download, 300);
        assert_eq!(pc.game_version_ok, Some(true));
        let pc = pre_check(&c, &[], true, Some("2.3"), true);
        assert_eq!(pc.manual_clicks, 1, "premium only clicks for the off-site mod");
        assert_eq!(pc.game_version_ok, Some(false));
        let items = queue_items(&c, &pc, "abc");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].origin, "collection:abc");
    }

    #[test]
    fn rejects_other_games_and_empty() {
        assert!(parse_manifest(r#"{"info": {"domainName": "skyrim"}, "mods": [{"name": "x", "source": {"type": "nexus"}}]}"#).is_err());
        assert!(parse_manifest(r#"{"info": {}, "mods": []}"#).is_err());
        assert!(parse_manifest("nope").is_err());
    }

    #[test]
    fn export_pins_and_links_dependencies() {
        let mut cet = sample_mod("107-1000", "CET");
        cet.md5 = Some("aa".into());
        let mut m = sample_mod("5-50", "Lua thing");
        m.frameworks = vec![crate::frameworks::Framework::Cet];
        m.redmods = vec!["MyRedmod".into()];
        let c = export("Mine", "me", "", &[cet, m], &["MyRedmod".into(), "Other".into()], Some("2.3")).unwrap();
        assert_eq!(c.mods[0].source.md5.as_deref(), Some("aa"));
        assert_eq!(c.mods[1].source.file_id, Some(50));
        assert_eq!(c.mod_rules.len(), 1);
        assert_eq!(c.mod_rules[0].kind, "requires");
        assert_eq!(c.pocket_mods.as_ref().unwrap()["loadOrder"], json!(["MyRedmod"]));
        // it reads back
        let text = serde_json::to_string(&c).unwrap();
        assert_eq!(parse_manifest(&text).unwrap(), c);
        assert!(export("x", "", "", &[], &[], None).is_err());
    }

    #[test]
    fn version_matching() {
        assert!(versions_match("2.21", "2.21"));
        assert!(versions_match("v2.21", "2.21.1"));
        assert!(!versions_match("2.2", "2.21"));
    }

    #[test]
    fn graphql_parsers() {
        let body = json!({"data": {"collectionsV2": {"totalCount": 1, "nodes": [{"slug": "abc", "name": "N", "endorsements": 3, "totalDownloads": "9",
            "user": {"name": "U"}, "latestPublishedRevision": {"revisionNumber": 4, "modCount": 12, "totalSize": 1000, "gameVersions": [{"reference": "2.21"}]}}]}}});
        let (cards, total) = parse_collections(&body).unwrap();
        assert_eq!(total, 1);
        assert_eq!(cards[0].downloads, 9);
        assert_eq!(cards[0].game_versions, vec!["2.21"]);
        let v = collections_variables(Some("night"), Some("2.21"), "downloads", 0, 100);
        assert_eq!(v["count"], 50);
        assert_eq!(v["filter"]["name"][0]["op"], "WILDCARD");
        let (rev, link) = parse_revision_link(&json!({"data": {"collectionRevision": {"revisionNumber": 4, "downloadLink": "/v1/collections/abc/revisions/4/download_link"}}})).unwrap();
        assert_eq!((rev, link.as_str()), (Some(4), "/v1/collections/abc/revisions/4/download_link"));
        assert_eq!(parse_collection_download(&json!({"download_links": [{"URI": "https://x/c.7z"}]})), vec!["https://x/c.7z"]);
    }

    #[test]
    fn reads_manifest_from_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zp = dir.path().join("c.zip");
        crate::archive::tests::make_zip(&zp, &[("collection.json", MANIFEST.as_bytes())]);
        let c = read_manifest_from_archive(&zp, &dir.path().join("x")).unwrap();
        assert_eq!(c.info.name, "Night City Plus");
    }
}
