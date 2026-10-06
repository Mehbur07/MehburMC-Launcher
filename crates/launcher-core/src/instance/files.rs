//! Content folders of an instance (mods, packs, worlds, screenshots, logs).
//! Entry names coming from the UI are validated to stay inside the folder.

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::InstanceStore;
use crate::error::{CoreError, Result};

pub const DISABLED_SUFFIX: &str = ".disabled";
/// Upper bound for log viewing (the tail is returned if larger).
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Folder {
    Root,
    Mods,
    ResourcePacks,
    ShaderPacks,
    Saves,
    Screenshots,
    Logs,
    CrashReports,
}

impl Folder {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Root => "",
            Self::Mods => "mods",
            Self::ResourcePacks => "resourcepacks",
            Self::ShaderPacks => "shaderpacks",
            Self::Saves => "saves",
            Self::Screenshots => "screenshots",
            Self::Logs => "logs",
            Self::CrashReports => "crash-reports",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileEntry {
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number")]
    pub modified: u64,
    pub is_dir: bool,
    /// `false` for `*.disabled` mods/packs.
    pub enabled: bool,
}

pub(crate) fn checked_name(name: &str) -> Result<&str> {
    let bad = name.is_empty()
        || name == "."
        || name.contains("..")
        || name.contains(['/', '\\', ':', '\0']);
    if bad {
        Err(CoreError::InvalidInstance(format!("file name {name:?}")))
    } else {
        Ok(name)
    }
}

pub fn folder_path(store: &InstanceStore, id: &str, folder: Folder) -> Result<PathBuf> {
    let dir = store.dir(id)?;
    Ok(match folder {
        Folder::Root => dir,
        f => dir.join(f.dir_name()),
    })
}

pub fn list(store: &InstanceStore, id: &str, folder: Folder) -> Result<Vec<FileEntry>> {
    store.get(id)?;
    let dir = folder_path(store, id, folder)?;
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(vec![]);
    };
    let mut out: Vec<FileEntry> = rd
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            Some(FileEntry {
                enabled: !name.ends_with(DISABLED_SUFFIX),
                size: if meta.is_dir() {
                    dir_size(&e.path())
                } else {
                    meta.len()
                },
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or_default(),
                is_dir: meta.is_dir(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| match folder {
        Folder::Saves | Folder::Screenshots | Folder::Logs | Folder::CrashReports => {
            b.modified.cmp(&a.modified)
        }
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

fn dir_size(dir: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Enables/disables a mod or pack by toggling the `.disabled` suffix.
pub fn toggle(store: &InstanceStore, id: &str, folder: Folder, name: &str) -> Result<FileEntry> {
    let name = checked_name(name)?;
    let dir = folder_path(store, id, folder)?;
    let from = dir.join(name);
    if !from.exists() {
        return Err(CoreError::io(&from, std::io::ErrorKind::NotFound.into()));
    }
    let new_name = match name.strip_suffix(DISABLED_SUFFIX) {
        Some(base) => base.to_owned(),
        None => format!("{name}{DISABLED_SUFFIX}"),
    };
    let to = dir.join(&new_name);
    // `rename` replaces an existing target on Windows: with both `a.jar` and
    // `a.jar.disabled` present, toggling would silently delete one of them.
    if to.exists() {
        return Err(CoreError::io(
            &to,
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "both the enabled and the disabled file exist",
            ),
        ));
    }
    std::fs::rename(&from, &to).map_err(|e| CoreError::io(&to, e))?;
    list(store, id, folder)?
        .into_iter()
        .find(|e| e.name == new_name)
        .ok_or_else(|| CoreError::io(&to, std::io::ErrorKind::NotFound.into()))
}

pub fn delete(store: &InstanceStore, id: &str, folder: Folder, name: &str) -> Result<()> {
    if folder == Folder::Root {
        return Err(CoreError::InvalidInstance("cannot delete from root".into()));
    }
    let name = checked_name(name)?;
    let path = folder_path(store, id, folder)?.join(name);
    let meta = std::fs::symlink_metadata(&path).map_err(|e| CoreError::io(&path, e))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(&path)
    } else {
        std::fs::remove_file(&path)
    }
    .map_err(|e| CoreError::io(&path, e))
}

/// Reads a log or crash report (last `MAX_LOG_BYTES` if it is larger).
/// Path of a file directly inside an instance folder (name validated).
pub fn file_path(store: &InstanceStore, id: &str, folder: Folder, name: &str) -> Result<PathBuf> {
    Ok(folder_path(store, id, folder)?.join(checked_name(name)?))
}

pub fn read_text(store: &InstanceStore, id: &str, folder: Folder, name: &str) -> Result<String> {
    if !matches!(folder, Folder::Logs | Folder::CrashReports) {
        return Err(CoreError::InvalidInstance("not a log folder".into()));
    }
    let name = checked_name(name)?;
    if name.ends_with(".gz") {
        return Err(CoreError::InvalidInstance(
            "compressed logs are not supported yet".into(),
        ));
    }
    let path = folder_path(store, id, folder)?.join(name);
    let bytes = std::fs::read(&path).map_err(|e| CoreError::io(&path, e))?;
    let start = bytes.len().saturating_sub(MAX_LOG_BYTES as usize);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{new, store};
    use super::*;

    #[test]
    fn list_toggle_delete() {
        let (_t, s) = store();
        let a = s.create(new("A")).unwrap();
        let mods = folder_path(&s, &a.id, Folder::Mods).unwrap();
        std::fs::write(mods.join("sodium.jar"), b"1234").unwrap();
        std::fs::write(mods.join(".hidden"), b"").unwrap();

        let l = list(&s, &a.id, Folder::Mods).unwrap();
        assert_eq!(l.len(), 1);
        assert!(l[0].enabled && l[0].size == 4);

        let e = toggle(&s, &a.id, Folder::Mods, "sodium.jar").unwrap();
        assert_eq!(e.name, "sodium.jar.disabled");
        assert!(!e.enabled);
        let e = toggle(&s, &a.id, Folder::Mods, "sodium.jar.disabled").unwrap();
        assert!(e.enabled);

        delete(&s, &a.id, Folder::Mods, "sodium.jar").unwrap();
        assert!(list(&s, &a.id, Folder::Mods).unwrap().is_empty());
    }

    #[test]
    fn rejects_traversal_names() {
        let (_t, s) = store();
        let a = s.create(new("A")).unwrap();
        for bad in ["../instance.json", "..", "a/b", "C:x", ""] {
            assert!(delete(&s, &a.id, Folder::Mods, bad).is_err(), "{bad}");
            assert!(read_text(&s, &a.id, Folder::Logs, bad).is_err(), "{bad}");
        }
        assert!(delete(&s, &a.id, Folder::Root, "instance.json").is_err());
        assert!(read_text(&s, &a.id, Folder::Mods, "x.jar").is_err());
    }

    #[test]
    fn reads_logs() {
        let (_t, s) = store();
        let a = s.create(new("A")).unwrap();
        let logs = folder_path(&s, &a.id, Folder::Logs).unwrap();
        std::fs::write(logs.join("latest.log"), "hello\n").unwrap();
        assert_eq!(
            read_text(&s, &a.id, Folder::Logs, "latest.log").unwrap(),
            "hello\n"
        );
    }
}
