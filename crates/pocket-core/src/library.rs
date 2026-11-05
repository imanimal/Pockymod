//! The local database (SQLite): installed mods, profiles, load order, hidden and downloaded marks, the
//! derived-facts index for Browse filters, settings, and a TTL cache for Nexus responses.

use crate::classify::{Derived, ModKind};
use crate::frameworks::Framework;
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = "Default";

const MIGRATIONS: &[&str] = &[
    // 1
    r#"
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE cache (key TEXT PRIMARY KEY, value TEXT NOT NULL, expires INTEGER NOT NULL);
    CREATE TABLE derived (mod_id INTEGER PRIMARY KEY, card TEXT, data TEXT NOT NULL, updated INTEGER NOT NULL);
    CREATE TABLE hidden (mod_id INTEGER PRIMARY KEY, at INTEGER NOT NULL);
    CREATE TABLE downloads (mod_id INTEGER NOT NULL, file_id INTEGER NOT NULL, path TEXT NOT NULL, md5 TEXT, size INTEGER,
        at INTEGER NOT NULL, PRIMARY KEY (mod_id, file_id));
    CREATE TABLE mods (key TEXT PRIMARY KEY, data TEXT NOT NULL);
    CREATE TABLE profiles (name TEXT PRIMARY KEY, created INTEGER NOT NULL);
    CREATE TABLE profile_mods (profile TEXT NOT NULL, mod_key TEXT NOT NULL, enabled INTEGER NOT NULL,
        PRIMARY KEY (profile, mod_key));
    CREATE TABLE load_order (profile TEXT NOT NULL, position INTEGER NOT NULL, folder TEXT NOT NULL,
        PRIMARY KEY (profile, folder));
    CREATE TABLE collections (name TEXT PRIMARY KEY, data TEXT NOT NULL, updated INTEGER NOT NULL);
    "#,
];

/// A mod in the library (installed into the store; deployed when enabled in the active profile).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub key: String,
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    pub name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub kind: ModKind,
    pub frameworks: Vec<Framework>,
    /// Required Nexus mod IDs, when known.
    pub requirements: Vec<u64>,
    pub md5: Option<String>,
    pub size: u64,
    pub installed_at: i64,
    /// When this file was uploaded to Nexus.
    pub uploaded_at: Option<i64>,
    pub archive_path: Option<PathBuf>,
    pub redmods: Vec<String>,
    pub thumbnail: Option<String>,
    pub unplaced: Vec<String>,
    /// Set by update checks: a newer file exists on Nexus.
    #[serde(default)]
    pub update_available: Option<UpdateInfo>,
    #[serde(default)]
    pub endorsed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub file_id: u64,
    pub version: Option<String>,
    pub uploaded_at: i64,
    pub file_name: String,
    pub size_bytes: u64,
}

