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

    #[error("network error while fetching {url}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("server returned HTTP {status} for {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("download blocked: {url} is not on the allowlist")]
    UrlNotAllowed { url: String },

    #[error("checksum mismatch for {}: expected {expected}, got {actual}", path.display())]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("not enough disk space in {}: need {needed} bytes, {available} available", path.display())]
    DiskFull {
        path: PathBuf,
        needed: u64,
        available: u64,
    },

    #[error("operation cancelled")]
    Cancelled,

    #[error("Minecraft version {0} was not found")]
    VersionNotFound(String),

    #[error("version {id} is invalid: {reason}")]
    InvalidVersion { id: String, reason: String },

    #[error("no Java {major} runtime is available")]
    JavaNotFound { major: u32 },

    #[error("Java {major} is not offered for {os}/{arch}")]
    JavaUnavailable {
        major: u32,
        os: String,
        arch: String,
    },

    #[error("invalid archive {}", path.display())]
    Archive {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("archive {} contains an unsafe entry: {entry}", path.display())]
    UnsafeArchiveEntry { path: PathBuf, entry: String },

    #[error("invalid offline player name: {0}")]
    InvalidPlayerName(String),

    #[error("the command line is too long for Java {major}")]
    CommandLineTooLong { major: u32 },

    #[error("failed to start {}", program.display())]
    Spawn {
        program: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("instance {0} was not found")]
    InstanceNotFound(String),

    #[error("instance {name} is busy (running or being prepared)")]
    InstanceBusy { name: String },

    #[error("invalid instance: {0}")]
    InvalidInstance(String),

    #[error("no account selected")]
    NoAccount,

    #[error("account {0} was not found")]
    AccountNotFound(String),

    #[error("no {loader} version is available for Minecraft {mc}")]
    LoaderUnavailable { loader: String, mc: String },

    #[error("installing {loader} failed: {reason}")]
    LoaderInstall { loader: String, reason: String },

    #[error("{} is not an OptiFine jar", path.display())]
    OptifineInvalid { path: PathBuf },

    #[error("the OptiFine jar for {mc} {edition} has not been imported")]
    OptifineMissing { mc: String, edition: String },

    #[error("invalid modpack: {0}")]
    ModpackInvalid(String),

    #[error("a CurseForge API key is required (or it was rejected)")]
    CurseForgeKey,

    #[error("{project} is not available for {loader} on Minecraft {mc}")]
    AddonUnavailable {
        project: String,
        loader: String,
        mc: String,
    },

    #[error("invalid skin or cape image: {0}")]
    SkinInvalid(String),

    #[error("skin or cape {0} was not found")]
    SkinNotFound(String),

    #[error("no Minecraft player named {0}")]
    PlayerNotFound(String),

    #[error("cannot move the data folder: {0}")]
    DataMove(String),
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
            Self::Network { source, .. } if source.is_timeout() => "net.timeout",
            Self::Network { .. } => "net.unreachable",
            Self::HttpStatus { .. } => "net.httpStatus",
            Self::UrlNotAllowed { .. } => "net.notAllowed",
            Self::HashMismatch { .. } => "download.hashMismatch",
            Self::DiskFull { .. } => "io.diskFull",
            Self::Cancelled => "task.cancelled",
            Self::VersionNotFound(_) => "version.notFound",
            Self::InvalidVersion { .. } => "version.invalid",
            Self::JavaNotFound { .. } => "java.notFound",
            Self::JavaUnavailable { .. } => "java.unavailable",
            Self::Archive { .. } => "archive.invalid",
            Self::UnsafeArchiveEntry { .. } => "archive.unsafe",
            Self::InvalidPlayerName(_) => "auth.invalidName",
            Self::CommandLineTooLong { .. } => "launch.commandTooLong",
            Self::Spawn { .. } => "launch.spawnFailed",
            Self::InstanceNotFound(_) => "instance.notFound",
            Self::InstanceBusy { .. } => "instance.busy",
            Self::InvalidInstance(_) => "instance.invalid",
            Self::NoAccount => "account.none",
            Self::AccountNotFound(_) => "account.notFound",
            Self::LoaderUnavailable { .. } => "loader.unavailable",
            Self::LoaderInstall { .. } => "loader.installFailed",
            Self::OptifineInvalid { .. } => "loader.optifineInvalid",
            Self::OptifineMissing { .. } => "loader.optifineMissing",
            Self::AddonUnavailable { .. } => "loader.addonUnavailable",
            Self::ModpackInvalid(_) => "content.modpackInvalid",
            Self::CurseForgeKey => "content.curseforgeKey",
            Self::SkinInvalid(_) => "skin.invalid",
            Self::SkinNotFound(_) => "skin.notFound",
            Self::PlayerNotFound(_) => "skin.playerNotFound",
            Self::DataMove(_) => "paths.moveFailed",
        }
    }

    /// Interpolation parameters for the translated message.
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        let mut put = |k: &str, v: String| {
            p.insert(k.to_owned(), v);
        };
        match self {
            Self::DataDirNotWritable { path, .. }
            | Self::Io { path, .. }
            | Self::Json { path, .. }
            | Self::Archive { path, .. } => put("path", path.display().to_string()),
            Self::InvalidRedirect { path, reason } => {
                put("path", path.display().to_string());
                put("reason", reason.clone());
            }
            Self::InvalidSetting(what) => put("setting", what.clone()),
            Self::Network { url, .. } | Self::UrlNotAllowed { url } => put("url", url.clone()),
            Self::HttpStatus { url, status } => {
                put("url", url.clone());
                put("status", status.to_string());
            }
            Self::HashMismatch { path, .. } => put("path", path.display().to_string()),
            Self::DiskFull {
                path,
                needed,
                available,
            } => {
                put("path", path.display().to_string());
                put("needed", format_mb(*needed));
                put("available", format_mb(*available));
            }
            Self::VersionNotFound(id) => put("version", id.clone()),
            Self::InvalidVersion { id, reason } => {
                put("version", id.clone());
                put("reason", reason.clone());
            }
            Self::JavaNotFound { major } | Self::CommandLineTooLong { major } => {
                put("major", major.to_string())
            }
            Self::JavaUnavailable { major, os, arch } => {
                put("major", major.to_string());
                put("os", os.clone());
                put("arch", arch.clone());
            }
            Self::UnsafeArchiveEntry { path, entry } => {
                put("path", path.display().to_string());
                put("entry", entry.clone());
            }
            Self::InvalidPlayerName(name) => put("name", name.clone()),
            Self::Spawn { program, .. } => put("path", program.display().to_string()),
            Self::InstanceNotFound(id) | Self::AccountNotFound(id) => put("id", id.clone()),
            Self::InstanceBusy { name } => put("name", name.clone()),
            Self::InvalidInstance(reason) => put("reason", reason.clone()),
            Self::LoaderUnavailable { loader, mc } => {
                put("loader", loader.clone());
                put("mc", mc.clone());
            }
            Self::LoaderInstall { loader, reason } => {
                put("loader", loader.clone());
                put("reason", reason.clone());
            }
            Self::OptifineInvalid { path } => put("path", path.display().to_string()),
            Self::OptifineMissing { mc, edition } => {
                put("mc", mc.clone());
                put("edition", edition.clone());
            }
            Self::AddonUnavailable {
                project,
                loader,
                mc,
            } => {
                put("project", project.clone());
                put("loader", loader.clone());
                put("mc", mc.clone());
            }
            Self::ModpackInvalid(reason) | Self::SkinInvalid(reason) | Self::DataMove(reason) => {
                put("reason", reason.clone())
            }
            Self::SkinNotFound(id) => put("id", id.clone()),
            Self::PlayerNotFound(name) => put("name", name.clone()),
            Self::NoDataDir | Self::Cancelled | Self::NoAccount | Self::CurseForgeKey => {}
        }
        p
    }

    /// Whether retrying the same request may succeed (network blips, 5xx, 429,
    /// corrupted transfer).
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network { .. } | Self::HashMismatch { .. } => true,
            Self::HttpStatus { status, .. } => *status == 429 || *status == 408 || *status >= 500,
            Self::Io { source, .. } => matches!(
                source.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::Interrupted
            ),
            _ => false,
        }
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

fn format_mb(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_048_576.0)
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
