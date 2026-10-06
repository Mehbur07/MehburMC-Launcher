//! MehburMC Library (K72): scan, submit, list, withdraw, report, install.

use launcher_core::CoreError;
use launcher_core::content::scan::{self, ScanReport, Verdict};
use launcher_core::friends::library::{LibraryInstall, LibraryMod, LibraryReportReason};
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use super::CmdResult;
use super::dialogs::{PickPurpose, take_pick};
use crate::state::AppState;

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryScan {
    pub file_name: String,
    pub report: ScanReport,
}

/// Scans the jar picked with `pick_path("libraryMod")`. A jar that is not
/// blocked is kept for `library_submit`.
#[tauri::command]
pub async fn library_scan_pick(state: State<'_, AppState>) -> CmdResult<LibraryScan> {
    let path = take_pick(&state, PickPurpose::LibraryMod)?;
    state.launcher()?;
    let p = path.clone();
    let report = tauri::async_runtime::spawn_blocking(move || scan::scan_file(&p))
        .await
        .map_err(|e| CoreError::io(&path, std::io::Error::other(e.to_string())).to_payload())??;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    *state.library_upload.lock().expect("library upload lock") =
        (report.verdict != Verdict::Block).then(|| (path, report.sha1.clone()));
    Ok(LibraryScan { file_name, report })
}

/// Uploads the scanned jar for admin review; returns its library id.
#[tauri::command]
pub async fn library_submit(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> CmdResult<i64> {
    let l = state.launcher()?.clone();
    let (path, sha1) = state
        .library_upload
        .lock()
        .expect("library upload lock")
        .take()
        .ok_or_else(|| CoreError::InvalidSetting("no file selected".into()).to_payload())?;
    Ok(l.submit_library_mod(&path, &sha1, &name, &description)
        .await?
        .0)
}

/// Approved mods and the caller's own uploads.
#[tauri::command]
pub async fn library_mods(state: State<'_, AppState>) -> CmdResult<Vec<LibraryMod>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.library_mods().await?)
}

#[tauri::command]
pub async fn library_withdraw(state: State<'_, AppState>, library_id: i64) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.withdraw_library_mod(library_id).await?)
}

#[tauri::command]
pub async fn library_report(
    state: State<'_, AppState>,
    library_id: i64,
    reason: LibraryReportReason,
    note: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends
        .report_library_mod(library_id, reason, &note)
        .await?)
}

#[tauri::command]
pub async fn library_install(
    state: State<'_, AppState>,
    library_id: i64,
    instance_id: String,
    accept_warnings: bool,
) -> CmdResult<LibraryInstall> {
    let l = state.launcher()?.clone();
    Ok(
        l.install_library_mod(library_id, &instance_id, accept_warnings)
            .await?,
    )
}
