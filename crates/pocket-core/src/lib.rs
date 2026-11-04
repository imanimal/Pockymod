//! Pocket Mods core: everything that is not UI. The Tauri app in `src-tauri` is a thin layer of commands over it.

pub mod appupdate;
pub mod archive;
pub mod browse;
pub mod collections;
pub mod creator;
pub mod credentials;
pub mod cyberpunk;
pub mod db;
pub mod deploy;
pub mod download;
pub mod fomod;
pub mod found;
pub mod health;
pub mod install;
pub mod launch;
pub mod modio;
pub mod platform;
pub mod profiles;
pub mod queue;
pub mod updates;
pub mod vortex;
pub mod error;
pub mod nexus;

pub use error::{Error, Result};

/// Name and version sent to Nexus in the `Application-Name` and `Application-Version` headers.
pub const APP_NAME: &str = "Pocket Mods";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Nexus game domain this app is for.
pub const GAME_DOMAIN: &str = "cyberpunk2077";
