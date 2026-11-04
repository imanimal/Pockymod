//! Cyberpunk-aware facts the Nexus API does not give: which frameworks a mod needs, and its mod type.
//! Derived from, strongest first: the archive's file list (when Nexus has it indexed), requirement links, tags,
//! and words in the name and summary. Each result says where it came from.

use super::frameworks::{self, Framework, FRAMEWORKS};
use super::layout::{plan_install, ModType};
use crate::nexus::types::ModCard;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Source { Files, Requirements, Tags, Text, Identity }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkUse { pub framework: Framework, pub key: &'static str, pub name: &'static str, pub source: Source }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub frameworks: Vec<FrameworkUse>,
    pub mod_type: ModType,
    pub type_source: Option<Source>,
    /// Set when the mod is itself one of the frameworks.
    pub is_framework: Option<Framework>,
}

/// Text that names a framework, as a whole word (case-insensitive; "CET" only in capitals).
fn mentions(text: &str, word: &str) -> bool {
    let (hay, needle) = if word.len() <= 3 { (text.to_string(), word.to_string()) } else { (text.to_lowercase(), word.to_lowercase()) };
    let b = hay.as_bytes();
    let mut start = 0;
    while let Some(i) = hay[start..].find(&needle) {
        let at = start + i;
        let before = at.checked_sub(1).map(|j| b[j]).unwrap_or(b' ');
        let after = b.get(at + needle.len()).copied().unwrap_or(b' ');
        if !before.is_ascii_alphanumeric() && !after.is_ascii_alphanumeric() { return true; }
        start = at + needle.len();
    }
    false
}

