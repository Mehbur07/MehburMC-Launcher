//! Shared skins and capes (K70).

use launcher_core::friends::textures::{ReportReason, SharedTexture, Visibility};
use launcher_core::skin::TextureKind;
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

/// Everything the caller can see (own shares first).
#[tauri::command]
pub async fn community_textures(state: State<'_, AppState>) -> CmdResult<Vec<SharedTexture>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.community_textures().await?)
}

/// Shares a library texture (`id` = its SHA-1); returns the share id.
#[tauri::command]
pub async fn share_texture(
    state: State<'_, AppState>,
    kind: TextureKind,
    id: String,
    name: String,
    visibility: Visibility,
) -> CmdResult<i64> {
    let l = state.launcher()?.clone();
    Ok(l.share_texture(kind, &id, &name, visibility).await?)
}

#[tauri::command]
pub async fn unshare_texture(state: State<'_, AppState>, share_id: i64) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.unshare_texture(share_id).await?)
}

#[tauri::command]
pub async fn report_texture(
    state: State<'_, AppState>,
    share_id: i64,
    reason: ReportReason,
    note: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.report_texture(share_id, reason, &note).await?)
}
