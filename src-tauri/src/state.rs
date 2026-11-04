//! Shared app state: the Nexus client, the key store, the local database and the download queue.
use pocket_core::credentials::{OsKeyring, SecretStore};
use pocket_core::db::Db;
use pocket_core::deploy::DeployOptions;
use pocket_core::nexus::rest::NexusUser;
use pocket_core::nexus::NexusClient;
use pocket_core::queue::Queue;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, RwLock};

pub struct AppState {
    pub nexus: Arc<NexusClient>,
    pub keys: Box<dyn SecretStore>,
    pub user: RwLock<Option<NexusUser>>,
    pub db: Arc<Mutex<Db>>,
    pub queue: Arc<Mutex<Queue>>,
    /// Wakes the queue runner.
    pub pump: Arc<Notify>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> pocket_core::Result<Self> {
        let keys: Box<dyn SecretStore> = Box::new(OsKeyring::nexus());
        // a broken credential store must not stop the app: start without a key and let Settings report it
        let key = keys.get().unwrap_or(None);
        let db = Db::open(&data_dir.join("pocket-mods.sqlite"))?;
        // old pages are useless once their TTL has passed; keep the file small
        let _ = db.cache_prune(pocket_core::db::now() - 7 * 86400);
        Ok(AppState {
            nexus: Arc::new(NexusClient::new(key)?), keys, user: RwLock::new(None), db: Arc::new(Mutex::new(db)),
            queue: Arc::new(Mutex::new(Queue::default())), pump: Arc::new(Notify::new()),
        })
    }
}

/// Where the game and the mod library are, from Settings.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paths {
    pub game_dir: Option<PathBuf>,
    pub library_dir: Option<PathBuf>,
    pub allow_copy: bool,
}

impl Paths {
    pub fn load(db: &Db) -> Paths {
        let game_dir = db.setting("game.dir").ok().flatten().filter(|s| !s.is_empty()).map(PathBuf::from);
        let library_dir = db.setting("library.dir").ok().flatten().filter(|s| !s.is_empty()).map(PathBuf::from)
            .or_else(|| game_dir.as_deref().map(pocket_core::install::default_library));
        let allow_copy = db.setting("deploy.allowCopy").ok().flatten().as_deref() == Some("1");
        Paths { game_dir, library_dir, allow_copy }
    }
    pub fn require(&self) -> pocket_core::Result<(PathBuf, PathBuf)> {
        match (&self.game_dir, &self.library_dir) {
            (Some(g), Some(l)) => Ok((g.clone(), l.clone())),
            _ => Err(pocket_core::Error::Invalid("Set the Cyberpunk 2077 folder in Settings first.".into())),
        }
    }
    pub fn deploy_options(&self) -> DeployOptions { DeployOptions { allow_copy: self.allow_copy, take_conflicts: false } }
}