pub fn classify(card: &ModCard, file_list: &[String]) -> Classification {
    let mut found: Vec<FrameworkUse> = Vec::new();
    fn add(found: &mut Vec<FrameworkUse>, f: Framework, source: Source) {
        if !found.iter().any(|u| u.framework == f) {
            let i = frameworks::info(f);
            found.push(FrameworkUse { framework: f, key: i.key, name: i.name, source });
        }
    }
    let is_framework = frameworks::by_mod_id(card.mod_id).map(|i| i.id);

    let plan = (!file_list.is_empty()).then(|| plan_install(file_list, &card.name));
    if let Some(p) = &plan { for f in &p.uses { if Some(*f) != is_framework { add(&mut found, *f, Source::Files); } } }
    if let Some(r) = &card.mod_requirements {
        for req in &r.nexus_requirements.nodes {
            if let Some(i) = frameworks::by_mod_id(req.mod_id) { add(&mut found, i.id, Source::Requirements); }
        }
    }
    let tag_names: BTreeSet<&str> = card.tags.iter().map(|t| t.name.as_str()).collect();
    for i in FRAMEWORKS {
        if Some(i.id) == is_framework { continue; }
        if i.tag.is_some_and(|t| tag_names.contains(t)) { add(&mut found, i.id, Source::Tags); }
    }
    let text = format!("{} {}", card.name, card.summary);
    for i in FRAMEWORKS {
        if Some(i.id) == is_framework { continue; }
        if i.words.iter().any(|w| mentions(&text, w)) { add(&mut found, i.id, Source::Text); }
    }
    // frameworks pull in what they need (ArchiveXL needs RED4ext)
    let direct: Vec<(Framework, Source)> = found.iter().map(|u| (u.framework, u.source)).collect();
    for (f, source) in direct {
        for need in frameworks::info(f).needs { add(&mut found, *need, source); }
    }
    found.sort_by_key(|u| u.framework);

    let (mod_type, type_source) = if is_framework.is_some() {
        (ModType::Framework, Some(Source::Identity))
    } else if let Some(t) = plan.as_ref().and_then(|p| p.mod_type).filter(|t| *t != ModType::Unknown) {
        (t, Some(Source::Files))
    } else if tag_names.contains("REDMod") {
        (ModType::Redmod, Some(Source::Tags))
    } else if tag_names.contains("Modder's Resource") || card.category == "Modders Resources" {
        (ModType::Framework, Some(Source::Tags))
    } else if card.category == "Scripts" || found.iter().any(|u| matches!(u.framework, Framework::Redscript | Framework::Cet)) && !found.iter().any(|u| u.framework == Framework::ArchiveXl) {
        (ModType::Script, Some(Source::Text))
    } else if found.iter().any(|u| u.framework == Framework::ArchiveXl) || matches!(card.category.as_str(), "Armour and Clothing" | "Appearance" | "Visuals and Graphics" | "Weapons" | "Vehicles") {
        (ModType::Archive, Some(Source::Text))
    } else {
        (ModType::Unknown, None)
    };
    Classification { frameworks: found, mod_type, type_source, is_framework }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::types::*;

    fn card(id: u32, name: &str, summary: &str, category: &str, tags: &[&str], reqs: &[(u32, &str)]) -> ModCard {
        ModCard {
            mod_id: id, uid: "1".into(), name: name.into(), summary: summary.into(), author: None, uploader: None, thumbnail_url: None,
            downloads: 0, endorsements: 0, file_size: None, created_at: String::new(), updated_at: String::new(), version: "1".into(),
            category: category.into(), mod_category: None, adult_content: Some(false), status: "published".into(),
            tags: tags.iter().map(|t| TagRef { id: "0".into(), name: t.to_string() }).collect(),
            mod_requirements: Some(Requirements { nexus_requirements: RequirementPage { total_count: reqs.len() as u32,
                nodes: reqs.iter().map(|(i, n)| Requirement { mod_id: *i, mod_name: n.to_string(), external_requirement: false, url: None, notes: None }).collect() }, dlc_requirements: vec![] }),
        }
    }

    #[test]
    fn requirement_links_win_and_pull_in_red4ext() {
        // a live card: "Blackwall Monowire - ArasakaCyberarm Add-on" requires ArchiveXL (4198)
        let c = classify(&card(17173, "Blackwall Monowire", "", "Weapons", &[], &[(4198, "ArchiveXL"), (1099, "Arasaka Cyberarms")]), &[]);
        let keys: Vec<_> = c.frameworks.iter().map(|f| (f.key, f.source)).collect();
        assert_eq!(keys, [("red4ext", Source::Requirements), ("archivexl", Source::Requirements)]);
        assert_eq!(c.mod_type, ModType::Archive);
    }

    #[test]
    fn a_framework_is_typed_by_identity_and_does_not_need_itself() {
        let c = classify(&card(4198, "ArchiveXL", "Archive extension loader", "Modders Resources", &[], &[(2380, "RED4ext")]), &[]);
        assert_eq!(c.is_framework, Some(Framework::ArchiveXl));
        assert_eq!(c.mod_type, ModType::Framework);
        assert_eq!(c.frameworks.len(), 1);
    }

    #[test]
    fn tags_and_words() {
        let c = classify(&card(9, "Quick hack menu", "A CET mod for fast travel", "Gameplay", &["Cyber Engine Tweaks"], &[]), &[]);
        assert_eq!(c.frameworks[0].source, Source::Tags);
        assert_eq!(c.mod_type, ModType::Script);
        let d = classify(&card(10, "Recet stuff", "codewarefoo", "Gameplay", &["REDMod"], &[]), &[]);
        assert!(d.frameworks.is_empty(), "partial words do not count");
        assert_eq!(d.mod_type, ModType::Redmod);
    }

    #[test]
    fn file_list_is_strongest() {
        let files = vec!["mods/X/info.json".to_string(), "mods/X/archives/x.archive".to_string(), "r6/scripts/X/a.reds".to_string()];
        let c = classify(&card(11, "X", "", "Gameplay", &[], &[]), &files);
        assert_eq!((c.mod_type, c.type_source), (ModType::Redmod, Some(Source::Files)));
        assert_eq!(c.frameworks[0].source, Source::Files);
    }
}
