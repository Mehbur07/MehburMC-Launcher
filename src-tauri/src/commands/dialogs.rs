//! Native file dialogs, opened from Rust.
//!
//! The webview never hands a file path to a command: it asks for a dialog
//! with a fixed purpose, the chosen path stays here, and the command that
//! needs it takes it exactly once. A compromised page therefore cannot make
//! the launcher read or write arbitrary files (the `dialog` capability is
//! not granted to the webview at all).

use std::path::PathBuf;

use launcher_core::CoreError;
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use super::CmdResult;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PickPurpose {
    /// `.mrpack` / CurseForge `.zip` to import.
    Modpack,
    /// Exported instance `.zip` to import.
    InstanceArchive,
    /// OptiFine installer `.jar`.
    Optifine,
    /// Mod `.jar` to upload to the MehburMC Library.
    LibraryMod,
    Skin,
    Cape,
    /// Account profile photo (`.png`).
    Avatar,
    /// Private MehburMC skin/cape uploaded by the founder (K74).
    PrivateTexture,
    /// A Java executable for the instance settings (returned, not stored).
    Java,
    /// Destination folder for "Move data folder".
    DataFolder,
    /// Save targets.
    InstanceExport,
    ConsoleLog,
    SkinExport,
}

enum Kind {
    Open(&'static str, &'static [&'static str]),
    Save(&'static str, &'static [&'static str]),
    Folder,
}

fn kind(p: PickPurpose) -> Kind {
    use PickPurpose::*;
    match p {
        Modpack => Kind::Open("Modpack", &["mrpack", "zip"]),
        InstanceArchive => Kind::Open("Zip", &["zip"]),
        Optifine => Kind::Open("OptiFine", &["jar"]),
        LibraryMod => Kind::Open("Mod", &["jar"]),
        Skin | Cape | Avatar | PrivateTexture => Kind::Open("PNG", &["png"]),
        Java if cfg!(windows) => Kind::Open("Java", &["exe"]),
        Java => Kind::Open("Java", &["*"]),
        DataFolder => Kind::Folder,
        InstanceExport => Kind::Save("Zip", &["zip"]),
        ConsoleLog => Kind::Save("Log", &["log", "txt"]),
        SkinExport => Kind::Save("PNG", &["png"]),
    }
}

/// Keeps only characters that are safe in a file name suggestion.
fn clean_file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(120)
        .collect();
    let s = s.trim().trim_matches('.').to_owned();
    if s.is_empty() { "file".into() } else { s }
}

/// Opens the dialog for `purpose`. Returns the chosen path for display
/// (`None` = cancelled); the path itself is kept for the next command.
#[tauri::command]
pub async fn pick_path(
    app: AppHandle,
    purpose: PickPurpose,
    default_name: Option<String>,
    title: Option<String>,
) -> CmdResult<Option<String>> {
    let handle = app.clone();
    let picked = tokio::task::spawn_blocking(move || {
        let mut d = handle.dialog().file();
        if let Some(w) = handle.get_webview_window("main") {
            d = d.set_parent(&w);
        }
        if let Some(t) = title {
            d = d.set_title(t.chars().take(120).collect::<String>());
        }
        let picked = match kind(purpose) {
            Kind::Open(name, exts) => d.add_filter(name, exts).blocking_pick_file(),
            Kind::Save(name, exts) => {
                let file = clean_file_name(default_name.as_deref().unwrap_or("file"));
                d.add_filter(name, exts)
                    .set_file_name(file)
                    .blocking_save_file()
            }
            Kind::Folder => d.blocking_pick_folder(),
        };
        picked.and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| CoreError::io("", std::io::Error::other(e.to_string())).to_payload())?;

    let Some(path) = picked else {
        return Ok(None);
    };
    let shown = path.display().to_string();
    if purpose != PickPurpose::Java {
        app.state::<AppState>()
            .picks
            .lock()
            .expect("picks lock")
            .insert(purpose, path);
    }
    Ok(Some(shown))
}

/// The path picked for `purpose`, consumed.
pub fn take_pick(state: &State<'_, AppState>, purpose: PickPurpose) -> CmdResult<PathBuf> {
    state
        .picks
        .lock()
        .expect("picks lock")
        .remove(&purpose)
        .ok_or_else(|| CoreError::InvalidSetting("no file selected".into()).to_payload())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_suggestions_are_sanitised() {
        assert_eq!(clean_file_name("My Pack.mehbur.zip"), "My Pack.mehbur.zip");
        assert_eq!(clean_file_name(r"..\..\evil/x.zip"), r"_.._evil_x.zip");
        assert_eq!(clean_file_name("..."), "file");
        assert_eq!(clean_file_name("a:b*c?.log"), "a_b_c_.log");
    }
}
