//! IPC commands. Each one validates input, calls launcher-core and maps the
//! error to an [`ErrorPayload`]; no business logic lives here.

use launcher_core::{CoreError, ErrorPayload, PathsInfo, Settings};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use crate::state::AppState;

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
    pub msa_configured: bool,
}

#[tauri::command]
pub fn get_bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let settings = state.settings.lock().expect("settings lock").clone();
    Bootstrap {
        app_name: launcher_core::LAUNCHER_NAME.to_owned(),
        version: launcher_core::LAUNCHER_VERSION.to_owned(),
        paths: state.paths.as_ref().map(|p| p.info()),
        msa_configured: settings.msa_client_id().is_some(),
        settings,
        startup_error: state.startup_error.clone(),
    }
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, ErrorPayload> {
    let paths = state.usable_paths()?;
    settings.save(paths)?;
    tracing::info!("settings saved");
    *state.settings.lock().expect("settings lock") = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub fn open_data_dir(app: AppHandle, state: State<'_, AppState>) -> Result<(), ErrorPayload> {
    let paths = state.usable_paths()?;
    let dir = paths.content();
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| CoreError::io(dir, std::io::Error::other(e.to_string())).to_payload())
}
