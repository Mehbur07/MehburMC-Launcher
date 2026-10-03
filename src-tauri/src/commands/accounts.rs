use launcher_core::CoreError;
use launcher_core::auth::store::{Account, AccountsView};
use launcher_core::session::LoginPrompt;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

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
    Ok(state.launcher()?.accounts.remove(&id)?)
}

#[tauri::command]
pub fn select_account(state: State<'_, AppState>, id: String) -> CmdResult<AccountsView> {
    Ok(state.launcher()?.accounts.select(&id)?)
}

/// Starts Microsoft sign-in; returns the code the user types in the browser.
#[tauri::command]
pub async fn begin_microsoft_login(state: State<'_, AppState>) -> CmdResult<LoginPrompt> {
    let l = state.launcher()?.clone();
    let client_id = state.settings().msa_client_id();
    Ok(l.begin_microsoft_login(client_id).await?)
}

/// Resolves once the user finished (or abandoned) the browser step.
#[tauri::command]
pub async fn finish_microsoft_login(
    state: State<'_, AppState>,
    login_id: String,
) -> CmdResult<Account> {
    let l = state.launcher()?.clone();
    Ok(l.finish_microsoft_login(&login_id).await?)
}

#[tauri::command]
pub fn cancel_microsoft_login(state: State<'_, AppState>, login_id: String) -> CmdResult<()> {
    state.launcher()?.cancel_microsoft_login(&login_id);
    Ok(())
}

/// Opens the verification page in the default browser (Microsoft hosts only).
#[tauri::command]
pub fn open_microsoft_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    login_id: String,
) -> CmdResult<()> {
    let url = state
        .launcher()?
        .login_page(&login_id)
        .ok_or(CoreError::AuthCodeExpired)?;
    if !launcher_core::auth::microsoft::is_microsoft_page(&url) {
        return Err(CoreError::UrlNotAllowed { url }.into());
    }
    app.opener()
        .open_url(url.clone(), None::<&str>)
        .map_err(|e| CoreError::UrlNotAllowed {
            url: format!("{url} ({e})"),
        })?;
    Ok(())
}
