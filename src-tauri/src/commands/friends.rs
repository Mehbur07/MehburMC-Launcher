//! Friends: identity, friend requests, chat and shared mod lists (K64).

use launcher_core::friends::share::{FriendInstallResult, SharedList};
use launcher_core::friends::{ChatMessage, Friend, FriendsStatus, Profile};
use std::sync::Arc;

use launcher_core::friends::presence;
use launcher_core::session::Launcher;
use tauri::{Manager, State};

use super::CmdResult;
use crate::state::AppState;

/// Name and photo friends see come from the selected launcher account.
#[tauri::command]
pub async fn friends_status(state: State<'_, AppState>) -> CmdResult<FriendsStatus> {
    let launcher = state.launcher()?.clone();
    let (name, avatar) = launcher.friend_identity();
    Ok(launcher.friends.status(&name, avatar.as_deref()).await?)
}

/// Whether friends see us online right now (cheap, no network).
#[tauri::command]
pub fn friends_online(state: State<'_, AppState>) -> CmdResult<bool> {
    Ok(state.launcher()?.friends.is_online())
}

/// Keeps us online for friends while the launcher runs (K66).
pub async fn heartbeat_loop(launcher: Arc<Launcher>) {
    let period = std::time::Duration::from_secs(presence::HEARTBEAT_SECS);
    loop {
        if let Err(e) = launcher.friends.heartbeat().await {
            tracing::debug!(error = %e.detail(), "heartbeat failed");
        }
        tokio::time::sleep(period).await;
    }
}

/// Last chance to tell friends we left; bounded so closing never hangs.
pub fn go_offline_on_exit(app: &tauri::AppHandle) {
    let Some(launcher) = app.try_state::<AppState>().and_then(|s| s.launcher.clone()) else {
        return;
    };
    tauri::async_runtime::block_on(async move {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            launcher.friends.go_offline(),
        )
        .await;
    });
}

/// Creates the anonymous identity after the user agreed to the notice.
#[tauri::command]
pub async fn friends_enable(state: State<'_, AppState>) -> CmdResult<Profile> {
    let launcher = state.launcher()?.clone();
    let (name, avatar) = launcher.friend_identity();
    Ok(launcher.friends.enable(&name, avatar.as_deref()).await?)
}

/// Deletes all server-side data and forgets the identity.
#[tauri::command]
pub async fn friends_disable(state: State<'_, AppState>) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.disable_and_delete().await?)
}

#[tauri::command]
pub async fn friends_list(state: State<'_, AppState>) -> CmdResult<Vec<Friend>> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.friends().await?)
}

/// Returns "pending" or "accepted".
#[tauri::command]
pub async fn friend_request(state: State<'_, AppState>, code: String) -> CmdResult<String> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.send_request(&code).await?)
}

#[tauri::command]
pub async fn friend_respond(
    state: State<'_, AppState>,
    request_id: i64,
    accept: bool,
) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.respond(request_id, accept).await?)
}

#[tauri::command]
pub async fn friend_remove(state: State<'_, AppState>, friend: String) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.remove(&friend).await?)
}

#[tauri::command]
pub async fn friend_block(state: State<'_, AppState>, friend: String) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.block(&friend).await?)
}

#[tauri::command]
pub async fn chat_messages(
    state: State<'_, AppState>,
    friend: String,
    after: Option<i64>,
) -> CmdResult<Vec<ChatMessage>> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.messages(&friend, after).await?)
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    friend: String,
    body: String,
) -> CmdResult<ChatMessage> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.send_message(&friend, &body).await?)
}

#[tauri::command]
pub async fn chat_mark_read(state: State<'_, AppState>, friend: String) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.mark_read(&friend).await?)
}

#[tauri::command]
pub async fn my_shares(state: State<'_, AppState>) -> CmdResult<Vec<SharedList>> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.my_shares().await?)
}

#[tauri::command]
pub async fn share_instance(state: State<'_, AppState>, id: String) -> CmdResult<SharedList> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.share_instance(&id).await?)
}

#[tauri::command]
pub async fn unshare_instance(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.unshare_instance(&id).await?)
}

#[tauri::command]
pub async fn friend_shared_lists(
    state: State<'_, AppState>,
    friend: String,
) -> CmdResult<Vec<SharedList>> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.friend_lists(&friend).await?)
}

#[tauri::command]
pub async fn install_friend_mods(
    state: State<'_, AppState>,
    list_id: i64,
    instance_id: String,
    files: Vec<String>,
) -> CmdResult<FriendInstallResult> {
    let launcher = state.launcher()?.clone();
    Ok(launcher
        .install_friend_mods(list_id, &instance_id, &files)
        .await?)
}
