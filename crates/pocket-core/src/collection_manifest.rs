//! What a Nexus collection's own manifest adds to its mod list: the installer options its curator picked for each
//! mod, and its rules (this mod's files win over that one's, this one needs or clashes with that one).
//!
//! The manifest is `collection.json` inside the collection's archive. Its layout follows Vortex's collections
//! extension (`ICollection`, `ICollectionMod`, `ICollectionModRule` in github.com/Nexus-Mods/vortex), read
//! 2026-11-04. UNVERIFIED against the live API: the revision's `downloadLink` field and the download-link answer it
//! points to. Everything here is best effort: without the manifest a collection still installs, each mod with its
//! installer's defaults, as before.
//!
//! - choices: `mods[].choices = { type: "fomod", options: [{ name: <step>, groups: [{ name, choices: [{ name, idx }] }] }] }`,
//!   kept by Nexus mod and file and used when that file's installer asks (fomod::choices_by_name).
//! - rules: `modRules[] = { source, type, reference }`, each end an `IReference` (`fileMD5`, `logicalFileName`,
//!   `repo.modId`). "before" means the source loads first, so the reference's files win; "after" the other way.
//!   They become standing "wins over" choices by Nexus mod, never replacing one the user made. "conflicts" and
//!   "requires" are reported on the check.

use crate::db::{now, Db};
use crate::error::{Error, Result};
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A curator's pick, by name: (step, group, plugin, plugin position).
pub type NamedChoice = (String, String, String, Option<usize>);

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Picks by (mod id, file id).
    #[serde(skip)]
    pub choices: BTreeMap<(u32, u32), Vec<NamedChoice>>,
    /// `collection.json` as it came, so an install reads the picks again without a second download.
    #[serde(skip)]
    pub text: String,
    /// Mods with installer picks, for the check: their count.
    pub with_choices: usize,
    /// (winner mod, loser mod).
    pub wins: Vec<(u32, u32)>,
    /// (mod, mod it clashes with).
    pub conflicts: Vec<(u32, u32)>,
    /// (mod, mod it needs).
    pub requires: Vec<(u32, u32)>,
    /// Curator's notes for a mod (`instructions`), by mod id.
    pub instructions: BTreeMap<u32, String>,
}

/// Read `collection.json`.
pub fn parse(text: &str) -> Result<Manifest> {
    let v: Value = serde_json::from_str(text).map_err(|e| Error::Invalid(format!("The collection's manifest is not valid JSON: {e}")))?;
    let mods = v["mods"].as_array().cloned().unwrap_or_default();
    let mut m = Manifest::default();
    // every way a rule can name a mod, to its Nexus mod id
    let mut by_md5: BTreeMap<String, u32> = BTreeMap::new();
    let mut by_logical: BTreeMap<String, u32> = BTreeMap::new();
    for x in &mods {
        let s = &x["source"];
        let (Some(mod_id), Some(file_id)) = (num(&s["modId"]), num(&s["fileId"])) else { continue };
        if let Some(md5) = s["md5"].as_str() { by_md5.insert(md5.to_ascii_lowercase(), mod_id); }
        if let Some(l) = s["logicalFilename"].as_str() { by_logical.insert(l.to_ascii_lowercase(), mod_id); }
        if let Some(t) = x["instructions"].as_str().filter(|t| !t.trim().is_empty()) { m.instructions.insert(mod_id, t.trim().chars().take(2000).collect()); }
        let c = &x["choices"];
        if c["type"].as_str() == Some("fomod") {
            let mut picks = Vec::new();
            for step in c["options"].as_array().into_iter().flatten() {
                let sn = step["name"].as_str().unwrap_or("").to_string();
                for g in step["groups"].as_array().into_iter().flatten() {
                    let gn = g["name"].as_str().unwrap_or("").to_string();
                    for p in g["choices"].as_array().into_iter().flatten() {
                        picks.push((sn.clone(), gn.clone(), p["name"].as_str().unwrap_or("").to_string(), p["idx"].as_u64().map(|i| i as usize)));
                    }
                }
            }
            if !picks.is_empty() { m.choices.insert((mod_id, file_id), picks); }
        }
    }
    m.with_choices = m.choices.len();
    m.text = text.to_string();
    let who = |r: &Value| -> Option<u32> {
        num(&r["repo"]["modId"])
            .or_else(|| r["fileMD5"].as_str().and_then(|h| by_md5.get(&h.to_ascii_lowercase()).copied()))
            .or_else(|| r["logicalFileName"].as_str().and_then(|l| by_logical.get(&l.to_ascii_lowercase()).copied()))
    };
    for r in v["modRules"].as_array().into_iter().flatten() {
        let (Some(a), Some(b)) = (who(&r["source"]), who(&r["reference"])) else { continue };
        if a == b { continue; }
        match r["type"].as_str().unwrap_or("") {
            "before" => m.wins.push((b, a)),
            "after" => m.wins.push((a, b)),
            "conflicts" => m.conflicts.push((a, b)),
            "requires" => m.requires.push((a, b)),
            _ => {}
        }
    }
    Ok(m)
}

fn num(v: &Value) -> Option<u32> { v.as_u64().map(|n| n as u32).or_else(|| v.as_str().and_then(|s| s.parse().ok())) }

