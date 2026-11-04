//! Nexus records as the live API returns them. Field names and types were checked against the v2 schema by
//! introspection and against real responses on 2026-11-04 (docs/nexus/live-probe.md) unless marked UNVERIFIED.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// One mod as a browser card. Only the fields the card shows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModCard {
    pub mod_id: u32,
    pub uid: String,
    pub name: String,
    pub summary: String,
    pub author: Option<String>,
    pub uploader: Option<Uploader>,
    pub thumbnail_url: Option<String>,
    pub downloads: u64,
    pub endorsements: u64,
    /// Kilobytes. Inferred: Cyber Engine Tweaks reports 34051 for a ~33 MB archive.
    pub file_size: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub version: String,
    pub category: String,
    pub mod_category: Option<ModCategory>,
    pub adult_content: Option<bool>,
    pub status: String,
    /// The spec's unconfirmed list said a mod has no returnable tags. The live API returns them.
    #[serde(default)]
    pub tags: Vec<TagRef>,
    /// Requirement links, fetched in the same page query (they drive the framework chips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_requirements: Option<Requirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Uploader { pub name: String, pub member_id: u64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModCategory { pub category_id: u32, pub name: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagRef { pub id: String, pub name: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Requirements {
    pub nexus_requirements: RequirementPage,
    #[serde(default)]
    pub dlc_requirements: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequirementPage {
    pub total_count: u32,
    pub nodes: Vec<Requirement>,
}

/// `modId` and `gameId` come back as strings (GraphQL `ID`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    #[serde(deserialize_with = "id_u32")]
    pub mod_id: u32,
    pub mod_name: String,
    #[serde(default)]
    pub external_requirement: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A page of `mods()`. Facet counts come from the same response (`facetsData`: facet → value → count).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPage {
    pub total_count: u64,
    pub nodes_count: u64,
    #[serde(default)]
    pub facets_data: Option<Value>,
    pub nodes: Vec<ModCard>,
}

/// Detail view: one `mod()` plus `modFiles()` in a single request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDetail {
    pub mod_id: u32,
    pub uid: String,
    pub name: String,
    pub summary: String,
    /// BBCode mixed with HTML.
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub uploader: Option<Uploader>,
    pub picture_url: Option<String>,
    pub downloads: u64,
    pub endorsements: u64,
    pub file_size: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub category: String,
    pub adult_content: Option<bool>,
    pub status: String,
    #[serde(default)]
    pub tags: Vec<TagRef>,
    pub mod_requirements: Requirements,
}

/// A downloadable file of a mod.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_id: u32,
    pub name: String,
    pub version: String,
    /// MAIN, UPDATE, OPTIONAL, OLD_VERSION, MISCELLANEOUS, REMOVED, ARCHIVED
    pub category: String,
    /// Kilobytes.
    pub size: u64,
    /// BigInt, sent as a string.
    #[serde(default, deserialize_with = "opt_bigint")]
    pub size_in_bytes: Option<u64>,
    /// Unix seconds.
    pub date: i64,
    pub primary: i32,
    /// The archive's file name on Nexus, e.g. `ArchiveXL-4198-1-0-0-1648124175.zip`.
    pub uri: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl ModFile {
    pub fn bytes(&self) -> u64 { self.size_in_bytes.unwrap_or(self.size * 1024) }
    pub fn is_current(&self) -> bool { matches!(self.category.as_str(), "MAIN" | "UPDATE" | "OPTIONAL" | "MISCELLANEOUS") }
}

/// One entry of an archive's file list from `modFileContents`. The index is partial: recent files often have none.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub file_id: u32,
    pub mod_id: u32,
    pub file_path: String,
    #[serde(default, deserialize_with = "opt_bigint")]
    pub file_size: Option<u64>,
}

fn id_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = Value::deserialize(d)?;
    match &v {
        Value::String(s) => s.parse().map_err(serde::de::Error::custom),
        Value::Number(n) => n.as_u64().map(|n| n as u32).ok_or_else(|| serde::de::Error::custom("bad id")),
        _ => Err(serde::de::Error::custom("bad id")),
    }
}

fn opt_bigint<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    Ok(match v {
        Some(Value::String(s)) => s.parse().ok(),
        Some(Value::Number(n)) => n.as_u64(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real node from the live API (2026-11-04), trimmed.
    const CARD: &str = r#"{"modId": 4198, "uid": "14315126001766", "name": "ArchiveXL", "summary": "Archive extension loader", "author": "psiberx",
      "uploader": {"name": "psiberx", "memberId": 108159138}, "thumbnailUrl": "https://staticdelivery.nexusmods.com/mods/3333/images/thumbnails/4198/4198-1648116857-72076015.jpeg",
      "downloads": 13965225, "endorsements": 146754, "fileSize": 1046, "createdAt": "2022-03-24T12:22:09Z", "updatedAt": "2026-10-29T12:59:42Z", "version": "1.28.6",
      "category": "Modders Resources", "modCategory": {"categoryId": 12, "name": "Modders Resources"}, "adultContent": false, "status": "published",
      "tags": [{"id": "1075", "name": "Modder's Resource"}],
      "modRequirements": {"nexusRequirements": {"totalCount": 1, "nodes": [{"id": "[5517, 3333]", "modId": "2380", "modName": "RED4ext", "gameId": "3333", "url": "", "notes": "", "externalRequirement": false}]}}}"#;

    #[test]
    fn parses_a_live_card() {
        let c: ModCard = serde_json::from_str(CARD).unwrap();
        assert_eq!(c.mod_id, 4198);
        assert_eq!(c.tags[0].name, "Modder's Resource");
        let r = c.mod_requirements.unwrap();
        assert_eq!(r.nexus_requirements.nodes[0].mod_id, 2380);
    }

    #[test]
    fn parses_a_live_file() {
        let f: ModFile = serde_json::from_str(r#"{"fileId": 22762, "name": "ArchiveXL", "version": "1.0.0", "category": "OLD_VERSION", "categoryId": 4,
            "size": 246, "sizeInBytes": "251411", "date": 1648124175, "primary": 0, "uri": "ArchiveXL-4198-1-0-0-1648124175.zip", "description": ""}"#).unwrap();
        assert_eq!(f.bytes(), 251411);
        assert!(!f.is_current());
    }
}
