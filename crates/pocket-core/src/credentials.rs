//! The Nexus API key lives in the OS credential store (Windows Credential Manager on Windows), never in a file or log.

use crate::error::{Error, Result};
use std::fmt;
use std::sync::Mutex;

/// A secret string. Its Debug and Display never print the value.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self { Secret(s.into()) }
    pub fn expose(&self) -> &str { &self.0 }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("Secret(***)") }
}
impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("***") }
}

pub trait SecretStore: Send + Sync {
    fn get(&self) -> Result<Option<Secret>>;
    fn set(&self, secret: &Secret) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

/// The OS store. On Windows the entry shows in Credential Manager under Windows Credentials as
/// `nexus-api-key.Pocket Mods`.
pub struct OsKeyring { service: String, user: String }

impl OsKeyring {
    pub fn nexus() -> Self { Self::named("nexus-api-key") }
    /// Another secret of this app, e.g. `mod.io-api-key`.
    pub fn named(user: &str) -> Self { OsKeyring { service: crate::APP_NAME.into(), user: user.into() } }
    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.user).map_err(|e| Error::Credentials(e.to_string()))
    }
}

impl SecretStore for OsKeyring {
    fn get(&self) -> Result<Option<Secret>> {
        match self.entry()?.get_password() {
            Ok(p) => Ok(Some(Secret(p))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Credentials(e.to_string())),
        }
    }
    fn set(&self, secret: &Secret) -> Result<()> {
        self.entry()?.set_password(secret.expose()).map_err(|e| Error::Credentials(e.to_string()))
    }
    fn clear(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Credentials(e.to_string())),
        }
    }
}

/// For tests and for machines without a credential store: keeps the key in memory only.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl SecretStore for MemoryStore {
    fn get(&self) -> Result<Option<Secret>> { Ok(self.0.lock().unwrap().clone().map(Secret)) }
    fn set(&self, secret: &Secret) -> Result<()> { *self.0.lock().unwrap() = Some(secret.0.clone()); Ok(()) }
    fn clear(&self) -> Result<()> { *self.0.lock().unwrap() = None; Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secret_never_prints() {
        let s = Secret::new("abc123");
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert_eq!(format!("{s}"), "***");
        assert_eq!(s.expose(), "abc123");
    }
    #[test]
    fn memory_store_roundtrip() {
        let m = MemoryStore::default();
        assert!(m.get().unwrap().is_none());
        m.set(&Secret::new("k")).unwrap();
        assert_eq!(m.get().unwrap().unwrap().expose(), "k");
        m.clear().unwrap();
        assert!(m.get().unwrap().is_none());
    }
}