/// Keep the manifest's picks and rules for the installs that follow. Rules never replace one the user made.
pub fn store(db: &Db, m: &Manifest, names: &BTreeMap<u32, String>) -> Result<usize> {
    for ((mod_id, file_id), picks) in &m.choices {
        db.set_setting(&format!("collection.choices.{mod_id}.{file_id}"), &serde_json::to_string(picks).unwrap_or_default())?;
    }
    let mut added = 0;
    for (w, l) in &m.wins {
        let had: i64 = db.conn.query_row("SELECT COUNT(*) FROM conflict_rules WHERE (winner_mod = ?1 AND loser_mod = ?2) OR (winner_mod = ?2 AND loser_mod = ?1)", params![w, l], |r| r.get(0))?;
        if had > 0 { continue; }
        let name = |id: &u32| names.get(id).cloned().unwrap_or_else(|| format!("Mod {id}"));
        // by Nexus mod only (no install yet): rules::verdict matches the mods
        db.conn.execute("INSERT INTO conflict_rules (winner_mod, winner_install, winner_name, loser_mod, loser_install, loser_name, files, created_at) VALUES (?1, -1, ?2, ?3, -1, ?4, 0, ?5)",
            params![w, name(w), l, name(l), now()])?;
        added += 1;
    }
    Ok(added)
}

/// The curator's picks for this file, when a collection gave some.
pub fn choices_for(db: &Db, mod_id: u32, file_id: u32) -> Option<Vec<NamedChoice>> {
    db.setting(&format!("collection.choices.{mod_id}.{file_id}")).ok().flatten().and_then(|t| serde_json::from_str(&t).ok())
}

/// The collection's archive, then `collection.json` inside it. None when the revision offers no download.
pub async fn fetch(client: &crate::nexus::NexusClient, slug: &str, revision: Option<u32>, tmp: &std::path::Path) -> Result<Option<Manifest>> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct R { collection_revision: Rev }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Rev { download_link: Option<String> }
    let q = "query RevisionDownload($slug: String!, $revision: Int) { collectionRevision(slug: $slug, revision: $revision, domainName: \"cyberpunk2077\") { downloadLink } }";
    let r: R = client.graphql(q, serde_json::json!({ "slug": slug, "revision": revision })).await?;
    let Some(link) = r.collection_revision.download_link else { return Ok(None) };
    // a v1 path that answers with CDN links (as for mod files)
    let path = link.strip_prefix("https://api.nexusmods.com/v1").or_else(|| link.strip_prefix("/v1")).unwrap_or(&link).to_string();
    let answer: Value = client.v1_get(&path).await?;
    let list = answer["download_links"].as_array().or_else(|| answer.as_array()).cloned().unwrap_or_default();
    let Some(url) = list.iter().find_map(|l| l["URI"].as_str().map(str::to_owned)) else { return Ok(None) };
    std::fs::create_dir_all(tmp)?;
    let dl = crate::download::download(client.download_http(), &url, tmp, &format!("collection-{}.7z", slug.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>()), |_, _| true).await?;
    let out = tmp.join("unpacked");
    let _ = std::fs::remove_dir_all(&out);
    let files = crate::archive::extract(&dl.path, &out)?;
    let m = files.iter().find(|f| f.to_ascii_lowercase().ends_with("collection.json"))
        .map(|f| std::fs::read_to_string(out.join(f)).map_err(Error::from).and_then(|t| parse(&t)))
        .transpose();
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_file(&dl.path);
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = r#"{ "info": { "name": "Arms" },
      "mods": [
        { "name": "Arasaka Cyberarms", "source": { "type": "nexus", "modId": 100, "fileId": 1001, "md5": "AAA", "logicalFilename": "arms" },
          "choices": { "type": "fomod", "options": [ { "name": "Model", "groups": [ { "name": "Arm", "choices": [ { "name": "Mantis", "idx": 0 } ] } ] } ] },
          "instructions": "Pick the mantis version." },
        { "name": "Retexture", "source": { "type": "nexus", "modId": 200, "fileId": 2001, "md5": "bbb" } }
      ],
      "modRules": [
        { "source": { "fileMD5": "aaa" }, "type": "before", "reference": { "fileMD5": "BBB" } },
        { "source": { "logicalFileName": "arms" }, "type": "conflicts", "reference": { "repo": { "modId": "200" } } }
      ] }"#;

    #[test]
    fn reads_choices_and_rules() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.choices[&(100, 1001)], vec![("Model".to_string(), "Arm".to_string(), "Mantis".to_string(), Some(0))]);
        assert_eq!(m.wins, vec![(200, 100)], "before: the reference wins");
        assert_eq!(m.conflicts, vec![(100, 200)]);
        assert!(m.instructions[&100].contains("mantis"));
    }

    #[test]
    fn stored_rules_never_replace_the_users() {
        let db = Db::memory().unwrap();
        db.conn.execute("INSERT INTO conflict_rules (winner_mod, winner_install, winner_name, loser_mod, loser_install, loser_name, files, created_at) VALUES (100, 1, 'a', 200, 2, 'b', 3, 0)", []).unwrap();
        let m = parse(SAMPLE).unwrap();
        assert_eq!(store(&db, &m, &BTreeMap::new()).unwrap(), 0);
        assert!(choices_for(&db, 100, 1001).is_some());
        let fresh = Db::memory().unwrap();
        assert_eq!(store(&fresh, &m, &BTreeMap::new()).unwrap(), 1);
    }
}
