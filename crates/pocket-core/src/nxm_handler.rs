//! Registering Pocket Mods as the nxm:// handler, per user (HKCU, no admin rights), and taking it back from
//! another manager. The previous handler's command is remembered so the person can hand it back.

use crate::Result;
use serde::Serialize;
use std::path::Path;

/// Where URL protocol handlers live for the current user.
pub const NXM_KEY: &str = "Software\\Classes\\nxm";
/// Where Pocket Mods remembers the handler it replaced.
pub const APP_KEY: &str = "Software\\PocketMods";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HandlerStatus {
    pub supported: bool,
    pub ours: bool,
    /// The command currently registered, if any.
    pub command: Option<String>,
    /// A short name for whoever holds it now ("Vortex", "Mod Organizer 2", or the exe name).
    pub holder: Option<String>,
    pub previous: Option<String>,
}

/// The open command for our exe: `"C:\...\Pocket Mods.exe" "%1"`.
pub fn command_for(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// The exe path a handler command starts with.
pub fn exe_of(command: &str) -> String {
    let c = command.trim();
    if let Some(rest) = c.strip_prefix('"') {
        rest.split('"').next().unwrap_or("").to_string()
    } else {
        c.split_whitespace().next().unwrap_or("").to_string()
    }
}

pub fn is_ours(command: &str, exe: &Path) -> bool {
    exe_of(command).replace('/', "\\").eq_ignore_ascii_case(&exe.display().to_string().replace('/', "\\"))
}

pub fn holder_name(command: &str) -> String {
    let exe = exe_of(command);
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(&exe).to_string();
    let lower = file.to_ascii_lowercase();
    if lower.contains("vortex") {
        "Vortex".into()
    } else if lower.contains("nxmhandler") || lower.contains("modorganizer") {
        "Mod Organizer 2".into()
    } else {
        file
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::Error;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    fn reg_err(e: std::io::Error) -> Error {
        Error::Other(format!("Couldn't update the registry: {e}"))
    }

    pub fn current(nxm_key: &str) -> Option<String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey_with_flags(format!("{nxm_key}\\shell\\open\\command"), KEY_READ).ok()?.get_value::<String, _>("").ok()
    }

    pub fn status(exe: &Path, nxm_key: &str, app_key: &str) -> HandlerStatus {
        let command = current(nxm_key);
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let previous = hkcu.open_subkey(app_key).ok().and_then(|k| k.get_value::<String, _>("PreviousNxmHandler").ok());
        HandlerStatus {
            supported: true,
            ours: command.as_deref().is_some_and(|c| is_ours(c, exe)),
            holder: command.as_deref().map(holder_name),
            command,
            previous,
        }
    }

    pub fn register(exe: &Path, nxm_key: &str, app_key: &str) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Some(prev) = current(nxm_key) {
            if !is_ours(&prev, exe) {
                let (app, _) = hkcu.create_subkey(app_key).map_err(reg_err)?;
                app.set_value("PreviousNxmHandler", &prev).map_err(reg_err)?;
            }
        }
        let (k, _) = hkcu.create_subkey(nxm_key).map_err(reg_err)?;
        k.set_value("", &"URL:NXM Protocol").map_err(reg_err)?;
        k.set_value("URL Protocol", &"").map_err(reg_err)?;
        let (icon, _) = k.create_subkey("DefaultIcon").map_err(reg_err)?;
        icon.set_value("", &format!("\"{}\",0", exe.display())).map_err(reg_err)?;
        let (cmd, _) = k.create_subkey("shell\\open\\command").map_err(reg_err)?;
        cmd.set_value("", &command_for(exe)).map_err(reg_err)?;
        Ok(())
    }

    pub fn restore_previous(nxm_key: &str, app_key: &str) -> Result<bool> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Some(prev) = hkcu.open_subkey(app_key).ok().and_then(|k| k.get_value::<String, _>("PreviousNxmHandler").ok()) else {
            return Ok(false);
        };
        let (cmd, _) = hkcu.create_subkey(format!("{nxm_key}\\shell\\open\\command")).map_err(reg_err)?;
        cmd.set_value("", &prev).map_err(reg_err)?;
        Ok(true)
    }
}

#[cfg(windows)]
pub fn status(exe: &Path) -> HandlerStatus {
    imp::status(exe, NXM_KEY, APP_KEY)
}
#[cfg(windows)]
pub fn register(exe: &Path) -> Result<()> {
    imp::register(exe, NXM_KEY, APP_KEY)
}
#[cfg(windows)]
pub fn restore_previous() -> Result<bool> {
    imp::restore_previous(NXM_KEY, APP_KEY)
}

#[cfg(not(windows))]
pub fn status(_exe: &Path) -> HandlerStatus {
    HandlerStatus { supported: false, ours: false, command: None, holder: None, previous: None }
}
#[cfg(not(windows))]
pub fn register(_exe: &Path) -> Result<()> {
    Err(crate::Error::Other("nxm:// registration is Windows only.".into()))
}
#[cfg(not(windows))]
pub fn restore_previous() -> Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands() {
        let exe = Path::new("C:\\Program Files\\Pocket Mods\\Pocket Mods.exe");
        let c = command_for(exe);
        assert_eq!(c, "\"C:\\Program Files\\Pocket Mods\\Pocket Mods.exe\" \"%1\"");
        assert!(is_ours(&c, exe));
        assert!(is_ours(&c.to_ascii_lowercase(), exe));
        assert!(!is_ours("\"C:\\Vortex\\Vortex.exe\" -d \"%1\"", exe));
        assert_eq!(holder_name("\"C:\\Program Files\\Black Tree Gaming Ltd\\Vortex\\Vortex.exe\" -d \"%1\""), "Vortex");
        assert_eq!(holder_name("C:\\MO2\\nxmhandler.exe \"%1\""), "Mod Organizer 2");
        assert_eq!(exe_of("C:\\x\\y.exe \"%1\""), "C:\\x\\y.exe");
    }

    /// Registers under a test-only key in HKCU, takes it over from a fake Vortex, then hands it back.
    #[cfg(windows)]
    #[test]
    fn register_take_back_and_restore() {
        use winreg::enums::HKEY_CURRENT_USER;
        let nxm = "Software\\PocketModsTest\\Classes\\nxm";
        let app = "Software\\PocketModsTest\\App";
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all("Software\\PocketModsTest");
        let (k, _) = hkcu.create_subkey(format!("{nxm}\\shell\\open\\command")).unwrap();
        let vortex = "\"C:\\Vortex\\Vortex.exe\" -d \"%1\"";
        k.set_value("", &vortex).unwrap();

        let exe = Path::new("C:\\Apps\\Pocket Mods.exe");
        let s = imp::status(exe, nxm, app);
        assert!(!s.ours);
        assert_eq!(s.holder.as_deref(), Some("Vortex"));

        imp::register(exe, nxm, app).unwrap();
        let s = imp::status(exe, nxm, app);
        assert!(s.ours);
        assert_eq!(s.previous.as_deref(), Some(vortex));

        assert!(imp::restore_previous(nxm, app).unwrap());
        assert_eq!(imp::current(nxm).as_deref(), Some(vortex));
        hkcu.delete_subkey_all("Software\\PocketModsTest").unwrap();
    }
}
