//! Admine mesaj (K101). Sunucu her yetkiyi denetler; bunlar yalnızca iletir.

use launcher_core::friends::support::{
    AdminSupportThread, SupportMessage, SupportThread, SupportUnread,
};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn support_threads(state: State<'_, AppState>) -> CmdResult<Vec<SupportThread>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_threads().await?)
}

#[tauri::command]
pub async fn support_open(
    state: State<'_, AppState>,
    subject: String,
    body: String,
) -> CmdResult<i64> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_open(&subject, &body).await?)
}

#[tauri::command]
pub async fn support_send(state: State<'_, AppState>, thread: i64, body: String) -> CmdResult<i64> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_send(thread, &body).await?)
}

#[tauri::command]
pub async fn support_messages(
    state: State<'_, AppState>,
    thread: i64,
    after: Option<i64>,
) -> CmdResult<Vec<SupportMessage>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_messages(thread, after).await?)
}

#[tauri::command]
pub async fn support_mark_read(state: State<'_, AppState>, thread: i64) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_mark_read(thread).await?)
}

#[tauri::command]
pub async fn support_set_open(
    state: State<'_, AppState>,
    thread: i64,
    open: bool,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_set_open(thread, open).await?)
}

#[tauri::command]
pub async fn support_unread(state: State<'_, AppState>) -> CmdResult<SupportUnread> {
    let l = state.launcher()?.clone();
    Ok(l.friends.support_unread().await?)
}

#[tauri::command]
pub async fn admin_support_threads(
    state: State<'_, AppState>,
    open: bool,
) -> CmdResult<Vec<AdminSupportThread>> {
    let l = state.launcher()?.clone();
    Ok(l.friends.admin_support_threads(open).await?)
}
