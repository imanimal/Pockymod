//! The application service: everything the UI asks for, in one place, with no UI in it.
//!
//! The Tauri shell owns one `App`, forwards commands to it, and implements [`Shell`] so the queue driver can
//! open a Nexus page inside the app and push events to the window.

use crate::classify::{self, Compat, ModKind};
use crate::collections::{self, Collection, CollectionCard, PreCheck};
use crate::deploy::{self, DeployOptions, Manifest, Ownership, PlannedFile, Store};
use crate::frameworks;
use crate::game::{self, GameInstall, Launcher};
use crate::health::{self, ConflictRecord, HealthInput, HealthReport};
use crate::layout;
use crate::library::{InstalledMod, Library, UpdateInfo};
use crate::nexus::ratelimit::{Level, RateLimit};
use crate::nexus::{self, Account, BrowseQuery, Category, Client, FacetCount, ModDetail, ModPage};
use crate::nxm::NxmLink;
use crate::queue::{ItemState, NewItem, Queue, QueueItem, QueueSummary, Step};
use crate::redmod::{self, DeployOutcome, RedmodInfo};
use crate::{secrets, timefmt, Error, Result, GAME_DOMAIN};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// TTLs for cached Nexus responses (engineering rule: cache keyed on filter, sort and offset).
pub const TTL_BROWSE: i64 = 10 * 60;
pub const TTL_DETAIL: i64 = 30 * 60;
pub const TTL_CATEGORIES: i64 = 24 * 3600;
pub const TTL_TRACKED: i64 = 10 * 60;
pub const TTL_COLLECTIONS: i64 = 15 * 60;

