//! Pockymod core: everything that is not UI. The Tauri app in `src-tauri` is a thin layer of commands over it.

pub mod appupdate;
pub mod archive;
pub mod browse;
pub mod collection_manifest;
pub mod collections;
pub mod creator;
pub mod credentials;
pub mod cyberpunk;
pub mod db;
pub mod deploy;
pub mod download;
pub mod firefox;
pub mod fomod;
pub mod found;
pub mod health;
pub mod install;
pub mod launch;
pub mod links;
pub mod modio;
pub mod platform;
pub mod profiles;
pub mod queue;
pub mod relocate;
pub mod rules;
pub mod updates;
pub mod vortex;
pub mod vortex_profiles;
pub mod error;
pub mod nexus;

pub use error::{Error, Result};

/// Name and version sent to Nexus in the `Application-Name` and `Application-Version` headers.
pub const APP_NAME: &str = "Pockymod";

/// The credential store's service name: the app's first name, so keys saved before the rename still work.
pub const KEYRING_SERVICE: &str = "Pocket Mods";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Nexus game domain this app is for.
pub const GAME_DOMAIN: &str = "cyberpunk2077";
