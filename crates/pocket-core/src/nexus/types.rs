//! The shapes the app works with. Parsers in `queries.rs` fill these from Nexus responses.

use crate::classify::{Compat, Derived, ModKind};
use crate::frameworks::Framework;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    #[default]
    Relevance,
    Downloads,
    Endorsements,
    Updated,
    Created,
    Size,
    Name,
}

/// One Browse request. The first group goes to Nexus; the second is applied locally (the API has no such filters).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowseQuery {
    pub text: Option<String>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub sort: SortKey,
    pub ascending: bool,
    pub updated_after: Option<i64>,
    pub updated_before: Option<i64>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub min_downloads: Option<u64>,
    pub max_downloads: Option<u64>,
    pub min_endorsements: Option<u64>,
    pub max_endorsements: Option<u64>,
    /// Kilobytes, like Nexus's own size filter. UNVERIFIED unit.
    pub min_size_kb: Option<u64>,
    pub max_size_kb: Option<u64>,
    pub include_adult: bool,
    pub offset: u32,
    pub count: u32,

    // local-only
    pub frameworks: Vec<Framework>,
    pub kinds: Vec<ModKind>,
    pub compatible_only: bool,
    pub show_hidden: bool,
    /// Some(true): only downloaded, Some(false): only not downloaded.
    pub downloaded: Option<bool>,
}

impl BrowseQuery {
    /// The part of the query that goes to Nexus, as a cache key. Local filters don't change what Nexus returns.
    pub fn remote_key(&self) -> String {
        let mut q = self.clone();
        q.frameworks.clear();
        q.kinds.clear();
        q.compatible_only = false;
        q.show_hidden = false;
        q.downloaded = None;
        format!("mods:{}", serde_json::to_string(&q).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModCard {
    pub mod_id: u64,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
    pub picture_url: Option<String>,
    pub downloads: u64,
    pub endorsements: u64,
    pub updated_at: i64,
    pub created_at: i64,
    /// Bytes, converted from what Nexus sends.
    pub size_bytes: Option<u64>,
    pub category: Option<String>,
    pub adult: bool,
    pub version: Option<String>,
    /// Filled locally.
    #[serde(default)]
    pub derived: Derived,
    #[serde(default = "unknown_compat")]
    pub compat: Compat,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub downloaded: bool,
    #[serde(default)]
    pub installed: bool,
}

fn unknown_compat() -> Compat {
    Compat::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct FacetCount {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Facets {
    /// From Nexus (facetsData), for the whole result set.
    pub categories: Vec<FacetCount>,
    pub tags: Vec<FacetCount>,
    /// Derived locally, counted over the results loaded so far.
    pub frameworks: Vec<FacetCount>,
    pub kinds: Vec<FacetCount>,
    pub compatible: u64,
    pub downloaded: u64,
    pub not_downloaded: u64,
    pub hidden: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModPage {
    pub mods: Vec<ModCard>,
    /// Nexus's total for the remote part of the query.
    pub total: u64,
    pub offset: u32,
    pub facets: Facets,
    /// True when the page came from the local cache.
    pub cached: bool,
    /// How many results local filters removed from this page.
    pub filtered_out: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_id: u64,
    pub name: String,
    pub version: Option<String>,
    /// MAIN, OPTIONAL, UPDATE, MISCELLANEOUS, OLD_VERSION, ARCHIVED
    pub category: String,
    pub size_bytes: u64,
    pub file_name: String,
    pub uploaded_at: i64,
    pub is_primary: bool,
    pub description: Option<String>,
    pub content_preview_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub mod_id: Option<u64>,
    pub name: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModDetail {
    pub card: ModCard,
    /// BBCode as Nexus stores it; the UI renders a safe subset.
    pub description: String,
    pub files: Vec<ModFile>,
    pub requirements: Vec<Requirement>,
    /// Which of the files the install key picks (primary main file, else newest main file).
    pub recommended_file_id: Option<u64>,
    pub endorse_status: Option<String>,
    pub tracked: Option<bool>,
    /// The archive file list behind the derived kind, when Nexus has a content preview.
    pub preview_files: Vec<String>,
    /// True when the mod's page says files must be downloaded by hand (no mod manager download).
    pub manual_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub user_id: u64,
    pub name: String,
    pub is_premium: bool,
    pub is_supporter: bool,
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedMod {
    pub mod_id: u64,
    pub latest_file_update: i64,
    pub latest_mod_activity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub parent: Option<u64>,
}
