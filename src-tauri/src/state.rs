//! Shared app state: the Nexus client and the key store.
use pocket_core::credentials::{OsKeyring, SecretStore};
use pocket_core::nexus::rest::NexusUser;
use pocket_core::nexus::NexusClient;
use pocket_core::db::Db;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

pub struct AppState {
    pub nexus: Arc<NexusClient>,
    pub keys: Box<dyn SecretStore>,
    pub user: RwLock<Option<NexusUser>>,
    pub db: Arc<Mutex<Db>>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> pocket_core::Result<Self> {
        let keys: Box<dyn SecretStore> = Box::new(OsKeyring::nexus());
        // a broken credential store must not stop the app: start without a key and let Settings report it
        let key = keys.get().unwrap_or(None);
        let db = Db::open(&data_dir.join("pocket-mods.sqlite"))?;
        // old pages are useless once their TTL has passed; keep the file small
        let _ = db.cache_prune(pocket_core::db::now() - 7 * 86400);
        Ok(AppState { nexus: Arc::new(NexusClient::new(key)?), keys, user: RwLock::new(None), db: Arc::new(Mutex::new(db)), data_dir })
    }
}
