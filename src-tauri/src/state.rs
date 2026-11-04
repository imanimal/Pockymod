//! Shared app state: the Nexus client and the key store.
use pocket_core::credentials::{OsKeyring, SecretStore};
use pocket_core::nexus::rest::NexusUser;
use pocket_core::nexus::NexusClient;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub nexus: Arc<NexusClient>,
    pub keys: Box<dyn SecretStore>,
    pub user: RwLock<Option<NexusUser>>,
}

impl AppState {
    pub fn new() -> pocket_core::Result<Self> {
        let keys: Box<dyn SecretStore> = Box::new(OsKeyring::nexus());
        // a broken credential store must not stop the app: start without a key and let Settings report it
        let key = keys.get().unwrap_or(None);
        Ok(AppState { nexus: Arc::new(NexusClient::new(key)?), keys, user: RwLock::new(None) })
    }
}
