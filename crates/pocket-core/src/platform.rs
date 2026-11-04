//! Operating-system hooks: the nxm:// handler. Windows only for now; other systems report "unsupported".
//!
//! The handler is registered per user (HKCU\Software\Classes\nxm), so no admin rights are needed. The user can take
//! it back from another manager (Vortex, Mod Organizer 2) at any time; the other manager can take it back too.

use crate::error::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerStatus {
    pub supported: bool,
    /// This app's executable is the registered handler.
    pub ours: bool,
    /// Who has it now, by name when recognised: "Pockymod", "Vortex", "Mod Organizer 2", "another app", or none.
    pub owner: Option<String>,
    /// The registered command line, for the curious.
    pub command: Option<String>,
}

pub fn owner_name(command: &str, our_exe: &Path) -> String {
    let c = command.to_lowercase();
    let ours = our_exe.to_string_lossy().to_lowercase();
    if !ours.is_empty() && c.contains(&ours) { return crate::APP_NAME.into(); }
    if c.contains("vortex") { return "Vortex".into(); }
    if c.contains("nxmhandler") || c.contains("modorganizer") { return "Mod Organizer 2".into(); }
    if c.contains("nexusmods.app") || c.contains("nexusmods") { return "Nexus Mods app".into(); }
    "another app".into()
}

/// The command this app registers: `"<exe>" "%1"`. The single-instance hook passes the link to the open window.
pub fn handler_command(exe: &Path) -> String { format!("\"{}\" \"%1\"", exe.display()) }

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::error::Error;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    use winreg::RegKey;

    pub fn status(exe: &Path) -> Result<HandlerStatus> {
        // HKCR is the merged view (per-user over machine-wide), which is what Windows uses to open the link
        let cmd: Option<String> = RegKey::predef(HKEY_CLASSES_ROOT).open_subkey("nxm\\shell\\open\\command").ok().and_then(|k| k.get_value::<String, _>("").ok());
        let owner = cmd.as_deref().map(|c| owner_name(c, exe));
        Ok(HandlerStatus { supported: true, ours: owner.as_deref() == Some(crate::APP_NAME), owner, command: cmd })
    }

    pub fn register(exe: &Path) -> Result<HandlerStatus> {
        let e = |x: std::io::Error| Error::Invalid(format!("Could not register the nxm:// handler: {x}"));
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (root, _) = hkcu.create_subkey("Software\\Classes\\nxm").map_err(e)?;
        root.set_value("", &"URL:NXM Protocol").map_err(e)?;
        root.set_value("URL Protocol", &"").map_err(e)?;
        let (icon, _) = root.create_subkey("DefaultIcon").map_err(e)?;
        icon.set_value("", &format!("\"{}\",0", exe.display())).map_err(e)?;
        let (cmd, _) = root.create_subkey("shell\\open\\command").map_err(e)?;
        cmd.set_value("", &handler_command(exe)).map_err(e)?;
        status(exe)
    }

    const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    pub fn set_autostart(exe: &Path, on: bool) -> Result<bool> {
        let e = |x: std::io::Error| Error::Invalid(format!("Could not change starting with Windows: {x}"));
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN).map_err(e)?;
        if on { k.set_value(super::AUTOSTART_NAME, &format!("\"{}\" --background", exe.display())).map_err(e)?; }
        else { let _ = k.delete_value(super::AUTOSTART_NAME); }
        autostart(exe)
    }
    pub fn autostart(exe: &Path) -> Result<bool> {
        let v: Option<String> = RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN).ok().and_then(|k| k.get_value(super::AUTOSTART_NAME).ok());
        Ok(v.is_some_and(|v| v.to_ascii_lowercase().contains(&exe.display().to_string().to_ascii_lowercase())))
    }
}

/// The Run key's value name for starting with Windows.
pub const AUTOSTART_NAME: &str = "Pockymod";

/// Start with Windows, in the background (`"<exe>" --background` in the user's Run key).
pub fn set_autostart(exe: &Path, on: bool) -> Result<bool> { imp::set_autostart(exe, on) }
pub fn autostart(exe: &Path) -> Result<bool> { imp::autostart(exe) }

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn status(_exe: &Path) -> Result<HandlerStatus> { Ok(HandlerStatus { supported: false, ours: false, owner: None, command: None }) }
    pub fn register(_exe: &Path) -> Result<HandlerStatus> {
        Err(crate::Error::Invalid("Registering the nxm:// handler is only built for Windows so far.".into()))
    }
    pub fn set_autostart(_exe: &Path, _on: bool) -> Result<bool> { Err(crate::Error::Invalid("Starting with the system is only built for Windows so far.".into())) }
    pub fn autostart(_exe: &Path) -> Result<bool> { Ok(false) }
}

pub use imp::{register as register_nxm_handler, status as nxm_handler_status};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognises_other_managers() {
        let us = Path::new("C:\\Apps\\Pockymod\\pocket-mods.exe");
        assert_eq!(owner_name("\"C:\\Program Files\\Black Tree Gaming Ltd\\Vortex\\Vortex.exe\" -d \"%1\"", us), "Vortex");
        assert_eq!(owner_name("\"D:\\MO2\\nxmhandler.exe\" \"%1\"", us), "Mod Organizer 2");
        assert_eq!(owner_name(&handler_command(us), us), "Pockymod");
        assert_eq!(owner_name("foo.exe %1", us), "another app");
    }
}
