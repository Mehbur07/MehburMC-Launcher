//! `launcher/settings.json` — versioned, forward-compatible launcher settings.
//!
//! Unknown fields are ignored and missing fields fall back to defaults, so old
//! and new launcher versions can share a file. A corrupt file is backed up and
//! replaced by defaults instead of blocking startup.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;
use crate::paths::Paths;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Language {
    /// Follow the OS language (Turkish if the system is Turkish, else English).
    #[default]
    System,
    Tr,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Accent {
    #[default]
    Cyan,
    Magenta,
    Green,
    Purple,
}

/// What the launcher window does once the game has started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LaunchBehavior {
    #[default]
    Minimize,
    Close,
    KeepOpen,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub schema_version: u32,
    pub language: Language,
    pub accent: Accent,
    /// Animated grid/particle background.
    pub background_effects: bool,
    /// Verbose (debug-level) launcher logging.
    pub debug_logging: bool,
    pub download_concurrency: u32,
    pub default_memory_mb: u32,
    pub launch_behavior: LaunchBehavior,
    /// The user's own CurseForge API key (needed for CurseForge modpacks).
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curseforge_api_key: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            language: Language::System,
            accent: Accent::Cyan,
            background_effects: true,
            debug_logging: false,
            download_concurrency: 8,
            default_memory_mb: 4096,
            launch_behavior: LaunchBehavior::Minimize,
            curseforge_api_key: None,
        }
    }
}

pub const DOWNLOAD_CONCURRENCY_RANGE: std::ops::RangeInclusive<u32> = 1..=32;
pub const MEMORY_MB_RANGE: std::ops::RangeInclusive<u32> = 512..=65536;

impl Settings {
    /// Loads settings; a missing file yields defaults, a corrupt one is
    /// renamed to `settings.json.corrupt-<unix>` and defaults are used.
    pub fn load(paths: &Paths) -> Result<Self> {
        let path = paths.settings_file();
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(CoreError::io(&path, e)),
        };
        match serde_json::from_slice::<Self>(&raw) {
            Ok(mut s) => {
                s.migrate();
                s.clamp();
                Ok(s)
            }
            Err(err) => {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or_default();
                let backup = path.with_file_name(format!("settings.json.corrupt-{stamp}"));
                tracing::warn!(%err, backup = %backup.display(), "settings file is corrupt, using defaults");
                fs::rename(&path, &backup).map_err(|e| CoreError::io(&path, e))?;
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        self.validate()?;
        write_json_atomic(&paths.settings_file(), self)
    }

    /// Rejects values the UI should never send.
    pub fn validate(&self) -> Result<()> {
        if !DOWNLOAD_CONCURRENCY_RANGE.contains(&self.download_concurrency) {
            return Err(CoreError::InvalidSetting("downloadConcurrency".into()));
        }
        if !MEMORY_MB_RANGE.contains(&self.default_memory_mb) {
            return Err(CoreError::InvalidSetting("defaultMemoryMb".into()));
        }
        Ok(())
    }

    fn migrate(&mut self) {
        // Future schema migrations go here, keyed on `schema_version`.
        self.schema_version = SCHEMA_VERSION;
    }

    fn clamp(&mut self) {
        self.download_concurrency = self.download_concurrency.clamp(
            *DOWNLOAD_CONCURRENCY_RANGE.start(),
            *DOWNLOAD_CONCURRENCY_RANGE.end(),
        );
        self.default_memory_mb = self
            .default_memory_mb
            .clamp(*MEMORY_MB_RANGE.start(), *MEMORY_MB_RANGE.end());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        (tmp, paths)
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_t, paths) = temp_paths();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
    }

    #[test]
    fn roundtrip() {
        let (_t, paths) = temp_paths();
        let s = Settings {
            accent: Accent::Purple,
            language: Language::Tr,
            ..Settings::default()
        };
        s.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), s);
    }

    #[test]
    fn partial_and_unknown_fields_are_tolerated() {
        let (_t, paths) = temp_paths();
        fs::write(
            paths.settings_file(),
            br#"{"accent":"green","someFutureField":42}"#,
        )
        .unwrap();
        let s = Settings::load(&paths).unwrap();
        assert_eq!(s.accent, Accent::Green);
        assert_eq!(s.download_concurrency, 8);
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let (_t, paths) = temp_paths();
        fs::write(paths.settings_file(), b"{not json").unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
        assert!(!paths.settings_file().exists());
        let backups = fs::read_dir(paths.launcher_dir())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn out_of_range_values_are_rejected_on_save_and_clamped_on_load() {
        let (_t, paths) = temp_paths();
        let bad = Settings {
            download_concurrency: 0,
            ..Settings::default()
        };
        assert_eq!(bad.save(&paths).unwrap_err().code(), "settings.invalid");

        fs::write(paths.settings_file(), br#"{"defaultMemoryMb":1}"#).unwrap();
        assert_eq!(Settings::load(&paths).unwrap().default_memory_mb, 512);
    }
}
