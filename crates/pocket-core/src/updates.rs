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
pub struct Update { pub mod_id: u32, pub name: String, pub installed: String, pub latest: String, pub updated_at: String }

pub async fn check(client: &NexusClient, db: &Mutex<Db>) -> Result<Vec<Update>> {
    let installed: Vec<(u32, String, String, i64)> = {
        let d = db.lock().unwrap();
        let flags = d.flags()?;
        let mut st = d.conn.prepare("SELECT mod_id, name, version, installed_at FROM installs WHERE mod_id < 900000000")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<Vec<(u32, String, String, i64)>, _>>()?;
        rows.into_iter().filter(|r| !flags.ignored.contains(&r.0)).collect()
    };
    let mut out = Vec::new();
    for chunk in installed.chunks(50) {
        let ids: Vec<u32> = chunk.iter().map(|r| r.0).collect();
        for n in client.mods_by_id(&ids).await? {
            let Some(id) = n["modId"].as_u64().map(|v| v as u32) else { continue };
            let Some((_, name, ver, at)) = chunk.iter().find(|r| r.0 == id) else { continue };
            let latest = n["version"].as_str().unwrap_or_default().to_string();
            let updated_at = n["updatedAt"].as_str().unwrap_or_default().to_string();
            let newer = chrono::DateTime::parse_from_rfc3339(&updated_at).map(|t| t.timestamp() > *at).unwrap_or(false);
            if newer && !latest.is_empty() && crate::health::version_key(&latest) != crate::health::version_key(ver) {
                out.push(Update { mod_id: id, name: name.clone(), installed: ver.clone(), latest, updated_at });
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
        })).collect()).unwrap_or_default()
}
