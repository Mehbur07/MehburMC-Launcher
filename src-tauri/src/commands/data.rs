//! Data folder relocation (K4) and app restart.

use std::time::{Duration, Instant};

use launcher_core::CoreError;
use launcher_core::datamove;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;

use super::{CmdResult, blocking};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoveProgress {
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

/// Copies all content to `dest`, switches the redirect and deletes the old
/// copy. Returns how many old files could not be deleted. The launcher must
/// be restarted afterwards.
#[tauri::command]
pub async fn move_data_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<usize> {
    let dest = super::dialogs::take_pick(&state, super::dialogs::PickPurpose::DataFolder)?;
    let launcher = state.launcher()?.clone();
    if launcher.tasks.list().iter().any(|t| t.status.is_active()) {
        return Err(CoreError::DataMove("a game or download is running".into()).into());
    }
    let paths = state.usable_paths()?.clone();
    blocking(move || {
        let last = std::sync::Mutex::new(Instant::now() - Duration::from_secs(1));
        let left = datamove::move_content(&paths, &dest, &|done, total| {
            let mut l = last.lock().expect("progress lock");
            if done == total || l.elapsed() >= Duration::from_millis(100) {
                *l = Instant::now();
                let _ = app.emit("core://datamove", MoveProgress { done, total });
            }
        })?;
        Ok(left.len())
    })
    .await
}

/// Selects the default (non-redirected) location as the move target
/// ("move back"); returns it for display.
#[tauri::command]
pub fn default_data_folder(state: State<'_, AppState>) -> CmdResult<String> {
    let mc = state.usable_paths()?.mc().to_path_buf();
    let shown = mc.display().to_string();
    state
        .picks
        .lock()
        .expect("picks lock")
        .insert(super::dialogs::PickPurpose::DataFolder, mc);
    Ok(shown)
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}
