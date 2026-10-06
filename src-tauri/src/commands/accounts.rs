use std::sync::Arc;

use launcher_core::auth::store::{Account, AccountsView};
use launcher_core::session::{AccountFaces, Launcher};
use tauri::State;

use super::CmdResult;
use super::dialogs::{PickPurpose, take_pick};
use crate::state::AppState;

/// Sends the (possibly changed) name and photo to friends in the background.
pub fn refresh_friend_profile(launcher: &Arc<Launcher>) {
    let launcher = launcher.clone();
    tauri::async_runtime::spawn(async move { launcher.refresh_friend_profile().await });
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> CmdResult<AccountsView> {
    Ok(state.launcher()?.accounts.view())
}

/// The name is reserved for all MehburMC users first (K67).
#[tauri::command]
pub async fn add_offline_account(state: State<'_, AppState>, name: String) -> CmdResult<Account> {
    let launcher = state.launcher()?.clone();
    let account = launcher.create_account(&name).await?;
    refresh_friend_profile(&launcher);
    Ok(account)
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> CmdResult<AccountsView> {
    let launcher = state.launcher()?.clone();
    let view = launcher.remove_account(&id).await?;
    refresh_friend_profile(&launcher);
    Ok(view)
}

/// Renames an account; the next launch uses the new in-game name.
#[tauri::command]
pub async fn rename_account(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CmdResult<Account> {
    let launcher = state.launcher()?.clone();
    let account = launcher.rename_account(&id, &name).await?;
    refresh_friend_profile(&launcher);
    Ok(account)
}

#[tauri::command]
pub fn select_account(state: State<'_, AppState>, id: String) -> CmdResult<AccountsView> {
    let launcher = state.launcher()?;
    let view = launcher.accounts.select(&id)?;
    refresh_friend_profile(launcher);
    Ok(view)
}

/// Profile photos and default-skin heads of the accounts.
#[tauri::command]
pub async fn account_avatars(state: State<'_, AppState>) -> CmdResult<AccountFaces> {
    let launcher = state.launcher()?.clone();
    // Reads the client jar for default skins: keep it off the UI thread.
    tauri::async_runtime::spawn_blocking(move || launcher.account_faces())
        .await
        .map_err(|e| launcher_core::CoreError::AvatarInvalid(e.to_string()).to_payload())
}

/// Sets the photo picked with `pick_path("avatar")`; returns it as a URI.
#[tauri::command]
pub async fn set_account_avatar(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let file = take_pick(&state, PickPurpose::Avatar)?;
    let launcher = state.launcher()?.clone();
    if !launcher.accounts.view().accounts.iter().any(|a| a.id == id) {
        return Err(launcher_core::CoreError::AccountNotFound(id).to_payload());
    }
    let l = launcher.clone();
    let uri = tauri::async_runtime::spawn_blocking(move || l.avatars.set_from_file(&id, &file))
        .await
        .map_err(|e| launcher_core::CoreError::AvatarInvalid(e.to_string()).to_payload())??;
    refresh_friend_profile(&launcher);
    Ok(uri)
}

/// Back to the skin head.
#[tauri::command]
pub fn clear_account_avatar(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let launcher = state.launcher()?;
    launcher.avatars.clear(&id)?;
    refresh_friend_profile(launcher);
    Ok(())
}
