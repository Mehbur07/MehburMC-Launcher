//! Admin tools (K73). The server checks every right; these only forward.

use launcher_core::content::scan::ScanReport;
use launcher_core::friends::admin::{
    AdminBan, AdminEntry, AdminMod, AdminModStatus, AdminReport, AdminUser, ReportKind,
};
use launcher_core::friends::private_textures::{PrivateTexture, TextureGrant};
use launcher_core::skin::{SkinModel, TextureKind};
use tauri::State;

use super::CmdResult;
use super::dialogs::{PickPurpose, take_pick};
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

/// Founder: private textures with previews and grant counts (K74).
#[tauri::command]
pub async fn admin_private_textures(state: State<'_, AppState>) -> CmdResult<Vec<PrivateTexture>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_private_textures().await?)
}

/// Founder: uploads the PNG picked with `pick_path("privateTexture")`.
#[tauri::command]
pub async fn admin_upload_private_texture(
    state: State<'_, AppState>,
    kind: TextureKind,
    model: SkinModel,
    name: String,
) -> CmdResult<i64> {
    let path = take_pick(&state, PickPurpose::PrivateTexture)?;
    let l = state.launcher()?.clone();
    let meta = std::fs::metadata(&path).map_err(|e| launcher_core::CoreError::io(&path, e))?;
    if meta.len() > launcher_core::friends::textures::MAX_BYTES as u64 {
        return Err(launcher_core::CoreError::SkinInvalid("too large".into()).to_payload());
    }
    let png = std::fs::read(&path).map_err(|e| launcher_core::CoreError::io(&path, e))?;
    Ok(l.friends
        .admin_upload_private_texture(kind, model, &name, &png)
        .await?)
}

#[tauri::command]
pub async fn admin_delete_private_texture(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_delete_private_texture(id).await?)
}

#[tauri::command]
pub async fn admin_texture_grants(
    state: State<'_, AppState>,
    id: i64,
) -> CmdResult<Vec<TextureGrant>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_texture_grants(id).await?)
}

#[tauri::command]
pub async fn admin_set_texture_grant(
    state: State<'_, AppState>,
    id: i64,
    user_id: String,
    grant: bool,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends
        .admin_set_texture_grant(id, &user_id, grant)
        .await?)
}
