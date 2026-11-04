//! Update checks for installed mods: one `legacyModsByDomain` request per 50 mods, anonymous. A mod has an update
//! when its Nexus version differs from the installed file's version and it was updated after the install.
//! Ignored mods are skipped. (The v1 `mods/updated.json` call needs a key and gives only IDs; this gives versions.)

use crate::db::Db;
use crate::error::Result;
use crate::nexus::NexusClient;
use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub mod_id: u32, pub name: String, pub installed: String, pub latest: String, pub updated_at: String,
    /// The install it is for, and the Nexus file that updates it (the same file line: 2K stays 2K).
    #[serde(default)]
    pub install_id: i64,
    #[serde(default)]
    pub file_id: u32,
}

/// Two steps: one batch request finds the mods whose page changed after they were installed; for those, the mod's
/// file list (one cached request each) says whether the installed file has a newer copy of its own. A page edit,
/// a new optional file or a new variant is not an update.
pub async fn check(client: &NexusClient, db: &Mutex<Db>) -> Result<Vec<Update>> {
    let installed: Vec<(i64, u32, u32, String, String, i64, Option<String>)> = {
        let d = db.lock().unwrap();
        let flags = d.flags()?;
        let mut st = d.conn.prepare("SELECT id, mod_id, file_id, name, version, installed_at, file_name FROM installs WHERE mod_id < 900000000")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
            .collect::<std::result::Result<Vec<(i64, u32, u32, String, String, i64, Option<String>)>, _>>()?;
        rows.into_iter().filter(|r| !flags.ignored.contains(&r.1)).collect()
    };
    let mut ids: Vec<u32> = installed.iter().map(|r| r.1).collect();
    ids.sort();
    ids.dedup();
    let mut changed: Vec<(u32, String)> = Vec::new();
    for chunk in ids.chunks(50) {
        for n in client.mods_by_id(chunk).await? {
            let Some(id) = n["modId"].as_u64().map(|v| v as u32) else { continue };
            let updated_at = n["updatedAt"].as_str().unwrap_or_default().to_string();
            let at = chrono::DateTime::parse_from_rfc3339(&updated_at).map(|t| t.timestamp()).unwrap_or(0);
            // the page changed after an install of it, or shows another version than an installed one (a mod tracked
            // from Vortex can be old already when it is tracked)
            let page_version = crate::health::version_key(n["version"].as_str().unwrap_or_default());
            if installed.iter().filter(|r| r.1 == id).any(|r| at > r.5 || (!r.4.is_empty() && crate::health::version_key(&r.4) != page_version)) {
                changed.push((id, updated_at));
            }
        }
    }
    let skipped = skipped(&db.lock().unwrap());
    let mut out = Vec::new();
    for (mod_id, updated_at) in changed {
        let Ok((_, files)) = crate::browse::mod_files(client, db, mod_id, Some(&updated_at)).await else { continue };
        for (install_id, _, file_id, name, ver, at, file_name) in installed.iter().filter(|r| r.1 == mod_id) {
            let Some(next) = crate::browse::successor(&files, *file_id, file_name.as_deref()) else { continue };
            // an unknown installed version (found in the game folder) cannot be compared
            let _ = at;
            let newer = next.file_id != *file_id && !ver.is_empty()
                && crate::health::version_key(&next.version) != crate::health::version_key(ver);
            let skip = skipped.get(&mod_id).is_some_and(|v| crate::health::version_key(v) == crate::health::version_key(&next.version));
            if newer && !skip {
                out.push(Update { mod_id, name: name.clone(), installed: ver.clone(), latest: next.version.clone(), updated_at: updated_at.clone(), install_id: *install_id, file_id: next.file_id });
            }
        }
    }
    let d = db.lock().unwrap();
    d.set_setting("updates.last", &serde_json::to_string(&out).unwrap_or_default())?;
    Ok(out)
}

/// The result of the last check, without asking Nexus.
pub fn last(db: &Db) -> Vec<Update> {
    db.setting("updates.last").ok().flatten().and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .map(|v| v.into_iter().filter_map(|u| Some(Update {
            mod_id: u["modId"].as_u64()? as u32, name: u["name"].as_str()?.into(), installed: u["installed"].as_str()?.into(),
            latest: u["latest"].as_str()?.into(), updated_at: u["updatedAt"].as_str()?.into(),
            install_id: u["installId"].as_i64().unwrap_or(0), file_id: u["fileId"].as_u64().unwrap_or(0) as u32,
        })).collect()).unwrap_or_default()
}

/// Versions the user chose to skip: mod id to the version (a later version shows again).
pub fn skipped(db: &Db) -> std::collections::BTreeMap<u32, String> {
    db.setting("updates.skip").ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Skip one version of a mod's update, and drop it from the last result.
pub fn skip(db: &Db, mod_id: u32, version: &str) -> Result<Vec<Update>> {
    let mut m = skipped(db);
    m.insert(mod_id, version.to_string());
    db.set_setting("updates.skip", &serde_json::to_string(&m).unwrap_or_default())?;
    let left: Vec<Update> = last(db).into_iter().filter(|u| !(u.mod_id == mod_id && crate::health::version_key(&u.latest) == crate::health::version_key(version))).collect();
    db.set_setting("updates.last", &serde_json::to_string(&left).unwrap_or_default())?;
    Ok(left)
}
