//! MehburMC Library (K72, K88): scan, submit, list, withdraw, report, install
//! — mods, modpacks, resource packs and shaders.

use launcher_core::CoreError;
use launcher_core::content::scan::{self, LibraryKind, ScanReport, Verdict};
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

/// The file dialog each kind opens with `pick_path`.
fn purpose(kind: LibraryKind) -> PickPurpose {
    match kind {
        LibraryKind::Mod => PickPurpose::LibraryMod,
        LibraryKind::Modpack => PickPurpose::LibraryModpack,
        LibraryKind::ResourcePack => PickPurpose::LibraryResourcePack,
        LibraryKind::Shader => PickPurpose::LibraryShader,
    }
}

/// Scans the file picked for `kind`. A file that is not blocked is kept
/// for `library_submit`.
#[tauri::command]
pub async fn library_scan_pick(
    state: State<'_, AppState>,
    kind: LibraryKind,
) -> CmdResult<LibraryScan> {
    let path = take_pick(&state, purpose(kind))?;
    state.launcher()?;
    let p = path.clone();
    let report = tauri::async_runtime::spawn_blocking(move || scan::scan_file_as(&p, kind))
        .await
        .map_err(|e| CoreError::io(&path, std::io::Error::other(e.to_string())).to_payload())??;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    *state.library_upload.lock().expect("library upload lock") =
        (report.verdict != Verdict::Block).then(|| (path, report.sha1.clone(), kind));
    Ok(LibraryScan { file_name, report })
}

/// Uploads the scanned file for admin review; returns its library id.
#[tauri::command]
pub async fn library_submit(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> CmdResult<i64> {
    let l = state.launcher()?.clone();
    let (path, sha1, kind) = state
        .library_upload
        .lock()
        .expect("library upload lock")
        .take()
        .ok_or_else(|| CoreError::InvalidSetting("no file selected".into()).to_payload())?;
    Ok(
        l.submit_library_mod(&path, &sha1, kind, &name, &description)
            .await?
            .0,
    )
}

/// Approved items of every kind and the caller's own uploads.
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

/// Into `instance_id` for mods, resource packs and shaders; a modpack
/// (`instance_id` = null) becomes a new instance.
#[tauri::command]
pub async fn library_install(
    state: State<'_, AppState>,
    library_id: i64,
    instance_id: Option<String>,
    accept_warnings: bool,
) -> CmdResult<LibraryInstall> {
    let l = state.launcher()?.clone();
    Ok(
        l.install_library_mod(library_id, instance_id.as_deref(), accept_warnings)
            .await?,
    )
}
