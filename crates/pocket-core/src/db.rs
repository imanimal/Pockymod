//! Local SQLite store: the Nexus response cache, local mod flags, downloads, the library and deploy manifests.
//! One file, `pocket-mods.sqlite`, in the app's data folder.

use crate::error::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self { Error::Db(e.to_string()) }
}

/// Schema steps, applied in order and recorded in `user_version`. Never edit a released step; add one.
const MIGRATIONS: &[&str] = &[
    // 1: cache and local flags
    "CREATE TABLE query_cache (key TEXT PRIMARY KEY, body TEXT NOT NULL, fetched_at INTEGER NOT NULL);
     CREATE TABLE mod_cache (mod_id INTEGER PRIMARY KEY, updated_at TEXT NOT NULL, detail TEXT NOT NULL, files TEXT NOT NULL, fetched_at INTEGER NOT NULL);
     CREATE TABLE file_lists (mod_id INTEGER NOT NULL, file_id INTEGER NOT NULL, paths TEXT NOT NULL, fetched_at INTEGER NOT NULL, PRIMARY KEY (mod_id, file_id));
     CREATE TABLE mod_flags (mod_id INTEGER PRIMARY KEY, hidden INTEGER NOT NULL DEFAULT 0, ignored INTEGER NOT NULL DEFAULT 0, note TEXT);",
    // 2: downloads, library, deployments
    "CREATE TABLE downloads (id INTEGER PRIMARY KEY, mod_id INTEGER NOT NULL, file_id INTEGER NOT NULL, name TEXT NOT NULL, version TEXT NOT NULL,
        path TEXT NOT NULL, size INTEGER NOT NULL, md5 TEXT NOT NULL, downloaded_at INTEGER NOT NULL, UNIQUE (mod_id, file_id));
     CREATE TABLE installs (id INTEGER PRIMARY KEY, mod_id INTEGER NOT NULL, file_id INTEGER NOT NULL, name TEXT NOT NULL, version TEXT NOT NULL,
        mod_type TEXT NOT NULL, frameworks TEXT NOT NULL, staging TEXT NOT NULL, installed_at INTEGER NOT NULL, updated_at TEXT, UNIQUE (mod_id, file_id));
     CREATE TABLE profiles (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, active INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE profile_mods (profile_id INTEGER NOT NULL, install_id INTEGER NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, load_order INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (profile_id, install_id));
     CREATE TABLE deployed_files (target TEXT PRIMARY KEY, install_id INTEGER NOT NULL, source TEXT NOT NULL, method TEXT NOT NULL, size INTEGER NOT NULL, deployed_at INTEGER NOT NULL);
     CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     INSERT INTO profiles (name, active) VALUES ('Default', 1);",
    // 3: deploy ownership: the file a deploy replaced (backup) or the install it took a path from (prev_install)
    "ALTER TABLE deployed_files ADD COLUMN target_path TEXT NOT NULL DEFAULT '';
     ALTER TABLE deployed_files ADD COLUMN backup TEXT;
     ALTER TABLE deployed_files ADD COLUMN prev_install INTEGER;",
    // 4: where a mod came from (pocket, vortex, found), its Nexus category, the user's own group; groups; saved queue
    "ALTER TABLE installs ADD COLUMN source TEXT NOT NULL DEFAULT 'pocket';
     ALTER TABLE installs ADD COLUMN external TEXT;
     ALTER TABLE installs ADD COLUMN category TEXT;
     ALTER TABLE installs ADD COLUMN group_name TEXT;
     CREATE TABLE groups (name TEXT PRIMARY KEY, sort INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE queue_items (id INTEGER PRIMARY KEY, data TEXT NOT NULL);",
    // 5: what an install was made for (the mod needing it, or the collection), so the user's own copies win over ones added for them
    "ALTER TABLE installs ADD COLUMN origin TEXT;",
    // 6: removed mods, kept for a while so a removal (or an update) can be undone
    "CREATE TABLE trash (id INTEGER PRIMARY KEY, install_id INTEGER NOT NULL, name TEXT NOT NULL, mod_id INTEGER NOT NULL, version TEXT NOT NULL,
        source TEXT NOT NULL, reason TEXT NOT NULL, data TEXT NOT NULL, kept TEXT, size INTEGER NOT NULL DEFAULT 0, removed_at INTEGER NOT NULL);",
    // 7: the Nexus file's own name (its line: 2K or 4K, main or optional), and faster per-install lookups
    "ALTER TABLE installs ADD COLUMN file_name TEXT;
     CREATE INDEX IF NOT EXISTS deployed_by_install ON deployed_files (install_id);",
    // 8: install ids are never reused (undo, redo and Recently removed refer to mods by id), so the table gets
    // AUTOINCREMENT, starting above every id in use or in the trash
    "CREATE TABLE installs_v8 (id INTEGER PRIMARY KEY AUTOINCREMENT, mod_id INTEGER NOT NULL, file_id INTEGER NOT NULL, name TEXT NOT NULL,
        version TEXT NOT NULL, mod_type TEXT NOT NULL, frameworks TEXT NOT NULL, staging TEXT NOT NULL, installed_at INTEGER NOT NULL, updated_at TEXT,
        source TEXT NOT NULL DEFAULT 'pocket', external TEXT, category TEXT, group_name TEXT, origin TEXT, file_name TEXT, UNIQUE (mod_id, file_id));
     INSERT INTO installs_v8 (id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, updated_at, source, external, category, group_name, origin, file_name)
        SELECT id, mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at, updated_at, source, external, category, group_name, origin, file_name FROM installs;
     DROP TABLE installs;
     ALTER TABLE installs_v8 RENAME TO installs;
     DELETE FROM sqlite_sequence WHERE name IN ('installs', 'installs_v8');
     INSERT INTO sqlite_sequence (name, seq) VALUES ('installs', MAX(COALESCE((SELECT MAX(id) FROM installs), 0), COALESCE((SELECT MAX(install_id) FROM trash), 0)));",
];