impl InstalledMod {
    pub fn key_for(mod_id: u64, file_id: u64) -> String {
        format!("{mod_id}-{file_id}")
    }
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: &Path) -> Result<Library> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| Error::io_at(e, p))?;
        }
        Library::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Library> {
        Library::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Library> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(m)?;
            conn.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        let lib = Library { conn };
        lib.conn.execute("INSERT OR IGNORE INTO profiles (name, created) VALUES (?1, ?2)", params![DEFAULT_PROFILE, crate::timefmt::now()])?;
        Ok(lib)
    }

    // settings ------------------------------------------------------------------------------------------

    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let v: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, serde_json::to_string(value)?])?;
        Ok(())
    }

    // cache ---------------------------------------------------------------------------------------------

    pub fn cache_get(&self, key: &str) -> Result<Option<Value>> {
        let now = crate::timefmt::now();
        let v: Option<String> = self.conn.query_row("SELECT value FROM cache WHERE key = ?1 AND expires > ?2", params![key, now], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn cache_put(&self, key: &str, value: &Value, ttl_secs: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO cache (key, value, expires) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value = excluded.value, expires = excluded.expires",
            params![key, value.to_string(), crate::timefmt::now() + ttl_secs],
        )?;
        Ok(())
    }

    pub fn cache_forget_prefix(&self, prefix: &str) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM cache WHERE key LIKE ?1 ESCAPE '\\'", [format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"))])?)
    }

    pub fn cache_purge_expired(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM cache WHERE expires <= ?1", [crate::timefmt::now()])?)
    }

    // derived-facts index -------------------------------------------------------------------------------

    pub fn put_derived(&self, mod_id: u64, d: &Derived) -> Result<()> {
        self.conn.execute(
            "INSERT INTO derived (mod_id, data, updated) VALUES (?1, ?2, ?3) ON CONFLICT(mod_id) DO UPDATE SET data = excluded.data, updated = excluded.updated",
            params![mod_id as i64, serde_json::to_string(d)?, crate::timefmt::now()],
        )?;
        Ok(())
    }

    pub fn derived(&self, mod_id: u64) -> Result<Option<Derived>> {
        let v: Option<String> = self.conn.query_row("SELECT data FROM derived WHERE mod_id = ?1", [mod_id as i64], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn derived_many(&self, ids: &[u64]) -> Result<HashMap<u64, Derived>> {
        let mut out = HashMap::new();
        let mut stmt = self.conn.prepare_cached("SELECT data FROM derived WHERE mod_id = ?1")?;
        for id in ids {
            if let Some(s) = stmt.query_row([*id as i64], |r| r.get::<_, String>(0)).optional()? {
                if let Ok(d) = serde_json::from_str(&s) {
                    out.insert(*id, d);
                }
            }
        }
        Ok(out)
    }

    // hidden / downloaded -------------------------------------------------------------------------------

    pub fn set_hidden(&self, mod_id: u64, hidden: bool) -> Result<()> {
        if hidden {
            self.conn.execute("INSERT OR REPLACE INTO hidden (mod_id, at) VALUES (?1, ?2)", params![mod_id as i64, crate::timefmt::now()])?;
        } else {
            self.conn.execute("DELETE FROM hidden WHERE mod_id = ?1", [mod_id as i64])?;
        }
        Ok(())
    }

    pub fn hidden(&self) -> Result<HashSet<u64>> {
        let mut stmt = self.conn.prepare("SELECT mod_id FROM hidden")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(|v| v as u64).collect())
    }

    pub fn mark_downloaded(&self, mod_id: u64, file_id: u64, path: &Path, md5: Option<&str>, size: u64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO downloads (mod_id, file_id, path, md5, size, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![mod_id as i64, file_id as i64, path.to_string_lossy(), md5, size as i64, crate::timefmt::now()],
        )?;
        Ok(())
    }

    pub fn downloaded_mods(&self) -> Result<HashSet<u64>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT mod_id FROM downloads")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(|v| v as u64).collect())
    }

    /// A previously downloaded archive for this file, if it is still on disk.
    pub fn downloaded_file(&self, mod_id: u64, file_id: u64) -> Result<Option<(PathBuf, Option<String>)>> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row("SELECT path, md5 FROM downloads WHERE mod_id = ?1 AND file_id = ?2", params![mod_id as i64, file_id as i64], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(row.map(|(p, m)| (PathBuf::from(p), m)).filter(|(p, _)| p.is_file()))
    }

    // installed mods ------------------------------------------------------------------------------------

    pub fn upsert_mod(&self, m: &InstalledMod) -> Result<()> {
        self.conn.execute(
            "INSERT INTO mods (key, data) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET data = excluded.data",
            params![m.key, serde_json::to_string(m)?],
        )?;
        Ok(())
    }

    pub fn mods(&self) -> Result<Vec<InstalledMod>> {
        let mut stmt = self.conn.prepare("SELECT data FROM mods")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut v: Vec<InstalledMod> = rows.filter_map(|r| r.ok()).filter_map(|s| serde_json::from_str(&s).ok()).collect();
        v.sort_by_key(|a| a.name.to_lowercase());
        Ok(v)
    }

    pub fn get_mod(&self, key: &str) -> Result<Option<InstalledMod>> {
        let v: Option<String> = self.conn.query_row("SELECT data FROM mods WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn delete_mod(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM mods WHERE key = ?1", [key])?;
        self.conn.execute("DELETE FROM profile_mods WHERE mod_key = ?1", [key])?;
        Ok(())
    }

    // profiles ------------------------------------------------------------------------------------------

    pub fn profiles(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM profiles ORDER BY created, name")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn active_profile(&self) -> Result<String> {
        let p: Option<String> = self.setting("activeProfile")?;
        Ok(p.filter(|p| self.profiles().map(|ps| ps.contains(p)).unwrap_or(false)).unwrap_or_else(|| DEFAULT_PROFILE.into()))
    }

    pub fn set_active_profile(&self, name: &str) -> Result<()> {
        if !self.profiles()?.iter().any(|p| p == name) {
            return Err(Error::NotFound(format!("There's no profile called {name}.")));
        }
        self.set_setting("activeProfile", &name)
    }

    /// Creates a profile, copying another's enabled mods and load order when `copy_from` is given.
    pub fn create_profile(&self, name: &str, copy_from: Option<&str>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("Give the profile a name.".into()));
        }
        if self.profiles()?.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            return Err(Error::Invalid(format!("A profile called {name} already exists.")));
        }
        self.conn.execute("INSERT INTO profiles (name, created) VALUES (?1, ?2)", params![name, crate::timefmt::now()])?;
        if let Some(src) = copy_from {
            self.conn.execute("INSERT INTO profile_mods (profile, mod_key, enabled) SELECT ?1, mod_key, enabled FROM profile_mods WHERE profile = ?2", params![name, src])?;
            self.conn.execute("INSERT INTO load_order (profile, position, folder) SELECT ?1, position, folder FROM load_order WHERE profile = ?2", params![name, src])?;
        }
        Ok(())
    }

    pub fn delete_profile(&self, name: &str) -> Result<()> {
        if name == DEFAULT_PROFILE {
            return Err(Error::Invalid("The Default profile can't be deleted.".into()));
        }
        if self.active_profile()? == name {
            return Err(Error::Invalid("Switch to another profile before deleting this one.".into()));
        }
        self.conn.execute("DELETE FROM profiles WHERE name = ?1", [name])?;
        self.conn.execute("DELETE FROM profile_mods WHERE profile = ?1", [name])?;
        self.conn.execute("DELETE FROM load_order WHERE profile = ?1", [name])?;
        Ok(())
    }

    pub fn enabled_in(&self, profile: &str) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT mod_key FROM profile_mods WHERE profile = ?1 AND enabled = 1")?;
        let rows = stmt.query_map([profile], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn set_enabled(&self, profile: &str, key: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profile_mods (profile, mod_key, enabled) VALUES (?1, ?2, ?3) ON CONFLICT(profile, mod_key) DO UPDATE SET enabled = excluded.enabled",
            params![profile, key, enabled as i64],
        )?;
        Ok(())
    }

    // load order ----------------------------------------------------------------------------------------

    pub fn load_order(&self, profile: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT folder FROM load_order WHERE profile = ?1 ORDER BY position")?;
        let rows = stmt.query_map([profile], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn set_load_order(&self, profile: &str, order: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM load_order WHERE profile = ?1", [profile])?;
        for (i, f) in order.iter().enumerate() {
            tx.execute("INSERT OR IGNORE INTO load_order (profile, position, folder) VALUES (?1, ?2, ?3)", params![profile, i as i64, f])?;
        }
        tx.commit()?;
        Ok(())
    }

    // local collections ---------------------------------------------------------------------------------

    pub fn save_collection(&self, name: &str, data: &Value) -> Result<()> {
        self.conn.execute(
            "INSERT INTO collections (name, data, updated) VALUES (?1, ?2, ?3) ON CONFLICT(name) DO UPDATE SET data = excluded.data, updated = excluded.updated",
            params![name, data.to_string(), crate::timefmt::now()],
        )?;
        Ok(())
    }

    pub fn collections(&self) -> Result<Vec<(String, Value)>> {
        let mut stmt = self.conn.prepare("SELECT name, data FROM collections ORDER BY updated DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).filter_map(|(n, d)| serde_json::from_str(&d).ok().map(|v| (n, v))).collect())
    }

    pub fn delete_collection(&self, name: &str) -> Result<()> {
        self.conn.execute("DELETE FROM collections WHERE name = ?1", [name])?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn sample_mod(key: &str, name: &str) -> InstalledMod {
    InstalledMod {
        key: key.into(),
        mod_id: key.split('-').next().and_then(|s| s.parse().ok()),
        file_id: key.split('-').nth(1).and_then(|s| s.parse().ok()),
        name: name.into(),
        file_name: format!("{name}.zip"),
        version: Some("1.0".into()),
        author: None,
        kind: ModKind::Archive,
        frameworks: vec![],
        requirements: vec![],
        md5: None,
        size: 10,
        installed_at: 0,
        uploaded_at: Some(0),
        archive_path: None,
        redmods: vec![],
        thumbnail: None,
        unplaced: vec![],
        update_available: None,
        endorsed: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_and_cache() {
        let l = Library::in_memory().unwrap();
        assert_eq!(l.setting::<String>("theme").unwrap(), None);
        l.set_setting("theme", &"night").unwrap();
        assert_eq!(l.setting::<String>("theme").unwrap().as_deref(), Some("night"));
        l.cache_put("mods:a", &json!({"x": 1}), 60).unwrap();
        assert_eq!(l.cache_get("mods:a").unwrap(), Some(json!({"x": 1})));
        l.cache_put("mods:b", &json!(2), -1).unwrap();
        assert_eq!(l.cache_get("mods:b").unwrap(), None, "expired entries are not returned");
        assert_eq!(l.cache_purge_expired().unwrap(), 1);
        assert_eq!(l.cache_forget_prefix("mods:").unwrap(), 1);
    }

    #[test]
    fn hidden_downloaded_derived() {
        let l = Library::in_memory().unwrap();
        l.set_hidden(5, true).unwrap();
        assert!(l.hidden().unwrap().contains(&5));
        l.set_hidden(5, false).unwrap();
        assert!(l.hidden().unwrap().is_empty());
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.zip");
        std::fs::write(&p, b"x").unwrap();
        l.mark_downloaded(7, 70, &p, Some("abc"), 1).unwrap();
        assert!(l.downloaded_mods().unwrap().contains(&7));
        assert_eq!(l.downloaded_file(7, 70).unwrap().unwrap().1.as_deref(), Some("abc"));
        std::fs::remove_file(&p).unwrap();
        assert!(l.downloaded_file(7, 70).unwrap().is_none(), "gone from disk means not downloaded");
        let d = Derived { kind: Some(ModKind::Script), ..Default::default() };
        l.put_derived(9, &d).unwrap();
        assert_eq!(l.derived(9).unwrap(), Some(d.clone()));
        assert_eq!(l.derived_many(&[9, 10]).unwrap().len(), 1);
    }

    #[test]
    fn mods_and_profiles() {
        let l = Library::in_memory().unwrap();
        l.upsert_mod(&sample_mod("1-1", "Beta")).unwrap();
        l.upsert_mod(&sample_mod("2-2", "alpha")).unwrap();
        assert_eq!(l.mods().unwrap().iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["alpha", "Beta"]);
        l.set_enabled(DEFAULT_PROFILE, "1-1", true).unwrap();
        l.set_load_order(DEFAULT_PROFILE, &["B".into(), "A".into()]).unwrap();
        l.create_profile("Photo mode", Some(DEFAULT_PROFILE)).unwrap();
        assert!(l.create_profile("photo MODE", None).is_err());
        assert_eq!(l.enabled_in("Photo mode").unwrap(), HashSet::from(["1-1".to_string()]));
        assert_eq!(l.load_order("Photo mode").unwrap(), vec!["B", "A"]);
        l.set_active_profile("Photo mode").unwrap();
        assert_eq!(l.active_profile().unwrap(), "Photo mode");
        assert!(l.delete_profile("Photo mode").is_err(), "can't delete the active one");
        l.set_active_profile(DEFAULT_PROFILE).unwrap();
        l.delete_profile("Photo mode").unwrap();
        assert!(l.delete_profile(DEFAULT_PROFILE).is_err());
        l.delete_mod("1-1").unwrap();
        assert!(l.enabled_in(DEFAULT_PROFILE).unwrap().is_empty());
    }

    #[test]
    fn reopening_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lib.db");
        Library::open(&p).unwrap().set_setting("x", &1).unwrap();
        assert_eq!(Library::open(&p).unwrap().setting::<i32>("x").unwrap(), Some(1));
    }
}
