use serde::Serialize;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors the UI shows in plain language. `kind` lets the UI pick a band colour or a fix key.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Nexus is limiting requests. Try again after {until}")]
    Throttled { until: String },
    #[error("No Nexus API key. Add one in Settings")]
    NoKey,
    #[error("Nexus rejected the API key")]
    BadKey,
    #[error("Needs Nexus Premium")]
    NeedsPremium,
    #[error("Nexus answered {status}: {message}")]
    Http { status: u16, message: String },
    #[error("Nexus query failed: {0}")]
    GraphQl(String),
    #[error("Could not reach Nexus: {0}")]
    Network(String),
    #[error("Unexpected answer from Nexus: {0}")]
    Decode(String),
    #[error("Windows Credential Manager: {0}")]
    Credentials(String),
    #[error("{}", io_text(.0))]
    Io(#[from] std::io::Error),
    #[error("Local database: {0}")]
    Db(String),
    #[error("{0}")]
    Archive(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
}

impl Error {
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Throttled { .. } => "throttled",
            Error::NoKey => "no_key",
            Error::BadKey => "bad_key",
            Error::NeedsPremium => "needs_premium",
            Error::Http { .. } => "http",
            Error::GraphQl(_) => "graphql",
            Error::Network(_) => "network",
            Error::Decode(_) => "decode",
            Error::Credentials(_) => "credentials",
            Error::Io(_) => "io",
            Error::Db(_) => "db",
            Error::Archive(_) => "archive",
            Error::Invalid(_) => "invalid",
            Error::Conflict(_) => "conflict",
        }
    }
}

/// A file error in words a player can act on. Windows' own text is kept where it already says what happened.
pub(crate) fn io_text(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    match (e.raw_os_error(), e.kind()) {
        // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
        (Some(32 | 33), _) => "A file is in use by another program. Close the game or the tool using it, then try again".into(),
        (_, K::StorageFull | K::QuotaExceeded) => "The drive is full. Free some space, then try again".into(),
        (_, K::PermissionDenied) => "Windows didn't allow access to a file. Close the game or the tool using it, then try again".into(),
        (_, K::InvalidFilename) => format!("A file name isn't allowed on Windows ({e})"),
        _ => e.to_string(),
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() { Error::Decode(e.to_string()) } else { Error::Network(e.without_url().to_string()) }
    }
}

/// Sent to the UI as `{ kind, message }`.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Error", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
