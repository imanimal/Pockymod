//! The Nexus API key, kept in Windows Credential Manager (a generic credential), never in files or logs.
//!
//! Off Windows (development and CI on Linux) the key is held in memory for the process, and can be supplied
//! through the NEXUS_API_KEY environment variable.

use crate::{Error, Result};

pub const NEXUS_KEY_TARGET: &str = "PocketMods/NexusApiKey";
pub const MODIO_KEY_TARGET: &str = "PocketMods/ModioApiKey";

pub fn set(target: &str, secret: &str) -> Result<()> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(Error::Invalid("The key is empty.".into()));
    }
    imp::set(target, secret)
}

pub fn get(target: &str) -> Result<Option<String>> {
    let v = imp::get(target)?;
    if v.is_none() && target == NEXUS_KEY_TARGET {
        if let Ok(k) = std::env::var("NEXUS_API_KEY") {
            if !k.trim().is_empty() {
                return Ok(Some(k.trim().to_string()));
            }
        }
    }
    Ok(v)
}

pub fn delete(target: &str) -> Result<()> {
    imp::delete(target)
}

#[cfg(windows)]
mod imp {
    use crate::{Error, Result};
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND, FILETIME};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set(target: &str, secret: &str) -> Result<()> {
        let mut t = wide(target);
        let mut user = wide("nexus");
        let mut blob = secret.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: t.as_mut_ptr(),
            Comment: std::ptr::null_mut(),
            LastWritten: FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 },
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: user.as_mut_ptr(),
        };
        // SAFETY: every pointer in `cred` points into a buffer that outlives the call.
        let ok = unsafe { CredWriteW(&cred, 0) };
        blob.iter_mut().for_each(|b| *b = 0);
        if ok == 0 {
            return Err(Error::Other(format!("Windows Credential Manager refused to save the key (error {}).", unsafe { GetLastError() })));
        }
        Ok(())
    }

    pub fn get(target: &str) -> Result<Option<String>> {
        let t = wide(target);
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW writes a pointer we free with CredFree below.
        let ok = unsafe { CredReadW(t.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_NOT_FOUND {
                return Ok(None);
            }
            return Err(Error::Other(format!("Windows Credential Manager couldn't read the key (error {err}).")));
        }
        // SAFETY: on success p is a valid CREDENTIALW whose blob has CredentialBlobSize bytes.
        let value = unsafe {
            let c = &*p;
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            let s = String::from_utf8_lossy(bytes).into_owned();
            CredFree(p as *const _);
            s
        };
        Ok(Some(value))
    }

    pub fn delete(target: &str) -> Result<()> {
        let t = wide(target);
        let ok = unsafe { CredDeleteW(t.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 && unsafe { GetLastError() } != ERROR_NOT_FOUND {
            return Err(Error::Other("Windows Credential Manager couldn't remove the key.".into()));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::Result;
    use std::collections::HashMap;
    use std::sync::Mutex;

    static STORE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

    pub fn set(target: &str, secret: &str) -> Result<()> {
        STORE.lock().unwrap().get_or_insert_with(HashMap::new).insert(target.into(), secret.into());
        Ok(())
    }
    pub fn get(target: &str) -> Result<Option<String>> {
        Ok(STORE.lock().unwrap().as_ref().and_then(|m| m.get(target).cloned()))
    }
    pub fn delete(target: &str) -> Result<()> {
        if let Some(m) = STORE.lock().unwrap().as_mut() {
            m.remove(target);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On Windows this round-trips through the real Credential Manager under a test-only target.
    #[test]
    fn round_trip() {
        let target = "PocketMods/Test/RoundTrip";
        set(target, "  abc123  ").unwrap();
        assert_eq!(get(target).unwrap().as_deref(), Some("abc123"));
        set(target, "second").unwrap();
        assert_eq!(get(target).unwrap().as_deref(), Some("second"));
        delete(target).unwrap();
        assert_eq!(get(target).unwrap(), None);
        delete(target).unwrap(); // deleting twice is fine
        assert!(set(target, "  ").is_err());
    }
}
