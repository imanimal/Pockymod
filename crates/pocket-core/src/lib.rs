//! Pocket Mods core: everything that is not UI. The Tauri app in `src-tauri` is a thin layer of commands over it.

/// Name and version sent to Nexus in the `Application-Name` and `Application-Version` headers.
pub const APP_NAME: &str = "Pocket Mods";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Nexus game domain this app is for.
pub const GAME_DOMAIN: &str = "cyberpunk2077";