pub struct Db { pub conn: Connection }

/// One copy of the database a day (before it is opened, so before any schema change), the last five kept, in
/// `backups/` beside it. Cheap insurance against a crash mid-write or a bad update.
fn backup_daily(path: &Path) {
    if !path.exists() { return; }
    let Some(dir) = path.parent().map(|d| d.join("backups")) else { return };
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let target = dir.join(format!("pocket-mods-{day}.sqlite"));
    if target.exists() || std::fs::create_dir_all(&dir).is_err() { return; }
    // the WAL may hold recent writes; a checkpoint folds them in first
    if let Ok(c) = Connection::open(path) { let _ = c.pragma_update(None, "wal_checkpoint", "TRUNCATE"); }
    if std::fs::copy(path, &target).is_err() { return; }
    let mut old: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("pocket-mods-"))).collect();
    old.sort();
    while old.len() > 5 { let _ = std::fs::remove_file(old.remove(0)); }
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
        backup_daily(path);
        Self::init(Connection::open(path)?)
    }

    pub fn memory() -> Result<Self> { Self::init(Connection::open_in_memory()?) }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        // a newer app made this file: an older one must not guess at a schema it does not know
        if version as usize > MIGRATIONS.len() {
            return Err(Error::Invalid("Your library was last opened by a newer version of Pocket Mods. Install that version again (a copy of the library from before is kept in the app's data folder, backups).".into()));
        }
        for (i, step) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(step)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(Db { conn })
    }

    // ── response cache ──

    /// A cached body if it is younger than `ttl_secs`.
    pub fn cache_get(&self, key: &str, ttl_secs: i64, now: i64) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT body FROM query_cache WHERE key = ?1 AND fetched_at > ?2", params![key, now - ttl_secs], |r| r.get(0)).optional()?)
    }

    pub fn cache_put(&self, key: &str, body: &str, now: i64) -> Result<()> {
        self.conn.execute("INSERT INTO query_cache (key, body, fetched_at) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET body = ?2, fetched_at = ?3", params![key, body, now])?;
        Ok(())
    }

    pub fn cache_prune(&self, older_than: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM query_cache WHERE fetched_at < ?1", params![older_than])?)
    }

    /// A mod's detail and files if cached for this `updated_at`. A newer `updatedAt` on a card makes it stale.
    pub fn mod_get(&self, mod_id: u32, updated_at: Option<&str>) -> Result<Option<(String, String)>> {
        let row: Option<(String, String, String)> = self.conn.query_row("SELECT updated_at, detail, files FROM mod_cache WHERE mod_id = ?1", params![mod_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        Ok(row.filter(|(u, _, _)| updated_at.is_none_or(|want| want == u)).map(|(_, d, f)| (d, f)))
    }

    pub fn mod_put(&self, mod_id: u32, updated_at: &str, detail: &str, files: &str, now: i64) -> Result<()> {
        self.conn.execute("INSERT OR REPLACE INTO mod_cache (mod_id, updated_at, detail, files, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![mod_id, updated_at, detail, files, now])?;
        Ok(())
    }

    pub fn file_list_get(&self, mod_id: u32, file_id: u32) -> Result<Option<Vec<String>>> {
        let s: Option<String> = self.conn.query_row("SELECT paths FROM file_lists WHERE mod_id = ?1 AND file_id = ?2", params![mod_id, file_id], |r| r.get(0)).optional()?;
        Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn file_list_put(&self, mod_id: u32, file_id: u32, paths: &[String], now: i64) -> Result<()> {
        self.conn.execute("INSERT OR REPLACE INTO file_lists (mod_id, file_id, paths, fetched_at) VALUES (?1, ?2, ?3, ?4)", params![mod_id, file_id, serde_json::to_string(paths).unwrap_or_default(), now])?;
        Ok(())
    }

    /// Every cached file list for a mod (any of its files), for classification.
    pub fn file_lists_for(&self, mod_id: u32) -> Result<Vec<String>> {
        let mut st = self.conn.prepare("SELECT paths FROM file_lists WHERE mod_id = ?1")?;
        let rows = st.query_map(params![mod_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows { out.extend(serde_json::from_str::<Vec<String>>(&r?).unwrap_or_default()); }
        Ok(out)
    }

    // ── local flags ──

    pub fn set_flag(&self, mod_id: u32, flag: Flag, on: bool) -> Result<()> {
        let col = match flag { Flag::Hidden => "hidden", Flag::Ignored => "ignored" };
        self.conn.execute(&format!("INSERT INTO mod_flags (mod_id, {col}) VALUES (?1, ?2) ON CONFLICT(mod_id) DO UPDATE SET {col} = ?2"), params![mod_id, on as i32])?;
        Ok(())
    }

    pub fn flags(&self) -> Result<Flags> {
        let mut st = self.conn.prepare("SELECT mod_id, hidden, ignored FROM mod_flags WHERE hidden = 1 OR ignored = 1")?;
        let mut f = Flags::default();
        for r in st.query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?)))? {
            let (id, h, i) = r?;
            if h { f.hidden.insert(id); }
            if i { f.ignored.insert(id); }
        }
        Ok(f)
    }

    pub fn downloaded_mod_ids(&self) -> Result<std::collections::BTreeSet<u32>> {
        let mut st = self.conn.prepare("SELECT DISTINCT mod_id FROM downloads")?;
        let ids = st.query_map([], |r| r.get::<_, u32>(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(ids)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2", params![key, value])?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flag { Hidden, Ignored }

#[derive(Debug, Default, Clone)]
pub struct Flags { pub hidden: std::collections::BTreeSet<u32>, pub ignored: std::collections::BTreeSet<u32> }

pub fn now() -> i64 { chrono::Utc::now().timestamp() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.sqlite");
        { let db = Db::open(&p).unwrap(); db.set_setting("a", "1").unwrap(); }
        let db = Db::open(&p).unwrap();
        assert_eq!(db.setting("a").unwrap().as_deref(), Some("1"));
        let v: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
    }

    #[test]
    fn cache_ttl() {
        let db = Db::memory().unwrap();
        db.cache_put("k", "body", 1000).unwrap();
        assert_eq!(db.cache_get("k", 600, 1500).unwrap().as_deref(), Some("body"));
        assert!(db.cache_get("k", 600, 1700).unwrap().is_none());
        assert_eq!(db.cache_prune(1001).unwrap(), 1);
    }

    #[test]
    fn mod_cache_goes_stale_on_newer_update() {
        let db = Db::memory().unwrap();
        db.mod_put(7, "2026-01-01T00:00:00Z", "{}", "[]", 1).unwrap();
        assert!(db.mod_get(7, Some("2026-01-01T00:00:00Z")).unwrap().is_some());
        assert!(db.mod_get(7, Some("2026-02-01T00:00:00Z")).unwrap().is_none());
        assert!(db.mod_get(7, None).unwrap().is_some());
    }

    #[test]
    fn flags() {
        let db = Db::memory().unwrap();
        db.set_flag(1, Flag::Hidden, true).unwrap();
        db.set_flag(2, Flag::Ignored, true).unwrap();
        db.set_flag(1, Flag::Ignored, true).unwrap();
        db.set_flag(2, Flag::Ignored, false).unwrap();
        let f = db.flags().unwrap();
        assert!(f.hidden.contains(&1) && f.ignored.contains(&1) && !f.ignored.contains(&2));
    }
}

#[cfg(test)]
mod id_tests {
    #[test]
    fn install_ids_are_never_reused() {
        let db = super::Db::memory().unwrap();
        let ins = |m: i64| { db.conn.execute("INSERT INTO installs (mod_id, file_id, name, version, mod_type, frameworks, staging, installed_at) VALUES (?1, 1, 'x', '1', 'archive', '[]', '', 0)", [m]).unwrap(); db.conn.last_insert_rowid() };
        let a = ins(1);
        db.conn.execute("DELETE FROM installs WHERE id = ?1", [a]).unwrap();
        let b = ins(2);
        assert!(b > a, "{b} reuses {a}");
    }
}
