//! Mod groups of an instance (K91): named sets of mods switched on or off
//! together, like the Switch mod. Only the groups are stored here, in the
//! instance folder; switching uses `set_content_enabled`, and a group's state
//! is read from the mods themselves.
//!
//! A member is remembered by its Modrinth project id, its own mod id and its
//! file name, so it stays in the group when an update renames the file or
//! Modrinth cannot be reached.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

pub const FILE: &str = "mod-groups.json";

const MAX_GROUPS: usize = 50;
const MAX_NAME_CHARS: usize = 40;
const MAX_MEMBERS: usize = 2000;
const MAX_FIELD_CHARS: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GroupMod {
    /// File name without `.disabled`.
    pub file: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub mod_id: Option<String>,
    /// Shown when the mod is gone from the folder.
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModGroup {
    pub id: String,
    pub name: String,
    pub mods: Vec<GroupMod>,
}

fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// The instance's groups; none when the file is missing or unreadable.
pub fn load(dir: &Path) -> Vec<ModGroup> {
    let Ok(bytes) = std::fs::read(path(dir)) else {
        return Vec::new();
    };
    match serde_json::from_slice::<Vec<ModGroup>>(&bytes) {
        Ok(groups) => groups,
        Err(e) => {
            tracing::warn!(error = %e, "mod groups unreadable, starting empty");
            Vec::new()
        }
    }
}

fn bad(reason: &str) -> CoreError {
    CoreError::InvalidSetting(format!("mod groups: {reason}"))
}

fn clean_field(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().chars().take(MAX_FIELD_CHARS).collect::<String>())
        .filter(|v| !v.is_empty() && !v.chars().any(char::is_control))
}

/// Checks and tidies the groups (trimmed names, no duplicate members), then
/// saves them; returns what was saved.
pub fn save(dir: &Path, groups: Vec<ModGroup>) -> Result<Vec<ModGroup>> {
    if groups.len() > MAX_GROUPS {
        return Err(bad("too many groups"));
    }
    let mut ids = HashSet::new();
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        let name = g.name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(bad("bad name"));
        }
        let id = g.id.trim().to_owned();
        if id.is_empty()
            || id.len() > 64
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !ids.insert(id.clone())
        {
            return Err(bad("bad id"));
        }
        if g.mods.len() > MAX_MEMBERS {
            return Err(bad("too many mods"));
        }
        let mut seen = HashSet::new();
        let mut mods = Vec::with_capacity(g.mods.len());
        for m in g.mods {
            let file = m.file.trim().to_owned();
            if file.is_empty()
                || file.len() > MAX_FIELD_CHARS
                || file.contains(['/', '\\'])
                || file.chars().any(char::is_control)
                || file.ends_with(super::files::DISABLED_SUFFIX)
            {
                return Err(bad("bad file name"));
            }
            if seen.insert(file.to_ascii_lowercase()) {
                mods.push(GroupMod {
                    file,
                    project_id: clean_field(m.project_id),
                    mod_id: clean_field(m.mod_id),
                    title: clean_field(m.title),
                });
            }
        }
        out.push(ModGroup { id, name, mods });
    }
    let bytes = serde_json::to_vec_pretty(&out).expect("groups serialize");
    write_atomic(&path(dir), &bytes)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, name: &str, files: &[&str]) -> ModGroup {
        ModGroup {
            id: id.into(),
            name: name.into(),
            mods: files
                .iter()
                .map(|f| GroupMod {
                    file: (*f).into(),
                    project_id: Some("AANobbMI".into()),
                    mod_id: Some(" sodium ".into()),
                    title: None,
                })
                .collect(),
        }
    }

    #[test]
    fn saves_tidies_and_loads() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).is_empty());
        let saved = save(
            tmp.path(),
            vec![group(
                "g-1",
                "  PvP   modları ",
                &["a.jar", "A.JAR", "b.jar"],
            )],
        )
        .unwrap();
        assert_eq!(saved[0].name, "PvP modları");
        assert_eq!(saved[0].mods.len(), 2);
        assert_eq!(saved[0].mods[0].mod_id.as_deref(), Some("sodium"));
        assert_eq!(load(tmp.path()), saved);
    }

    #[test]
    fn refuses_bad_input_and_survives_a_broken_file() {
        let tmp = tempfile::tempdir().unwrap();
        for g in [
            group("g", "   ", &[]),
            group("g", &"x".repeat(41), &[]),
            group("g/../x", "A", &[]),
            group("g", "A", &["../evil.jar"]),
            group("g", "A", &["a.jar.disabled"]),
        ] {
            assert!(save(tmp.path(), vec![g]).is_err());
        }
        assert!(save(tmp.path(), vec![group("g", "A", &[]), group("g", "B", &[])]).is_err());
        assert_eq!(
            save(tmp.path(), vec![group("g", " ", &[])])
                .unwrap_err()
                .code(),
            "settings.invalid"
        );
        std::fs::write(tmp.path().join(FILE), b"{not json").unwrap();
        assert!(load(tmp.path()).is_empty());
    }
}
