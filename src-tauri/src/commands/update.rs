//! Signed self-update (tauri-plugin-updater). The channel is a GitHub
//! release `latest.json`; while the repository is private it is unreachable
//! and the check reports `unavailable` instead of failing (ARCHITECTURE R7).

use launcher_core::CoreError;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;

use super::CmdResult;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateStatus {
    UpToDate,
    Available,
    /// Channel unreachable (offline, private repository, no release yet).
    Unavailable,
    /// Portable copies update by replacing the folder; the updater would
    /// run the installer instead.
    Portable,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppUpdate {
    pub status: UpdateStatus,
    pub current: String,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle, state: State<'_, AppState>) -> CmdResult<AppUpdate> {
    let current = app.package_info().version.to_string();
    if state
        .paths
        .as_ref()
        .is_some_and(|p| p.mode() == launcher_core::DataMode::Portable)
    {
        return Ok(AppUpdate {
            status: UpdateStatus::Portable,
            current,
            version: None,
            notes: None,
        });
    }
    let result = match app.updater() {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    let (status, update) = match result {
        Ok(Some(u)) => (UpdateStatus::Available, Some(u)),
        Ok(None) => (UpdateStatus::UpToDate, None),
        Err(e) => {
            tracing::info!(error = %e, "update channel unavailable");
            (UpdateStatus::Unavailable, None)
        }
    };
    let info = AppUpdate {
        status,
        current,
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.as_ref().and_then(|u| u.body.clone()),
    };
    *state.pending_update.lock().expect("update lock") = update;
    Ok(info)
}

/// Downloads, verifies (minisign) and installs the update found by the last
/// check, then restarts. Progress on `core://update` as `[done, total]`.
#[tauri::command]
pub async fn install_app_update(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let update = state
        .pending_update
        .lock()
        .expect("update lock")
        .take()
        .ok_or_else(|| CoreError::InvalidSetting("update".into()).to_payload())?;
    if let Ok(l) = state.launcher()
        && l.tasks.list().iter().any(|t| t.status.is_active())
    {
        return Err(CoreError::InstanceBusy {
            name: "MehburMC Launcher".into(),
        }
        .into());
    }
    let mut done = 0u64;
    let emitter = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = emitter.emit("core://update", (done, total.unwrap_or(0)));
            },
            || {},
        )
        .await
        .map_err(|e| {
            CoreError::LoaderInstall {
                loader: "MehburMC Launcher".into(),
                reason: e.to_string(),
            }
            .to_payload()
        })?;
    tracing::info!(version = %update.version, "update installed, restarting");
    app.restart();
}
