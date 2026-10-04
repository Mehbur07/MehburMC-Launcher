use launcher_core::auth::store::{Account, AccountsView};
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> CmdResult<AccountsView> {
    Ok(state.launcher()?.accounts.view())
}

#[tauri::command]
pub fn add_offline_account(state: State<'_, AppState>, name: String) -> CmdResult<Account> {
    Ok(state.launcher()?.accounts.add_offline(&name)?)
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, id: String) -> CmdResult<AccountsView> {
    let launcher = state.launcher()?;
    let view = launcher.accounts.remove(&id)?;
    if let Err(e) = launcher.skins.forget_account(&id) {
        tracing::warn!(error = %e.detail(), "could not clear skin assignment");
    }
    Ok(view)
}

/// Renames an account; the next launch uses the new in-game name.
#[tauri::command]
pub fn rename_account(state: State<'_, AppState>, id: String, name: String) -> CmdResult<Account> {
    Ok(state.launcher()?.accounts.rename(&id, &name)?)
}

#[tauri::command]
pub fn select_account(state: State<'_, AppState>, id: String) -> CmdResult<AccountsView> {
    Ok(state.launcher()?.accounts.select(&id)?)
}
