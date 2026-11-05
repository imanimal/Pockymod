//! Pocket Mods core: Nexus, file and Cyberpunk logic, kept free of any UI so it can be ported later.
//!
//! The Tauri shell (`src-tauri`) is a thin layer over this crate. Nothing in here knows about windows,
//! webviews or the front end.

pub mod app;
pub mod appupdate;
pub mod archive;
pub mod classify;
pub mod collections;
pub mod deploy;
pub mod download;
pub mod error;
pub mod frameworks;
pub mod game;
pub mod health;
pub mod layout;
pub mod library;
pub mod modio;
pub mod nexus;
pub mod nxm;
pub mod nxm_handler;
pub mod queue;
pub mod redmod;
pub mod secrets;
pub mod timefmt;

pub use error::{Error, Result};

/// The Nexus game domain this app is built for.
pub const GAME_DOMAIN: &str = "cyberpunk2077";
/// Sent as `Application-Name` on every Nexus request (spec section 4).
pub const APP_NAME: &str = "Pocket Mods";
/// Sent as `Application-Version` on every Nexus request.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
