//! Admin tools (K73). The server checks every right; these only forward.

use launcher_core::content::scan::ScanReport;
use launcher_core::friends::admin::{
    AdminBan, AdminEntry, AdminMod, AdminModStatus, AdminReport, AdminUser, ReportKind,
};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn admin_library(
    state: State<'_, AppState>,
    status: AdminModStatus,
) -> CmdResult<Vec<AdminMod>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_library(status).await?)
}

/// Downloads a library mod and scans it in this launcher.
#[tauri::command]
pub async fn admin_scan_mod(
    state: State<'_, AppState>,
    id: i64,
    status: AdminModStatus,
) -> CmdResult<ScanReport> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_scan_mod(id, status).await?)
}

#[tauri::command]
pub async fn admin_review_mod(
    state: State<'_, AppState>,
    id: i64,
    status: AdminModStatus,
    approve: bool,
    note: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends
        .admin_review_mod(id, status, approve, &note)
        .await?)
}

#[tauri::command]
pub async fn admin_remove_mod(state: State<'_, AppState>, id: i64, note: String) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_remove_mod(id, &note).await?)
}

#[tauri::command]
pub async fn admin_reports(state: State<'_, AppState>) -> CmdResult<Vec<AdminReport>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_reports().await?)
}

#[tauri::command]
pub async fn admin_dismiss_reports(
    state: State<'_, AppState>,
    kind: ReportKind,
    id: i64,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_dismiss_reports(kind, id).await?)
}

#[tauri::command]
pub async fn admin_hide_texture(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_hide_texture(id).await?)
}

#[tauri::command]
pub async fn admin_find_users(
    state: State<'_, AppState>,
    query: String,
) -> CmdResult<Vec<AdminUser>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_find_users(&query).await?)
}

/// `hours` = `None` bans permanently.
#[tauri::command]
pub async fn admin_ban(
    state: State<'_, AppState>,
    user_id: String,
    hours: Option<u32>,
    reason: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_ban(&user_id, hours, &reason).await?)
}

#[tauri::command]
pub async fn admin_unban(state: State<'_, AppState>, user_id: String) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_unban(&user_id).await?)
}

#[tauri::command]
pub async fn admin_bans(state: State<'_, AppState>) -> CmdResult<Vec<AdminBan>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_bans().await?)
}

#[tauri::command]
pub async fn admin_list(state: State<'_, AppState>) -> CmdResult<Vec<AdminEntry>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_list().await?)
}

#[tauri::command]
pub async fn admin_set_rank(
    state: State<'_, AppState>,
    user_id: String,
    admin: bool,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_set_rank(&user_id, admin).await?)
}
