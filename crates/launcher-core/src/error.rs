//! Typed error model.
//!
//! Every [`CoreError`] maps to a stable i18n `code` (looked up by the UI as
//! `errors.<code>`) plus string `params` for interpolation. `detail` is the
//! technical chain shown behind "Copy details"; it is always passed through
//! [`crate::logging::mask_secrets`].

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::Serialize;
use ts_rs::TS;

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("could not determine the user data directory")]
    NoDataDir,

    #[error("data directory is not writable: {}", path.display())]
    DataDirNotWritable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid data folder redirect in {}: {reason}", path.display())]
    InvalidRedirect { path: PathBuf, reason: String },

    #[error("i/o error at {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid json in {}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid setting: {0}")]
    InvalidSetting(String),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Stable i18n key suffix (`errors.<code>` in the frontend).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoDataDir => "paths.noDataDir",
            Self::DataDirNotWritable { .. } => "paths.notWritable",
            Self::InvalidRedirect { .. } => "paths.invalidRedirect",
            Self::Io { source, .. } if is_disk_full(source) => "io.diskFull",
            Self::Io { source, .. } if source.kind() == io::ErrorKind::PermissionDenied => {
                "io.permissionDenied"
            }
            Self::Io { .. } => "io.generic",
            Self::Json { .. } => "json.invalid",
            Self::InvalidSetting(_) => "settings.invalid",
        }
    }

    /// Interpolation parameters for the translated message.
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        match self {
            Self::DataDirNotWritable { path, .. }
            | Self::Io { path, .. }
            | Self::Json { path, .. } => {
                p.insert("path".into(), path.display().to_string());
            }
            Self::InvalidRedirect { path, reason } => {
                p.insert("path".into(), path.display().to_string());
                p.insert("reason".into(), reason.clone());
            }
            Self::InvalidSetting(what) => {
                p.insert("setting".into(), what.clone());
            }
            Self::NoDataDir => {}
        }
        p
    }

    /// Full technical description including the source chain.
    pub fn detail(&self) -> String {
        let mut out = self.to_string();
        let mut src = std::error::Error::source(self);
        while let Some(e) = src {
            out.push_str("\n  caused by: ");
            out.push_str(&e.to_string());
            src = e.source();
        }
        crate::logging::mask_secrets(&out)
    }

    pub fn to_payload(&self) -> ErrorPayload {
        ErrorPayload {
            code: self.code().to_owned(),
            params: self.params(),
            detail: self.detail(),
        }
    }
}

/// Serializable error shape returned over IPC.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ErrorPayload {
    pub code: String,
    pub params: BTreeMap<String, String>,
    pub detail: String,
}

impl From<CoreError> for ErrorPayload {
    fn from(e: CoreError) -> Self {
        e.to_payload()
    }
}

/// Detects "disk full" across platforms (`ERROR_DISK_FULL` = 112 and
/// `ERROR_HANDLE_DISK_FULL` = 39 on Windows, `ENOSPC` = 28 on Unix).
pub fn is_disk_full(e: &io::Error) -> bool {
    if e.kind() == io::ErrorKind::StorageFull {
        return true;
    }
    match e.raw_os_error() {
        #[cfg(windows)]
        Some(112) | Some(39) => true,
        #[cfg(unix)]
        Some(28) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        let e = CoreError::io("x", io::Error::new(io::ErrorKind::PermissionDenied, "nope"));
        assert_eq!(e.code(), "io.permissionDenied");
        assert_eq!(e.params()["path"], "x");
        assert_eq!(CoreError::NoDataDir.code(), "paths.noDataDir");
    }

    #[test]
    fn disk_full_is_detected() {
        let e = CoreError::io("x", io::Error::from(io::ErrorKind::StorageFull));
        assert_eq!(e.code(), "io.diskFull");
    }

    #[test]
    fn detail_contains_source_chain() {
        let e = CoreError::io("C:/data", io::Error::other("boom"));
        let d = e.detail();
        assert!(d.contains("C:/data"));
        assert!(d.contains("caused by: boom"));
    }
}