/// What the core needs from the window around it.
pub trait Shell: Send + Sync + 'static {
    /// Show a Nexus page inside the app (never an external browser).
    fn open_nexus_page(&self, url: &str, title: &str);
    /// Push an event to the UI.
    fn emit(&self, event: &str, payload: Value);
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Experiments {
    /// Publishing collections to Nexus: unconfirmed for third-party apps.
    pub publish_collections: bool,
    /// Read-only mod.io browsing.
    pub modio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub game_root: Option<PathBuf>,
    pub store_root: Option<PathBuf>,
    /// Copy when hard links are impossible. Off by default; turning it on shows a warning.
    pub allow_copy: bool,
    /// When two mods write the same file, the one installed (or enabled) last wins.
    pub later_wins: bool,
    /// "system", "cream" or "night"
    pub theme: String,
    pub motion: bool,
    pub include_adult: bool,
    pub redmod_auto_deploy: bool,
    pub check_app_updates: bool,
    pub modio_game_id: Option<u64>,
    pub experiments: Experiments,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game_root: None,
            store_root: None,
            allow_copy: false,
            later_wins: true,
            theme: "system".into(),
            motion: true,
            include_adult: false,
            redmod_auto_deploy: true,
            check_app_updates: true,
            modio_game_id: None,
            experiments: Experiments::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateStatus {
    #[serde(flatten)]
    pub rate: RateLimit,
    pub level: Level,
    pub remaining: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub has_key: bool,
    pub account: Option<Account>,
    pub game: Option<GameInstall>,
    pub store_root: Option<PathBuf>,
    pub rate: RateStatus,
    pub queue: QueueSummary,
    pub updates: usize,
    pub profile: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRow {
    #[serde(flatten)]
    pub m: InstalledMod,
    pub enabled: bool,
    pub deployed: bool,
    pub drift: usize,
    pub files: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstallMeta {
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    pub name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub uploaded_at: Option<i64>,
    pub md5: Option<String>,
    pub thumbnail: Option<String>,
    pub requirements: Vec<u64>,
    pub frameworks: Vec<frameworks::Framework>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub key: String,
    pub name: String,
    pub kind: ModKind,
    pub placed: usize,
    pub skipped_docs: usize,
    pub unplaced: Vec<String>,
    pub conflicts: Vec<deploy::FileConflict>,
    pub copied: usize,
    pub replaced: Option<String>,
    pub redmods: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedmodRow {
    pub folder: String,
    pub name: String,
    pub version: Option<String>,
    /// The library mod that installed it, if any.
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadOrderView {
    pub rows: Vec<RedmodRow>,
    pub tool: bool,
    pub dirty: bool,
    pub last: Option<DeployOutcome>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub items: Vec<QueueItem>,
    pub summary: QueueSummary,
    pub premium: bool,
    pub paused: bool,
}

pub struct App {
    pub data_dir: PathBuf,
    lib: Mutex<Library>,
    rate: Arc<Mutex<RateLimit>>,
    client: RwLock<Option<Client>>,
    account: RwLock<Option<Account>>,
    queue: Mutex<Queue>,
    settings: RwLock<Settings>,
    game: RwLock<Option<GameInstall>>,
    /// The queue item whose page is open, so it isn't opened twice.
    asked: Mutex<Option<u64>>,
    /// nxm keys waiting for their item to run, by item id. Never persisted.
    nxm_keys: Mutex<BTreeMap<u64, (String, i64)>>,
    /// Bounds REDmod deploys and library writes to one at a time.
    install_lock: tokio::sync::Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl App {
    /// Opens (or creates) the app's data in `data_dir`.
    pub fn open(data_dir: &Path) -> Result<App> {
        let lib = Library::open(&data_dir.join("library.db"))?;
        let settings: Settings = lib.setting("settings")?.unwrap_or_default();
        let mut queue: Queue = lib.setting("queue")?.unwrap_or_else(|| Queue::new(false));
        // anything that was mid-flight when the app closed starts over
        for i in queue.items.iter_mut() {
            if i.state.is_active() || i.state == ItemState::NeedsClick {
                i.state = ItemState::Waiting;
                i.received = 0;
            }
        }
        let app = App {
            data_dir: data_dir.to_path_buf(),
            lib: Mutex::new(lib),
            rate: Arc::new(Mutex::new(RateLimit::default())),
            client: RwLock::new(None),
            account: RwLock::new(None),
            queue: Mutex::new(queue),
            settings: RwLock::new(settings),
            game: RwLock::new(None),
            asked: Mutex::new(None),
            nxm_keys: Mutex::new(BTreeMap::new()),
            install_lock: tokio::sync::Mutex::new(()),
        };
        if let Ok(Some(key)) = secrets::get(secrets::NEXUS_KEY_TARGET) {
            if let Ok(c) = Client::new(&key) {
                *app.client.write().unwrap() = Some(c.with_rate(app.rate.clone()));
            }
        }
        if let Some(acct) = lock(&app.lib).setting::<Account>("account")? {
            lock(&app.queue).premium = acct.is_premium;
            *app.account.write().unwrap() = Some(acct);
        }
        app.refresh_game();
        Ok(app)
    }

    // settings and status ---------------------------------------------------------------------------------

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn save_settings(&self, s: Settings) -> Result<Settings> {
        let game_changed = s.game_root != self.settings().game_root;
        lock(&self.lib).set_setting("settings", &s)?;
        *self.settings.write().unwrap() = s.clone();
        if game_changed {
            self.refresh_game();
        }
        Ok(s)
    }

    /// Re-reads the game install: the folder in settings, else the first one found (and remembers it).
    pub fn refresh_game(&self) -> Option<GameInstall> {
        let s = self.settings();
        let g = match &s.game_root {
            Some(root) => game::inspect(root, Launcher::Manual).ok().map(|mut g| {
                if let Some(found) = game::detect_all().into_iter().find(|f| &f.root == root) {
                    g.launcher = found.launcher;
                }
                g
            }),
            None => {
                let found = game::detect_all().into_iter().next();
                if let Some(f) = &found {
                    let mut s2 = s.clone();
                    s2.game_root = Some(f.root.clone());
                    let _ = lock(&self.lib).set_setting("settings", &s2);
                    *self.settings.write().unwrap() = s2;
                }
                found
            }
        };
        *self.game.write().unwrap() = g.clone();
        g
    }

    pub fn game(&self) -> Option<GameInstall> {
        self.game.read().unwrap().clone()
    }

    pub fn detect_games(&self) -> Vec<GameInstall> {
        game::detect_all()
    }

    /// Points the app at a game folder. Refuses folders that aren't Cyberpunk 2077.
    pub fn set_game_root(&self, root: &Path) -> Result<GameInstall> {
        let g = game::inspect(root, Launcher::Manual)?;
        let mut s = self.settings();
        s.game_root = Some(root.to_path_buf());
        self.save_settings(s)?;
        Ok(self.refresh_game().unwrap_or(g))
    }

    fn store(&self) -> Result<Store> {
        let s = self.settings();
        if let Some(r) = s.store_root {
            return Ok(Store::new(r));
        }
        let g = self.game().ok_or_else(|| Error::NotFound("Pick the game folder in Settings first.".into()))?;
        Ok(Store::new(game::default_store(&g.root)))
    }

    fn game_root(&self) -> Result<PathBuf> {
        self.game().map(|g| g.root).ok_or_else(|| Error::NotFound("Pick the game folder in Settings first.".into()))
    }

    pub fn rate_status(&self) -> RateStatus {
        let rate = lock(&self.rate).clone();
        RateStatus { level: rate.level(timefmt::now()), remaining: rate.remaining_now(), rate }
    }

    pub fn status(&self) -> Result<AppStatus> {
        let lib = lock(&self.lib);
        let updates = lib.mods()?.iter().filter(|m| m.update_available.is_some()).count();
        let profile = lib.active_profile()?;
        drop(lib);
        Ok(AppStatus {
            has_key: self.client.read().unwrap().is_some(),
            account: self.account.read().unwrap().clone(),
            game: self.game(),
            store_root: self.store().ok().map(|s| s.root),
            rate: self.rate_status(),
            queue: lock(&self.queue).summary(),
            updates,
            profile,
            version: crate::APP_VERSION.into(),
        })
    }

    // account ---------------------------------------------------------------------------------------------

    fn client(&self) -> Result<Client> {
        self.client.read().unwrap().clone().ok_or(Error::NoKey)
    }

    /// Checks the key with Nexus first, then stores it in Credential Manager.
    pub async fn set_api_key(&self, key: &str) -> Result<Account> {
        let c = Client::new(key)?.with_rate(self.rate.clone());
        let acct = c.validate().await?;
        secrets::set(secrets::NEXUS_KEY_TARGET, key)?;
        *self.client.write().unwrap() = Some(c);
        self.remember_account(&acct)?;
        Ok(acct)
    }

    pub fn clear_api_key(&self) -> Result<()> {
        secrets::delete(secrets::NEXUS_KEY_TARGET)?;
        *self.client.write().unwrap() = None;
        *self.account.write().unwrap() = None;
        lock(&self.lib).set_setting("account", &Value::Null)?;
        lock(&self.queue).premium = false;
        Ok(())
    }

    fn remember_account(&self, acct: &Account) -> Result<()> {
        lock(&self.lib).set_setting("account", acct)?;
        lock(&self.queue).premium = acct.is_premium;
        *self.account.write().unwrap() = Some(acct.clone());
        Ok(())
    }

    pub async fn refresh_account(&self) -> Result<Account> {
        let acct = self.client()?.validate().await?;
        self.remember_account(&acct)?;
        Ok(acct)
    }

    pub async fn probe(&self) -> Result<Value> {
        let report = self.client()?.probe().await?;
        let path = self.data_dir.join("nexus-probe.json");
        let _ = std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap_or_default());
        Ok(report)
    }

    // browse ----------------------------------------------------------------------------------------------

    pub async fn browse(&self, q: &BrowseQuery) -> Result<ModPage> {
        let mut q = q.clone();
        q.include_adult = q.include_adult || self.settings().include_adult;
        let key = q.remote_key();
        let cached = lock(&self.lib).cache_get(&key)?.and_then(|v| serde_json::from_value::<ModPage>(v).ok());
        let mut page = match cached {
            Some(mut p) => {
                p.cached = true;
                p
            }
            None => {
                let p = self.client()?.mods(&q).await?;
                lock(&self.lib).cache_put(&key, &serde_json::to_value(&p)?, TTL_BROWSE)?;
                p
            }
        };
        self.enrich_page(&mut page, &q)?;
        Ok(page)
    }

    /// Adds the local facts to a page, counts the local facets, and applies the local filters.
    pub fn enrich_page(&self, page: &mut ModPage, q: &BrowseQuery) -> Result<()> {
        let lib = lock(&self.lib);
        let hidden = lib.hidden()?;
        let downloaded = lib.downloaded_mods()?;
        let installed: HashSet<u64> = lib.mods()?.iter().filter_map(|m| m.mod_id).collect();
        let ids: Vec<u64> = page.mods.iter().map(|m| m.mod_id).collect();
        let index = lib.derived_many(&ids)?;
        let build_time = self.game().and_then(|g| g.build_time);
        for card in page.mods.iter_mut() {
            card.derived = match index.get(&card.mod_id) {
                Some(d) => d.clone(),
                None => {
                    let d = classify::derive(card.mod_id, &card.name, &card.summary, None, None);
                    lib.put_derived(card.mod_id, &d)?;
                    d
                }
            };
            card.compat = classify::compat(card.updated_at, build_time, &card.derived);
            card.hidden = hidden.contains(&card.mod_id);
            card.downloaded = downloaded.contains(&card.mod_id);
            card.installed = installed.contains(&card.mod_id);
        }
        drop(lib);
        // local facet counts, over this page before local filters
        let mut fw: BTreeMap<String, u64> = BTreeMap::new();
        let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
        for c in &page.mods {
            for f in &c.derived.frameworks {
                *fw.entry(serde_json::to_value(f)?.as_str().unwrap_or_default().to_string()).or_default() += 1;
            }
            if let Some(k) = c.derived.kind {
                *kinds.entry(serde_json::to_value(k)?.as_str().unwrap_or_default().to_string()).or_default() += 1;
            }
        }
        let facets = &mut page.facets;
        facets.frameworks = fw.into_iter().map(|(value, count)| FacetCount { value, count }).collect();
        facets.kinds = kinds.into_iter().map(|(value, count)| FacetCount { value, count }).collect();
        facets.compatible = page.mods.iter().filter(|c| matches!(c.compat, Compat::Updated | Compat::Likely)).count() as u64;
        facets.downloaded = page.mods.iter().filter(|c| c.downloaded).count() as u64;
        facets.not_downloaded = page.mods.len() as u64 - facets.downloaded;
        facets.hidden = page.mods.iter().filter(|c| c.hidden).count() as u64;

        let before = page.mods.len();
        page.mods.retain(|c| {
            (q.show_hidden || !c.hidden)
                && q.frameworks.iter().all(|f| c.derived.frameworks.contains(f))
                && (q.kinds.is_empty() || c.derived.kind.is_some_and(|k| q.kinds.contains(&k)))
                && (!q.compatible_only || matches!(c.compat, Compat::Updated | Compat::Likely))
                && q.downloaded.is_none_or(|d| c.downloaded == d)
        });
        page.filtered_out = (before - page.mods.len()) as u32;
        Ok(())
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        if let Some(v) = lock(&self.lib).cache_get("categories")? {
            if let Ok(c) = serde_json::from_value(v) {
                return Ok(c);
            }
        }
        let c = self.client()?.categories().await?;
        lock(&self.lib).cache_put("categories", &serde_json::to_value(&c)?, TTL_CATEGORIES)?;
        Ok(c)
    }

    pub fn set_hidden(&self, mod_id: u64, hidden: bool) -> Result<()> {
        lock(&self.lib).set_hidden(mod_id, hidden)
    }

    /// The detail view: description, files, requirements, and the archive's file list when Nexus has one.
    /// Requirement and preview failures don't fail the view.
    pub async fn mod_detail(&self, mod_id: u64) -> Result<ModDetail> {
        let key = format!("detail:{mod_id}");
        let cached = lock(&self.lib).cache_get(&key)?.and_then(|v| serde_json::from_value::<ModDetail>(v).ok());
        let mut d = match cached {
            Some(d) => d,
            None => {
                let c = self.client()?;
                let raw = c.mod_v1(mod_id).await?;
                let mut card = nexus::queries::card_from_v1(&raw).ok_or_else(|| Error::NotFound(format!("Nexus has no mod {mod_id}.")))?;
                let files = c.files(mod_id).await?;
                let requirements = c.requirements(mod_id).await.unwrap_or_else(|e| {
                    log::warn!("requirements for {mod_id} unavailable: {e}");
                    Vec::new()
                });
                let recommended = nexus::queries::recommended_file(&files);
                let preview_url = recommended.and_then(|id| files.iter().find(|f| f.file_id == id)).and_then(|f| f.content_preview_url.clone());
                let preview_files = match preview_url {
                    Some(u) => c.content_preview(&u).await.unwrap_or_default(),
                    None => Vec::new(),
                };
                let description = raw["description"].as_str().unwrap_or_default().to_string();
                card.size_bytes = recommended.and_then(|id| files.iter().find(|f| f.file_id == id)).map(|f| f.size_bytes);
                let req_ids: Vec<u64> = requirements.iter().filter_map(|r| r.mod_id).collect();
                card.derived = classify::derive(mod_id, &card.name, &card.summary, Some(&req_ids), (!preview_files.is_empty()).then_some(preview_files.as_slice()));
                let d = ModDetail {
                    manual_only: nexus::queries::looks_manual_only(&files, &description),
                    description,
                    recommended_file_id: recommended,
                    endorse_status: raw["endorsement"]["endorse_status"].as_str().map(String::from),
                    tracked: None,
                    files,
                    requirements,
                    preview_files,
                    card,
                };
                let lib = lock(&self.lib);
                lib.put_derived(mod_id, &d.card.derived)?;
                lib.cache_put(&key, &serde_json::to_value(&d)?, TTL_DETAIL)?;
                d
            }
        };
        let lib = lock(&self.lib);
        d.card.hidden = lib.hidden()?.contains(&mod_id);
        d.card.downloaded = lib.downloaded_mods()?.contains(&mod_id);
        d.card.installed = lib.mods()?.iter().any(|m| m.mod_id == Some(mod_id));
        d.card.compat = classify::compat(d.card.updated_at, self.game().and_then(|g| g.build_time), &d.card.derived);
        let tracked: Option<Vec<u64>> = lib.cache_get("tracked")?.and_then(|v| serde_json::from_value(v).ok());
        drop(lib);
        d.tracked = tracked.map(|t| t.contains(&mod_id));
        Ok(d)
    }

    // account features ------------------------------------------------------------------------------------

    pub async fn endorse(&self, mod_id: u64, endorse: bool) -> Result<()> {
        let version = self.mod_detail(mod_id).await.ok().and_then(|d| d.card.version).unwrap_or_default();
        self.client()?.endorse(mod_id, &version, endorse).await?;
        let lib = lock(&self.lib);
        lib.cache_forget_prefix(&format!("detail:{mod_id}"))?;
        for mut m in lib.mods()?.into_iter().filter(|m| m.mod_id == Some(mod_id)) {
            m.endorsed = Some(endorse);
            lib.upsert_mod(&m)?;
        }
        Ok(())
    }

    pub async fn tracked(&self) -> Result<Vec<u64>> {
        if let Some(v) = lock(&self.lib).cache_get("tracked")? {
            if let Ok(t) = serde_json::from_value(v) {
                return Ok(t);
            }
        }
        let t = self.client()?.tracked().await?;
        lock(&self.lib).cache_put("tracked", &json!(t), TTL_TRACKED)?;
        Ok(t)
    }

    pub async fn set_tracked(&self, mod_id: u64, track: bool) -> Result<()> {
        self.client()?.set_tracked(mod_id, track).await?;
        lock(&self.lib).cache_forget_prefix("tracked")?;
        Ok(())
    }

    /// Update check: one `updated` call for the month, then files only for the mods that changed.
    pub async fn check_updates(&self) -> Result<usize> {
        let c = self.client()?;
        let installed: Vec<InstalledMod> = lock(&self.lib).mods()?.into_iter().filter(|m| m.mod_id.is_some()).collect();
        let changed = c.updated("1m").await?;
        let mut found = 0;
        for mut m in installed {
            let id = m.mod_id.unwrap();
            let Some(u) = changed.iter().find(|u| u.mod_id == id) else { continue };
            if u.latest_file_update <= m.uploaded_at.unwrap_or(0) {
                continue;
            }
            let files = c.files(id).await?;
            let newest = nexus::queries::recommended_file(&files).and_then(|fid| files.iter().find(|f| f.file_id == fid));
            m.update_available = newest.filter(|f| Some(f.file_id) != m.file_id && f.uploaded_at > m.uploaded_at.unwrap_or(0)).map(|f| UpdateInfo {
                file_id: f.file_id,
                version: f.version.clone(),
                uploaded_at: f.uploaded_at,
                file_name: f.file_name.clone(),
                size_bytes: f.size_bytes,
            });
            if m.update_available.is_some() {
                found += 1;
            }
            lock(&self.lib).upsert_mod(&m)?;
        }
        lock(&self.lib).set_setting("lastUpdateCheck", &timefmt::now())?;
        Ok(found)
    }

    // queue -----------------------------------------------------------------------------------------------

    fn save_queue(&self, q: &Queue) {
        let mut keep = q.clone();
        keep.items.retain(|i| i.state != ItemState::Done);
        let _ = lock(&self.lib).set_setting("queue", &keep);
    }

    pub fn queue_view(&self) -> QueueView {
        let q = lock(&self.queue);
        QueueView { items: q.items.clone(), summary: q.summary(), premium: q.premium, paused: q.paused }
    }

    fn with_queue<R>(&self, f: impl FnOnce(&mut Queue) -> R) -> R {
        let mut q = lock(&self.queue);
        let r = f(&mut q);
        self.save_queue(&q);
        r
    }

    /// Queues a mod (its recommended file unless one is given), plus the frameworks it needs that aren't
    /// installed or queued. Returns the queued item ids, dependencies first.
    pub async fn queue_mod(&self, mod_id: u64, file_id: Option<u64>, origin: &str) -> Result<Vec<u64>> {
        let d = self.mod_detail(mod_id).await?;
        let file_id = file_id.or(d.recommended_file_id).ok_or_else(|| Error::NotFound(format!("{} has no file to install.", d.card.name)))?;
        let file = d.files.iter().find(|f| f.file_id == file_id);
        let mut ids = Vec::new();
        // dependencies first: frameworks this mod needs, and their own needs
        let installed_fw: HashSet<frameworks::Framework> = self
            .game()
            .map(|g| frameworks::detect(&g.root).into_iter().filter(|f| f.installed).map(|f| f.id).collect())
            .unwrap_or_default();
        let in_lib: HashSet<u64> = lock(&self.lib).mods()?.iter().filter_map(|m| m.mod_id).collect();
        let wanted: Vec<frameworks::Framework> = d.card.derived.frameworks.iter().copied().collect();
        for fw in frameworks::closure(&wanted) {
            let info = frameworks::info(fw);
            if installed_fw.contains(&fw) || in_lib.contains(&info.nexus_mod_id) || info.nexus_mod_id == mod_id {
                continue;
            }
            if let Ok(fd) = self.mod_detail(info.nexus_mod_id).await {
                if let Some(fid) = fd.recommended_file_id {
                    let f = fd.files.iter().find(|f| f.file_id == fid);
                    ids.push(self.with_queue(|q| {
                        q.enqueue(
                            NewItem {
                                mod_id: info.nexus_mod_id,
                                file_id: fid,
                                mod_name: fd.card.name.clone(),
                                file_name: f.map(|f| f.file_name.clone()).unwrap_or_default(),
                                version: f.and_then(|f| f.version.clone()),
                                size_bytes: f.map(|f| f.size_bytes).unwrap_or(0),
                                manual_only: fd.manual_only,
                                origin: "dependency".into(),
                            },
                            timefmt::now(),
                        )
                    }));
                }
            }
        }
        ids.push(self.with_queue(|q| {
            q.enqueue(
                NewItem {
                    mod_id,
                    file_id,
                    mod_name: d.card.name.clone(),
                    file_name: file.map(|f| f.file_name.clone()).unwrap_or_default(),
                    version: file.and_then(|f| f.version.clone()),
                    size_bytes: file.map(|f| f.size_bytes).unwrap_or(0),
                    manual_only: d.manual_only,
                    origin: origin.into(),
                },
                timefmt::now(),
            )
        }));
        Ok(ids)
    }

    pub fn queue_items(&self, items: Vec<NewItem>) -> Vec<u64> {
        self.with_queue(|q| items.into_iter().map(|i| q.enqueue(i, timefmt::now())).collect())
    }

    pub fn queue_retry(&self, id: u64) {
        self.with_queue(|q| q.retry(id));
        let mut asked = lock(&self.asked);
        if *asked == Some(id) {
            *asked = None;
        }
    }
    pub fn queue_skip(&self, id: u64) {
        self.with_queue(|q| q.skip(id));
        *lock(&self.asked) = None;
    }
    pub fn queue_remove(&self, id: u64) {
        self.with_queue(|q| q.remove(id));
    }
    pub fn queue_clear_finished(&self) {
        self.with_queue(|q| q.clear_finished());
    }
    pub fn queue_pause(&self, paused: bool) {
        self.with_queue(|q| q.paused = paused);
    }

    /// The Nexus page where a free account clicks "Mod Manager Download" for this file.
    /// UNVERIFIED: `nmm=1` opens the mod-manager download step directly, as other managers' links do.
    pub fn file_page_url(mod_id: u64, file_id: u64) -> String {
        format!("https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}?tab=files&file_id={file_id}&nmm=1")
    }

    pub fn mod_page_url(mod_id: u64) -> String {
        format!("https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}")
    }

    /// Moves the queue along: starts the next Premium download, or opens the next page for a click.
    pub fn pump(self: &Arc<Self>, shell: &Arc<dyn Shell>) {
        let step = self.with_queue(|q| q.next_step());
        match step {
            Step::Resolve(id) => {
                let app = self.clone();
                let sh = shell.clone();
                tokio::spawn(async move { app.run_item(id, sh).await });
            }
            Step::AskClick(id) => {
                let mut asked = lock(&self.asked);
                if *asked != Some(id) {
                    *asked = Some(id);
                    drop(asked);
                    if let Some(item) = lock(&self.queue).get(id).cloned() {
                        shell.open_nexus_page(&App::file_page_url(item.mod_id, item.file_id), &format!("{}: click Mod Manager Download", item.mod_name));
                    }
                }
            }
            Step::Busy | Step::Idle => {}
        }
        shell.emit("queue", serde_json::to_value(self.queue_view()).unwrap_or_default());
    }

    /// An nxm:// link reached the app (from the in-app page, or from the browser via the handler).
    pub async fn handle_nxm(self: &Arc<Self>, raw: &str, shell: Arc<dyn Shell>) -> Result<Value> {
        let link = NxmLink::parse(raw)?;
        if !link.game().eq_ignore_ascii_case(GAME_DOMAIN) {
            return Err(Error::Invalid(format!("That link is for {}, not Cyberpunk 2077.", link.game())));
        }
        match link {
            NxmLink::ModFile { mod_id, file_id, key, expires, .. } => {
                if let (Some(k), Some(e)) = (key.clone(), expires) {
                    if e <= timefmt::now() {
                        return Err(Error::Invalid("That download link has expired. Click Mod Manager Download again.".into()));
                    }
                    let id = match self.with_queue(|q| q.on_nxm(mod_id, file_id)) {
                        Some(id) => id,
                        None => {
                            // a click on a page the queue didn't open: queue it as its own item
                            let ids = self.queue_mod(mod_id, Some(file_id), "browse").await?;
                            let id = *ids.last().unwrap();
                            self.with_queue(|q| q.on_nxm(mod_id, file_id));
                            id
                        }
                    };
                    lock(&self.nxm_keys).insert(id, (k, e));
                    *lock(&self.asked) = None;
                    let app = self.clone();
                    let sh = shell.clone();
                    tokio::spawn(async move { app.run_item(id, sh).await });
                    Ok(json!({ "type": "modFile", "itemId": id }))
                } else {
                    // no key: only Premium can fetch it; queue it normally
                    let ids = self.queue_mod(mod_id, Some(file_id), "browse").await?;
                    self.pump(&shell);
                    Ok(json!({ "type": "modFile", "itemId": ids.last() }))
                }
            }
            NxmLink::Collection { slug, revision, .. } => {
                shell.emit("collection", json!({ "slug": slug, "revision": revision }));
                Ok(json!({ "type": "collection", "slug": slug, "revision": revision }))
            }
        }
    }

    /// Resolves, downloads and installs one queue item, then moves the queue on.
    pub async fn run_item(self: Arc<Self>, id: u64, shell: Arc<dyn Shell>) {
        let result = self.clone().run_item_inner(id, shell.clone()).await;
        match result {
            Ok(report) => {
                self.with_queue(|q| q.set_state(id, ItemState::Done));
                shell.emit("installed", serde_json::to_value(&report).unwrap_or_default());
            }
            Err(e) => {
                self.with_queue(|q| q.fail(id, e.to_string()));
                shell.emit("error", json!({ "message": e.to_string(), "itemId": id }));
            }
        }
        self.pump(&shell);
    }

    async fn run_item_inner(self: Arc<Self>, id: u64, shell: Arc<dyn Shell>) -> Result<InstallReport> {
        let item = lock(&self.queue).get(id).cloned().ok_or_else(|| Error::NotFound("That queue item is gone.".into()))?;
        let store = self.store()?;
        // reuse an archive downloaded before
        let existing = lock(&self.lib).downloaded_file(item.mod_id, item.file_id)?;
        let (path, md5, size) = match existing {
            Some((p, md5)) => {
                let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                (p, md5, size)
            }
            None => {
                let nxm = lock(&self.nxm_keys).remove(&id);
                let c = self.client()?;
                let links = c.download_links(item.mod_id, item.file_id, nxm.as_ref().map(|(k, e)| (k.as_str(), *e))).await?;
                let url = links.first().ok_or_else(|| Error::BadResponse { what: "download_link".into(), detail: "no download URLs".into() })?;
                let name = if item.file_name.is_empty() { crate::download::file_name_from_url(url, "download.zip") } else { item.file_name.clone() };
                let mut last = (std::time::Instant::now(), 0u64);
                let mut last_emit = std::time::Instant::now();
                let dl = crate::download::download(c.http(), url, &store.downloads(), &name, |got, total| {
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last.0).as_secs_f64();
                    if dt >= 0.25 || total == Some(got) {
                        let delta = got.saturating_sub(last.1);
                        lock(&self.queue).progress(id, got, total, delta, dt);
                        last = (now, got);
                    }
                    if now.duration_since(last_emit).as_millis() >= 400 {
                        last_emit = now;
                        shell.emit("queue", serde_json::to_value(self.queue_view()).unwrap_or_default());
                    }
                })
                .await?;
                lock(&self.lib).mark_downloaded(item.mod_id, item.file_id, &dl.path, Some(&dl.md5), dl.size)?;
                (dl.path, Some(dl.md5), dl.size)
            }
        };
        self.with_queue(|q| q.set_state(id, ItemState::Installing));
        shell.emit("queue", serde_json::to_value(self.queue_view()).unwrap_or_default());
        let detail = self.mod_detail(item.mod_id).await.ok();
        let file = detail.as_ref().and_then(|d| d.files.iter().find(|f| f.file_id == item.file_id));
        let meta = InstallMeta {
            mod_id: Some(item.mod_id),
            file_id: Some(item.file_id),
            name: detail.as_ref().map(|d| d.card.name.clone()).unwrap_or(item.mod_name.clone()),
            file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            version: file.and_then(|f| f.version.clone()).or(item.version.clone()),
            author: detail.as_ref().map(|d| d.card.author.clone()),
            uploaded_at: file.map(|f| f.uploaded_at),
            md5,
            thumbnail: detail.as_ref().and_then(|d| d.card.thumbnail_url.clone()),
            requirements: detail.as_ref().map(|d| d.requirements.iter().filter_map(|r| r.mod_id).collect()).unwrap_or_default(),
            frameworks: detail.as_ref().map(|d| d.card.derived.frameworks.iter().copied().collect()).unwrap_or_default(),
        };
        let _ = size;
        let guard = self.install_lock.lock().await;
        let app = self.clone();
        let report = tokio::task::spawn_blocking(move || app.install_archive(&path, meta)).await.map_err(|e| Error::Other(e.to_string()))??;
        drop(guard);
        if !report.redmods.is_empty() && self.settings().redmod_auto_deploy && self.game().is_some_and(|g| g.redmod_tool) {
            let _ = self.redmod_deploy().await;
        }
        Ok(report)
    }

    // install ---------------------------------------------------------------------------------------------

    /// Installs an archive: extract into the store, plan the layout, deploy (enabled in the active profile).
    /// A newer file of a mod already in the library replaces the old one and keeps its enabled state.
    pub fn install_archive(&self, archive: &Path, mut meta: InstallMeta) -> Result<InstallReport> {
        let store = self.store()?;
        let game_root = self.game_root()?;
        if meta.md5.is_none() {
            meta.md5 = crate::download::md5_file(archive).ok();
        }
        if meta.name.trim().is_empty() {
            meta.name = archive.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Mod".into());
        }
        if meta.file_name.is_empty() {
            meta.file_name = archive.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        let key = match (meta.mod_id, meta.file_id) {
            (Some(m), Some(f)) => InstalledMod::key_for(m, f),
            _ => format!("local-{}", meta.md5.as_deref().unwrap_or("0000000000").chars().take(10).collect::<String>()),
        };
        let staging = store.staging(&key);
        let tmp = store.root.join("tmp").join(&key);
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp).map_err(|e| Error::io_at(e, &tmp))?;
        }
        let files = crate::archive::extract(archive, &tmp)?;
        let plan = match layout::plan(&files, &meta.name) {
            Ok(p) => p,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(e);
            }
        };
        // replace an earlier version of the same mod, keeping its enabled state
        let profile = lock(&self.lib).active_profile()?;
        let previous = meta.mod_id.and_then(|id| lock(&self.lib).mods().ok()?.into_iter().find(|m| m.mod_id == Some(id) && m.key != key));
        let was_enabled = match &previous {
            Some(p) => lock(&self.lib).enabled_in(&profile)?.contains(&p.key),
            None => true,
        };
        if let Some(p) = &previous {
            self.remove_mod(&p.key)?;
        }
        // undeploy a reinstall of the same key first
        if let Some(m) = store.load_manifest(&key)? {
            let mut others: Vec<Manifest> = store.load_manifests()?.into_iter().filter(|o| o.mod_key != key).collect();
            deploy::undeploy(&m, &mut others)?;
            for o in &others {
                store.save_manifest(o)?;
            }
            store.delete_manifest(&key)?;
        }
        // move the planned files from tmp into the staging folder at their game-relative paths
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(|e| Error::io_at(e, &staging))?;
        }
        for p in &plan.placements {
            let from = crate::frameworks::game_path(&tmp, &p.from);
            let to = deploy::game_path(&staging, &p.to);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::io_at(e, parent))?;
            }
            std::fs::rename(&from, &to).or_else(|_| std::fs::copy(&from, &to).map(|_| ())).map_err(|e| Error::io_at(e, &to))?;
        }
        let _ = std::fs::remove_dir_all(&tmp);

        let mut derived_fw: Vec<frameworks::Framework> = meta.frameworks.clone();
        let staged_files: Vec<String> = plan.placements.iter().map(|p| p.to.clone()).collect();
        derived_fw.extend(classify::from_files(&staged_files).1);
        derived_fw.extend(classify::from_requirements(&meta.requirements));
        if let Some(own) = meta.mod_id.and_then(frameworks::by_mod_id) {
            derived_fw.retain(|f| *f != own.id);
        }
        derived_fw.sort();
        derived_fw.dedup();
        let kind = if meta.mod_id.and_then(frameworks::by_mod_id).is_some() { ModKind::Framework } else { plan.kind };
        let m = InstalledMod {
            key: key.clone(),
            mod_id: meta.mod_id,
            file_id: meta.file_id,
            name: meta.name.clone(),
            file_name: meta.file_name.clone(),
            version: meta.version.clone(),
            author: meta.author.clone(),
            kind,
            frameworks: derived_fw,
            requirements: meta.requirements.clone(),
            md5: meta.md5.clone(),
            size: std::fs::metadata(archive).map(|m| m.len()).unwrap_or(0),
            installed_at: timefmt::now(),
            uploaded_at: meta.uploaded_at,
            archive_path: Some(archive.to_path_buf()),
            redmods: plan.redmods.clone(),
            thumbnail: meta.thumbnail.clone(),
            unplaced: plan.unplaced.clone(),
            update_available: None,
            endorsed: previous.as_ref().and_then(|p| p.endorsed),
        };
        {
            let lib = lock(&self.lib);
            lib.upsert_mod(&m)?;
            lib.set_enabled(&profile, &key, was_enabled)?;
        }
        let mut report = InstallReport {
            key: key.clone(),
            name: m.name.clone(),
            kind,
            placed: plan.placements.len(),
            skipped_docs: plan.skipped_docs.len(),
            unplaced: plan.unplaced.clone(),
            conflicts: vec![],
            copied: 0,
            replaced: previous.map(|p| p.name),
            redmods: plan.redmods.clone(),
        };
        if was_enabled {
            let dep = self.deploy_key(&store, &game_root, &key)?;
            report.conflicts = dep.conflicts;
            report.copied = dep.copied;
        }
        if !plan.redmods.is_empty() {
            self.mark_redmod_dirty()?;
        }
        Ok(report)
    }

    /// Installs an archive the person picked from disk (not from Nexus).
    pub fn install_file(&self, archive: &Path) -> Result<InstallReport> {
        self.install_archive(archive, InstallMeta::default())
    }

    fn staged_files(store: &Store, key: &str) -> Result<Vec<PlannedFile>> {
        let root = store.staging(key);
        if !root.exists() {
            return Err(Error::NotFound(format!("The store has no files for {key}; reinstall the mod.")));
        }
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
            let rel = e.path().strip_prefix(&root).unwrap_or(e.path()).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            out.push(PlannedFile { source: e.path().to_path_buf(), rel });
        }
        out.sort_by(|a, b| a.rel.cmp(&b.rel));
        Ok(out)
    }

    fn deploy_key(&self, store: &Store, game_root: &Path, key: &str) -> Result<deploy::DeployReport> {
        let files = App::staged_files(store, key)?;
        let others: Vec<Manifest> = store.load_manifests()?.into_iter().filter(|m| m.mod_key != key).collect();
        let owners = Ownership::from_manifests(&others);
        let s = self.settings();
        let rep = deploy::deploy(key, game_root, &files, &owners, &store.backups(), &DeployOptions { allow_copy: s.allow_copy, overwrite_other_mods: s.later_wins })?;
        store.save_manifest(&rep.manifest)?;
        Ok(rep)
    }

    fn undeploy_key(&self, store: &Store, key: &str) -> Result<()> {
        if let Some(m) = store.load_manifest(key)? {
            let mut others: Vec<Manifest> = store.load_manifests()?.into_iter().filter(|o| o.mod_key != key).collect();
            deploy::undeploy(&m, &mut others)?;
            for o in &others {
                store.save_manifest(o)?;
            }
            store.delete_manifest(key)?;
        }
        Ok(())
    }

    /// Removes a mod from the game and the library. Its downloaded archive stays in downloads.
    pub fn remove_mod(&self, key: &str) -> Result<()> {
        let store = self.store()?;
        let had_redmods = lock(&self.lib).get_mod(key)?.is_some_and(|m| !m.redmods.is_empty());
        self.undeploy_key(&store, key)?;
        let staging = store.staging(key);
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(|e| Error::io_at(e, &staging))?;
        }
        lock(&self.lib).delete_mod(key)?;
        if had_redmods {
            self.mark_redmod_dirty()?;
        }
        Ok(())
    }

    pub fn set_enabled(&self, key: &str, enabled: bool) -> Result<()> {
        let store = self.store()?;
        let root = self.game_root()?;
        let profile = lock(&self.lib).active_profile()?;
        if enabled {
            self.deploy_key(&store, &root, key)?;
        } else {
            self.undeploy_key(&store, key)?;
        }
        let lib = lock(&self.lib);
        lib.set_enabled(&profile, key, enabled)?;
        let redmods = lib.get_mod(key)?.is_some_and(|m| !m.redmods.is_empty());
        drop(lib);
        if redmods {
            self.mark_redmod_dirty()?;
        }
        Ok(())
    }

    /// Puts a mod's files back after something outside the app removed or changed them.
    pub fn redeploy(&self, key: &str) -> Result<()> {
        let store = self.store()?;
        let root = self.game_root()?;
        self.undeploy_key(&store, key)?;
        self.deploy_key(&store, &root, key)?;
        Ok(())
    }

    pub fn library(&self) -> Result<Vec<LibraryRow>> {
        let store = self.store().ok();
        let manifests: Vec<Manifest> = store.as_ref().map(|s| s.load_manifests()).transpose()?.unwrap_or_default();
        let lib = lock(&self.lib);
        let enabled = lib.enabled_in(&lib.active_profile()?)?;
        Ok(lib
            .mods()?
            .into_iter()
            .map(|m| {
                let man = manifests.iter().find(|x| x.mod_key == m.key);
                LibraryRow {
                    enabled: enabled.contains(&m.key),
                    deployed: man.is_some(),
                    drift: man.map(|x| deploy::verify(x).len()).unwrap_or(0),
                    files: man.map(|x| x.entries.len()).unwrap_or(0),
                    m,
                }
            })
            .collect())
    }

    // profiles --------------------------------------------------------------------------------------------

    pub fn profiles(&self) -> Result<(Vec<String>, String)> {
        let lib = lock(&self.lib);
        Ok((lib.profiles()?, lib.active_profile()?))
    }

    pub fn create_profile(&self, name: &str, copy_current: bool) -> Result<()> {
        let lib = lock(&self.lib);
        let cur = lib.active_profile()?;
        lib.create_profile(name, copy_current.then_some(cur.as_str()))
    }

    pub fn delete_profile(&self, name: &str) -> Result<()> {
        lock(&self.lib).delete_profile(name)
    }

    /// Switches profile: undeploys what the new profile doesn't enable, deploys what it does.
    pub fn switch_profile(&self, name: &str) -> Result<()> {
        let store = self.store()?;
        let root = self.game_root()?;
        let (from, to) = {
            let lib = lock(&self.lib);
            let cur = lib.active_profile()?;
            let from = lib.enabled_in(&cur)?;
            let to = lib.enabled_in(name)?;
            lib.set_active_profile(name)?;
            (from, to)
        };
        for k in from.difference(&to) {
            self.undeploy_key(&store, k)?;
        }
        let mods = lock(&self.lib).mods()?;
        for m in mods.iter().filter(|m| to.contains(&m.key) && !from.contains(&m.key)) {
            self.deploy_key(&store, &root, &m.key)?;
        }
        self.mark_redmod_dirty()?;
        Ok(())
    }

    // REDmod and load order -------------------------------------------------------------------------------

    fn mark_redmod_dirty(&self) -> Result<()> {
        lock(&self.lib).set_setting("redmodDirty", &true)
    }

    pub fn load_order(&self) -> Result<LoadOrderView> {
        let root = self.game_root()?;
        let present: Vec<RedmodInfo> = redmod::list(&root);
        let lib = lock(&self.lib);
        let profile = lib.active_profile()?;
        let saved = lib.load_order(&profile)?;
        let order = redmod::merge_order(&saved, &present.iter().map(|r| r.folder.clone()).collect::<Vec<_>>());
        let mods = lib.mods()?;
        let rows = order
            .iter()
            .filter_map(|f| present.iter().find(|r| &r.folder == f))
            .map(|r| RedmodRow {
                folder: r.folder.clone(),
                name: r.name.clone(),
                version: r.version.clone(),
                owner: mods.iter().find(|m| m.redmods.iter().any(|x| x.eq_ignore_ascii_case(&r.folder))).map(|m| m.name.clone()),
            })
            .collect();
        Ok(LoadOrderView {
            rows,
            tool: redmod::tool_path(&root).is_file(),
            dirty: lib.setting::<bool>("redmodDirty")?.unwrap_or(false),
            last: lib.setting::<DeployOutcome>("redmodLast")?,
        })
    }

    pub fn set_load_order(&self, order: &[String]) -> Result<()> {
        let lib = lock(&self.lib);
        let p = lib.active_profile()?;
        lib.set_load_order(&p, order)?;
        lib.set_setting("redmodDirty", &true)
    }

    pub async fn redmod_deploy(&self) -> Result<DeployOutcome> {
        let root = self.game_root()?;
        let order: Vec<String> = self.load_order()?.rows.into_iter().map(|r| r.folder).collect();
        let out = redmod::run_deploy(&root, &order).await?;
        let lib = lock(&self.lib);
        lib.set_setting("redmodLast", &out)?;
        if out.ok {
            lib.set_setting("redmodDirty", &false)?;
        }
        Ok(out)
    }

    // health ----------------------------------------------------------------------------------------------

    pub fn health(&self) -> Result<HealthReport> {
        let game = self.game();
        let fw = game.as_ref().map(|g| frameworks::detect(&g.root)).unwrap_or_default();
        let lib = lock(&self.lib);
        let enabled = lib.enabled_in(&lib.active_profile()?)?;
        let all = lib.mods()?;
        let mods: Vec<InstalledMod> = all.iter().filter(|m| enabled.contains(&m.key)).cloned().collect();
        let last: Option<DeployOutcome> = lib.setting("redmodLast")?;
        let dirty = lib.setting::<bool>("redmodDirty")?.unwrap_or(false);
        drop(lib);
        let name_of = |k: &str| all.iter().find(|m| m.key == k).map(|m| m.name.clone()).unwrap_or_else(|| k.to_string());
        let manifests = self.store().ok().map(|s| s.load_manifests()).transpose()?.unwrap_or_default();
        let conflicts: Vec<ConflictRecord> = manifests
            .iter()
            .flat_map(|m| m.entries.iter().filter_map(move |e| e.displaced.as_ref().map(|l| (m.mod_key.clone(), l.clone(), e.rel.clone()))))
            .map(|(w, l, rel)| ConflictRecord { rel, winner: name_of(&w), loser: name_of(&l) })
            .collect();
        let drift: Vec<(String, String, usize)> = manifests
            .iter()
            .map(|m| (m.mod_key.clone(), name_of(&m.mod_key), deploy::verify(m).len()))
            .filter(|(_, _, n)| *n > 0)
            .collect();
        let redmods_present = game.as_ref().map(|g| redmod::list(&g.root).len()).unwrap_or(0);
        Ok(health::check(&HealthInput {
            game: game.as_ref(),
            frameworks: &fw,
            mods: &mods,
            conflicts: &conflicts,
            drift: &drift,
            redmods_present,
            redmod_last: last.as_ref(),
            redmod_dirty: dirty,
        }))
    }

    // collections -----------------------------------------------------------------------------------------

    pub async fn collections_browse(&self, text: Option<&str>, matching_version: bool, sort: &str, offset: u32) -> Result<(Vec<CollectionCard>, u64)> {
        let version = if matching_version { self.game().and_then(|g| g.version) } else { None };
        let vars = collections::collections_variables(text, version.as_deref(), sort, offset, 20);
        let key = format!("collections:{vars}");
        if let Some(v) = lock(&self.lib).cache_get(&key)? {
            if let Ok(r) = serde_json::from_value(v) {
                return Ok(r);
            }
        }
        let body = self.client()?.graphql(collections::COLLECTIONS_QUERY, vars).await?;
        let r = collections::parse_collections(&body)?;
        lock(&self.lib).cache_put(&key, &serde_json::to_value(&r)?, TTL_COLLECTIONS)?;
        Ok(r)
    }

    /// Downloads a Nexus collection's manifest (not its mods) and keeps it in the store.
    pub async fn fetch_collection(&self, slug: &str, revision: Option<u64>) -> Result<Collection> {
        let c = self.client()?;
        let body = c.graphql(collections::REVISION_QUERY, collections::revision_variables(slug, revision)).await?;
        let (rev, link) = collections::parse_revision_link(&body)?;
        let links = collections::parse_collection_download(&c.v1_path(&link).await?);
        let url = links.first().ok_or_else(|| Error::BadResponse { what: "collection download".into(), detail: "no URLs".into() })?;
        let dir = self.store()?.root.join("collections");
        let name = format!("{slug}-{}.7z", rev.unwrap_or(0));
        let dl = crate::download::download(c.http(), url, &dir, &name, |_, _| {}).await?;
        let scratch = dir.join(format!("{slug}-{}", rev.unwrap_or(0)));
        let col = tokio::task::spawn_blocking(move || collections::read_manifest_from_archive(&dl.path, &scratch)).await.map_err(|e| Error::Other(e.to_string()))??;
        lock(&self.lib).save_collection(&format!("nexus:{slug}"), &serde_json::to_value(&col)?)?;
        Ok(col)
    }

    pub fn collection_precheck(&self, c: &Collection, include_optional: bool) -> Result<PreCheck> {
        let installed = lock(&self.lib).mods()?;
        let premium = lock(&self.queue).premium;
        let version = self.game().and_then(|g| g.version);
        Ok(collections::pre_check(c, &installed, premium, version.as_deref(), include_optional))
    }

    pub fn collection_install(&self, c: &Collection, slug: &str, include_optional: bool) -> Result<Vec<u64>> {
        let check = self.collection_precheck(c, include_optional)?;
        Ok(self.queue_items(collections::queue_items(c, &check, slug)))
    }

    pub fn collection_export(&self, name: &str, author: &str, description: &str, keys: &[String]) -> Result<Collection> {
        let lib = lock(&self.lib);
        let mods: Vec<InstalledMod> = lib.mods()?.into_iter().filter(|m| keys.contains(&m.key)).collect();
        let order = lib.load_order(&lib.active_profile()?)?;
        drop(lib);
        let version = self.game().and_then(|g| g.version);
        let c = collections::export(name, author, description, &mods, &order, version.as_deref())?;
        lock(&self.lib).save_collection(&format!("local:{}", c.info.name), &serde_json::to_value(&c)?)?;
        Ok(c)
    }

    pub fn saved_collections(&self) -> Result<Vec<(String, Value)>> {
        lock(&self.lib).collections()
    }

    pub fn delete_saved_collection(&self, name: &str) -> Result<()> {
        lock(&self.lib).delete_collection(name)
    }

    // mod.io (experiment) ---------------------------------------------------------------------------------

    pub async fn modio_browse(&self, text: &str, offset: u32) -> Result<(Vec<crate::modio::ModioMod>, u64)> {
        let s = self.settings();
        if !s.experiments.modio {
            return Err(Error::Invalid("Turn on the mod.io experiment in Settings first.".into()));
        }
        let game_id = s.modio_game_id.ok_or_else(|| Error::Invalid("Add Cyberpunk 2077's mod.io game ID in Settings.".into()))?;
        let key = secrets::get(secrets::MODIO_KEY_TARGET)?.unwrap_or_default();
        let http = reqwest::Client::builder().user_agent(nexus::user_agent()).build()?;
        crate::modio::search(&http, &key, game_id, text, offset, 20).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::make_zip;

    struct NullShell;
    impl Shell for NullShell {
        fn open_nexus_page(&self, _: &str, _: &str) {}
        fn emit(&self, _: &str, _: Value) {}
    }

    fn setup() -> (tempfile::TempDir, App, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Cyberpunk 2077");
        let exe = crate::frameworks::game_path(&game, game::EXE_REL);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"MZ").unwrap();
        let app = App::open(&dir.path().join("data")).unwrap();
        let mut s = app.settings();
        s.store_root = Some(dir.path().join("store"));
        app.save_settings(s).unwrap();
        app.set_game_root(&game).unwrap();
        (dir, app, game)
    }

    fn meta(mod_id: u64, file_id: u64, name: &str) -> InstallMeta {
        InstallMeta { mod_id: Some(mod_id), file_id: Some(file_id), name: name.into(), uploaded_at: Some(1), ..Default::default() }
    }

    #[test]
    fn install_disable_enable_remove() {
        let (dir, app, game) = setup();
        let zp = dir.path().join("jacket.zip");
        make_zip(&zp, &[("Jacket/archive/pc/mod/jacket.archive", b"J"), ("Jacket/readme.txt", b"r")]);
        let r = app.install_archive(&zp, meta(5, 50, "Jacket")).unwrap();
        assert_eq!((r.key.as_str(), r.placed, r.skipped_docs), ("5-50", 1, 1));
        let target = game.join("archive/pc/mod/jacket.archive");
        assert!(target.exists());
        let lib = app.library().unwrap();
        assert!(lib[0].enabled && lib[0].deployed && lib[0].drift == 0);

        app.set_enabled("5-50", false).unwrap();
        assert!(!target.exists());
        app.set_enabled("5-50", true).unwrap();
        assert!(target.exists());

        app.remove_mod("5-50").unwrap();
        assert!(!target.exists());
        assert!(app.library().unwrap().is_empty());
        assert!(zp.exists(), "the downloaded archive is kept");
    }

    #[test]
    fn a_new_file_replaces_the_old_version() {
        let (dir, app, game) = setup();
        let v1 = dir.path().join("v1.zip");
        make_zip(&v1, &[("archive/pc/mod/a.archive", b"1"), ("archive/pc/mod/old-only.archive", b"o")]);
        app.install_archive(&v1, meta(5, 50, "A")).unwrap();
        app.set_enabled("5-50", false).unwrap();
        let v2 = dir.path().join("v2.zip");
        make_zip(&v2, &[("archive/pc/mod/a.archive", b"2")]);
        let r = app.install_archive(&v2, meta(5, 51, "A")).unwrap();
        assert_eq!(r.replaced.as_deref(), Some("A"));
        let lib = app.library().unwrap();
        assert_eq!(lib.len(), 1);
        assert_eq!(lib[0].m.key, "5-51");
        assert!(!lib[0].enabled, "keeps the old version's disabled state");
        assert!(!game.join("archive/pc/mod/old-only.archive").exists());
    }

    #[test]
    fn conflicts_show_in_health_and_later_wins() {
        let (dir, app, game) = setup();
        let a = dir.path().join("a.zip");
        make_zip(&a, &[("archive/pc/mod/shared.archive", b"A")]);
        let b = dir.path().join("b.zip");
        make_zip(&b, &[("archive/pc/mod/shared.archive", b"B")]);
        app.install_archive(&a, meta(1, 1, "Alpha")).unwrap();
        let rb = app.install_archive(&b, meta(2, 2, "Beta")).unwrap();
        assert!(rb.conflicts[0].won);
        assert_eq!(std::fs::read(game.join("archive/pc/mod/shared.archive")).unwrap(), b"B");
        let h = app.health().unwrap();
        let c = h.modules.iter().find(|m| m.id == "conflicts").unwrap();
        assert_eq!(c.items.len(), 1);
        assert!(c.items[0].detail.starts_with("Beta wins"));
        app.remove_mod("2-2").unwrap();
        assert_eq!(std::fs::read(game.join("archive/pc/mod/shared.archive")).unwrap(), b"A");
    }

    #[test]
    fn profiles_switch_what_is_deployed() {
        let (dir, app, game) = setup();
        let a = dir.path().join("a.zip");
        make_zip(&a, &[("archive/pc/mod/a.archive", b"A")]);
        app.install_archive(&a, meta(1, 1, "Alpha")).unwrap();
        app.create_profile("Clean", false).unwrap();
        app.switch_profile("Clean").unwrap();
        assert!(!game.join("archive/pc/mod/a.archive").exists());
        app.switch_profile(crate::library::DEFAULT_PROFILE).unwrap();
        assert!(game.join("archive/pc/mod/a.archive").exists());
    }

    #[test]
    fn redmods_enter_the_load_order() {
        let (dir, app, _game) = setup();
        let z = dir.path().join("r.zip");
        make_zip(&z, &[("Zeta/info.json", b"{\"name\":\"Zeta\"}"), ("Zeta/archives/z.archive", b"z")]);
        app.install_archive(&z, meta(3, 3, "Zeta mod")).unwrap();
        let z2 = dir.path().join("r2.zip");
        make_zip(&z2, &[("mods/Alpha/info.json", b"{\"name\":\"Alpha\"}"), ("mods/Alpha/archives/a.archive", b"a")]);
        app.install_archive(&z2, meta(4, 4, "Alpha mod")).unwrap();
        let lo = app.load_order().unwrap();
        assert_eq!(lo.rows.iter().map(|r| r.folder.as_str()).collect::<Vec<_>>(), vec!["Alpha", "Zeta"]);
        assert!(lo.dirty);
        assert_eq!(lo.rows[1].owner.as_deref(), Some("Zeta mod"));
        app.set_load_order(&["Zeta".into(), "Alpha".into()]).unwrap();
        assert_eq!(app.load_order().unwrap().rows[0].folder, "Zeta");
        let h = app.health().unwrap();
        assert_eq!(h.modules.iter().find(|m| m.id == "redmod").unwrap().status, health::Status::Bad, "no REDmod tool in the fake game");
    }

    #[test]
    fn export_a_local_collection() {
        let (dir, app, _game) = setup();
        let a = dir.path().join("a.zip");
        make_zip(&a, &[("archive/pc/mod/a.archive", b"A")]);
        app.install_archive(&a, meta(1, 10, "Alpha")).unwrap();
        let c = app.collection_export("Mine", "me", "", &["1-10".into()]).unwrap();
        assert_eq!(c.mods[0].source.file_id, Some(10));
        assert!(c.mods[0].source.md5.is_some());
        let pc = app.collection_precheck(&c, false).unwrap();
        assert_eq!(pc.installed, 1);
        assert_eq!(app.saved_collections().unwrap().len(), 1);
    }

    #[test]
    fn bad_archives_leave_nothing_behind() {
        let (dir, app, game) = setup();
        let z = dir.path().join("docs.zip");
        make_zip(&z, &[("readme.txt", b"r")]);
        assert!(app.install_archive(&z, meta(9, 9, "Docs")).is_err());
        assert!(app.library().unwrap().is_empty());
        assert!(!game.join("archive").exists());
    }

    #[tokio::test]
    async fn free_queue_opens_pages_and_counts_clicks() {
        let (_dir, app, _game) = setup();
        let app = Arc::new(app);
        app.queue_items(vec![
            NewItem { mod_id: 1, file_id: 1, mod_name: "A".into(), file_name: "a.zip".into(), version: None, size_bytes: 10, manual_only: false, origin: "browse".into() },
            NewItem { mod_id: 2, file_id: 2, mod_name: "B".into(), file_name: "b.zip".into(), version: None, size_bytes: 10, manual_only: false, origin: "browse".into() },
        ]);
        struct Rec(Mutex<Vec<String>>);
        impl Shell for Rec {
            fn open_nexus_page(&self, url: &str, _: &str) {
                self.0.lock().unwrap().push(url.into());
            }
            fn emit(&self, _: &str, _: Value) {}
        }
        let rec = Arc::new(Rec(Mutex::new(vec![])));
        let shell: Arc<dyn Shell> = rec.clone();
        app.pump(&shell);
        app.pump(&shell); // doesn't open the same page twice
        assert_eq!(rec.0.lock().unwrap().as_slice(), [App::file_page_url(1, 1)]);
        assert_eq!(app.queue_view().summary.needs_click, 2);
        let _ = NullShell;
    }

    #[test]
    fn local_filters_and_facets() {
        let (_dir, app, _game) = setup();
        let card = |id: u64, name: &str, summary: &str| nexus::ModCard {
            mod_id: id,
            name: name.into(),
            summary: summary.into(),
            author: "a".into(),
            thumbnail_url: None,
            picture_url: None,
            downloads: 0,
            endorsements: 0,
            updated_at: i64::MAX / 2,
            created_at: 0,
            size_bytes: None,
            category: None,
            adult: false,
            version: None,
            derived: Default::default(),
            compat: Compat::Unknown,
            hidden: false,
            downloaded: false,
            installed: false,
        };
        let mut page = ModPage {
            mods: vec![card(1, "Lua thing", "Requires Cyber Engine Tweaks"), card(2, "Outfit", "ArchiveXL outfit"), card(3, "Hidden", "")],
            total: 3,
            offset: 0,
            facets: Default::default(),
            cached: false,
            filtered_out: 0,
        };
        app.set_hidden(3, true).unwrap();
        let q = BrowseQuery { frameworks: vec![frameworks::Framework::Cet], ..Default::default() };
        app.enrich_page(&mut page, &q).unwrap();
        assert_eq!(page.mods.len(), 1);
        assert_eq!(page.mods[0].mod_id, 1);
        assert_eq!(page.filtered_out, 2);
        assert_eq!(page.facets.hidden, 1);
        assert!(page.facets.frameworks.iter().any(|f| f.value == "cet" && f.count == 1));
        assert!(page.facets.frameworks.iter().any(|f| f.value == "archiveXl"));
    }
}
