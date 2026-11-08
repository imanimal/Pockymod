//! Shared app state: the Nexus client, the key and sign-in stores, the local database and the download queue.
use pocket_core::credentials::{OsKeyring, SecretStore};
use pocket_core::nexus::client::Auth;
use pocket_core::nexus::oauth::Tokens;
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
    /// The refresh token of a Nexus sign-in.
    pub sign_in: Box<dyn SecretStore>,
    /// Stops the sign-in that is waiting for the browser.
    pub sign_in_cancel: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub user: RwLock<Option<NexusUser>>,
    pub db: Arc<Mutex<Db>>,
    pub queue: Arc<Mutex<Queue>>,
    /// Wakes the queue runner.
    pub pump: Arc<Notify>,
    /// Queue items whose running download the user cancelled.
    pub cancel: Arc<Mutex<std::collections::HashSet<u64>>>,
    /// The library is being moved: the queue waits, so nothing is written to the old place meanwhile.
    pub moving: std::sync::atomic::AtomicBool,
    /// Files (mod, file) a background prefetch is downloading, so the queue never writes the same file at once.
    pub fetching: Mutex<std::collections::HashSet<(u32, u32)>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> pocket_core::Result<Self> {
        let keys: Box<dyn SecretStore> = Box::new(OsKeyring::nexus());
        let sign_in: Box<dyn SecretStore> = Box::new(OsKeyring::nexus_sign_in());
        // a broken credential store must not stop the app: start without a key and let Settings report it
        let key = keys.get().unwrap_or(None);
        let nexus = NexusClient::new(key)?;
        // a saved sign-in is used over a pasted key, which stays for the creator tools' Upload API
        if let Some(refresh) = sign_in.get().unwrap_or(None) { nexus.set_auth(Some(Auth::SignIn(Tokens::from_refresh(refresh)))); }
        nexus.set_token_saver(Arc::new(|t| {
            let store = OsKeyring::nexus_sign_in();
            let _ = match t.and_then(|t| t.refresh.as_ref()) { Some(r) => store.set(r), None => store.clear() };
        }));
        let db = Db::open(&data_dir.join("pocket-mods.sqlite"))?;
        // old pages are useless once their TTL has passed; keep the file small
        let _ = db.cache_prune(pocket_core::db::now() - 7 * 86400);
        // (removed mods past their time are purged after start, off the main thread: lib.rs)
        Ok(AppState {
            nexus: Arc::new(nexus), keys, sign_in, sign_in_cancel: Mutex::new(None), user: RwLock::new(None), db: Arc::new(Mutex::new(db)),
            queue: Arc::new(Mutex::new(Queue::default())), pump: Arc::new(Notify::new()), cancel: Arc::new(Mutex::new(Default::default())),
            moving: Default::default(), fetching: Default::default(),
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
            _ => Err(pocket_core::Error::Invalid("Set the Cyberpunk 2077 folder in Settings first".into())),
        }
    }
    pub fn deploy_options(&self) -> DeployOptions { DeployOptions { allow_copy: self.allow_copy, ..Default::default() } }
}
