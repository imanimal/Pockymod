pub mod addons;
pub mod browse;
pub mod collections;
pub mod existing;
pub mod extras;
pub mod files;
pub mod game;
pub mod library;
pub mod nexus;

/// Mods cannot change under a running game (its files are locked, and it would read half a change).
pub fn not_while_running() -> pocket_core::Result<()> {
    if pocket_core::launch::game_running() {
        return Err(pocket_core::Error::Invalid("Close Cyberpunk 2077 first: mods cannot change while the game is running.".into()));
    }
    Ok(())
}
