//! IPC commands. Each one validates input, calls launcher-core and maps the
//! error to an [`ErrorPayload`]; no business logic lives here.

pub mod accounts;
pub mod content;
pub mod data;
pub mod dialogs;
pub mod files;
pub mod friends;
pub mod home;
pub mod instances;
pub mod library;
pub mod loaders;
pub mod play;
pub mod servers;
pub mod skins;
pub mod textures;
pub mod update;

use launcher_core::{CoreError, ErrorPayload, PathsInfo, Settings};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use crate::state::AppState;

pub type CmdResult<T> = Result<T, ErrorPayload>;

/// Runs blocking filesystem work off the async runtime's worker threads.
pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> launcher_core::Result<T> + Send + 'static,
) -> CmdResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CoreError::io("", std::io::Error::other(e.to_string())).to_payload())?
        .map_err(Into::into)
}

pub fn open_path(app: &AppHandle, dir: &std::path::Path) -> CmdResult<()> {
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| CoreError::io(dir, std::io::Error::other(e.to_string())).to_payload())
}

/// Everything the UI needs for its first render, in one round trip.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bootstrap {
    pub app_name: String,
    pub version: String,
    pub paths: Option<PathsInfo>,
    pub settings: Settings,
    pub startup_error: Option<ErrorPayload>,
    /// Set on the first start after an update (see `note_launcher_version`).
    pub updated_from: Option<String>,
}

#[tauri::command]
pub fn get_bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let settings = state.settings();
    Bootstrap {
        app_name: launcher_core::LAUNCHER_NAME.to_owned(),
        version: launcher_core::LAUNCHER_VERSION.to_owned(),
        paths: state.paths.as_ref().map(|p| p.info()),
        settings,
        startup_error: state.startup_error.clone(),
        updated_from: state.updated_from.clone(),
    }
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<Settings> {
    let paths = state.usable_paths()?;
    settings.save(paths)?;
    tracing::info!("settings saved");
    *state.settings.lock().expect("settings lock") = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub fn open_data_dir(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    open_path(&app, state.usable_paths()?.content())
}

#[tauri::command]
pub async fn list_versions(
    state: State<'_, AppState>,
) -> CmdResult<Vec<launcher_core::version::ManifestEntry>> {
    let launcher = state.launcher()?.clone();
    let m = launcher_core::version::manifest(&launcher.ctx).await?;
    Ok(m.versions)
}

#[tauri::command]
pub async fn list_java(
    state: State<'_, AppState>,
) -> CmdResult<Vec<launcher_core::java::JavaInstall>> {
    let launcher = state.launcher()?.clone();
    blocking(move || {
        let mut all = launcher_core::java::managed(&launcher.ctx);
        all.extend(launcher_core::java::scan_system());
        Ok(all)
    })
    .await
}

/// Writes text (console export) to the path picked with
/// `pick_path(consoleLog)`.
#[tauri::command]
pub async fn save_text_file(state: State<'_, AppState>, contents: String) -> CmdResult<()> {
    let p = dialogs::take_pick(&state, dialogs::PickPurpose::ConsoleLog)?;
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "log" | "txt"));
    if !ext_ok || !p.is_absolute() {
        return Err(CoreError::InvalidSetting("path".into()).to_payload());
    }
    blocking(move || launcher_core::fsutil::write_atomic(&p, contents.as_bytes())).await
}
