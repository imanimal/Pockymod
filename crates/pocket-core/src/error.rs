//! One error type for the whole core. Every variant carries a plain-language message the UI can show as is.

use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::path::{Path, PathBuf};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No Nexus API key is stored yet.
    #[error("Add your Nexus API key in Settings first.")]
    NoKey,
    /// Nexus answered with an error status.
    #[error("Nexus said: {message} (HTTP {status})")]
    Nexus { status: u16, message: String },
    /// Nexus throttled us (HTTP 429), or the local budget says to stop.
    #[error("Nexus request limit reached. Requests resume {resume}.")]
    RateLimited { resume: String },
    /// The request needs a Premium account (direct download links).
    #[error("This needs a Nexus Premium account. Use the download queue instead.")]
    NeedsPremium,
    #[error("Network problem: {0}")]
    Network(String),
    #[error("Unexpected response from {what}: {detail}")]
    BadResponse { what: String, detail: String },
    #[error("{message}")]
    Io { message: String, path: Option<PathBuf> },
    /// Windows refused a write, usually under Program Files. `fix` says what to do about it.
    #[error("Windows blocked writing to {path}. {fix}")]
    Permission { path: PathBuf, fix: String },
    #[error("Could not read the archive: {0}")]
    Archive(String),
    /// The archive's layout did not match any known Cyberpunk mod shape.
    #[error("{0}")]
    Layout(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// A short machine-readable name for the UI ("noKey", "rateLimited", ...).
    pub fn kind(&self) -> &'static str {
        match self {
            Error::NoKey => "noKey",
            Error::Nexus { .. } => "nexus",
            Error::RateLimited { .. } => "rateLimited",
            Error::NeedsPremium => "needsPremium",
            Error::Network(_) => "network",
            Error::BadResponse { .. } => "badResponse",
            Error::Io { .. } => "io",
            Error::Permission { .. } => "permission",
            Error::Archive(_) => "archive",
            Error::Layout(_) => "layout",
            Error::Conflict(_) => "conflict",
            Error::Database(_) => "database",
            Error::NotFound(_) => "notFound",
            Error::Invalid(_) => "invalid",
            Error::Other(_) => "other",
        }
    }

    /// Wraps an I/O error with the path it happened on, and turns access-denied into [`Error::Permission`].
    pub fn io_at(err: std::io::Error, path: &Path) -> Error {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return Error::Permission { path: path.to_path_buf(), fix: permission_fix(path) };
        }
        Error::Io { message: format!("{} ({})", err, path.display()), path: Some(path.to_path_buf()) }
    }
}

/// The advice shown when Windows blocks a write. Game folders under Program Files are the usual case.
pub fn permission_fix(path: &Path) -> String {
    let p = path.to_string_lossy().to_ascii_lowercase();
    if p.contains("program files") {
        "The game is under Program Files, where Windows limits writes. Either move the game to another Steam or GOG \
         library folder (for example D:\\Games), or give your account Modify rights on the game folder: right-click it, \
         Properties, Security, Edit, select Users, tick Modify."
            .into()
    } else {
        "Check that the folder is not read-only and that no other program (the game, an antivirus scan) has the file open."
            .into()
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io { message: err.to_string(), path: None }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Database(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::BadResponse { what: "JSON".into(), detail: err.to_string() }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        // reqwest's Display can include the URL; our URLs never carry the API key (it goes in a header),
        // but nxm download keys can sit in query strings, so strip any query before showing it.
        let text = err.without_url().to_string();
        Error::Network(text)
    }
}

/// Errors reach the UI as `{ "kind": "...", "message": "..." }`, the message already in plain language.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Error", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let v = serde_json::to_value(Error::NoKey).unwrap();
        assert_eq!(v["kind"], "noKey");
        assert!(v["message"].as_str().unwrap().contains("API key"));
        let e = Error::io_at(std::io::Error::from(std::io::ErrorKind::PermissionDenied), Path::new("C:\\Program Files (x86)\\Steam\\x"));
        assert_eq!(serde_json::to_value(&e).unwrap()["kind"], "permission");
        assert!(e.to_string().contains("Program Files"));
    }
}
