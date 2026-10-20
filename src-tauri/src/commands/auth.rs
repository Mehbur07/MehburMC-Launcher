//! MehburMC account sign-in (K73). Passwords pass straight through to
//! Supabase over HTTPS; they are never stored or logged.

use launcher_core::friends::account::{AuthStatus, CodePurpose, SignUpResult};
use tauri::{AppHandle, State};

use super::CmdResult;
use crate::state::AppState;

/// Account state; `check` asks the server for rank and ban.
#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>, check: bool) -> CmdResult<AuthStatus> {
    let l = state.launcher()?.clone();
    Ok(l.friends.auth_status(check).await)
}

#[tauri::command]
pub async fn auth_sign_up(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> CmdResult<SignUpResult> {
    let l = state.launcher()?.clone();
    let r = l.friends.sign_up(&email, &password).await?;
    if matches!(r, SignUpResult::SignedIn) {
        l.sync_account_names().await;
    }
    Ok(r)
}

#[tauri::command]
pub async fn auth_verify(
    state: State<'_, AppState>,
    email: String,
    code: String,
    purpose: CodePurpose,
    password: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    l.friends
        .verify_code(&email, &code, purpose, &password)
        .await?;
    l.sync_account_names().await;
    Ok(())
}

#[tauri::command]
pub async fn auth_resend(
    state: State<'_, AppState>,
    email: String,
    purpose: CodePurpose,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.resend_code(&email, purpose).await?)
}

/// Signs in with Google in the default browser (K83); resolves once the
/// browser comes back, on cancel or after five minutes.
#[tauri::command]
pub async fn auth_google(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let l = state.launcher()?.clone();
    l.friends
        .google_sign_in(|url| {
            app.opener().open_url(url, None::<&str>).map_err(|e| {
                launcher_core::CoreError::UrlNotAllowed {
                    url: format!("{url} ({e})"),
                }
            })
        })
        .await?;
    // Re-reserve this launcher's account names for the signed-in account.
    l.sync_account_names().await;
    Ok(())
}

#[tauri::command]
pub fn auth_google_cancel(state: State<'_, AppState>) -> CmdResult<()> {
    state.launcher()?.friends.cancel_google_sign_in();
    Ok(())
}

#[tauri::command]
pub async fn auth_request_reset(state: State<'_, AppState>, email: String) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.request_password_reset(&email).await?)
}

#[tauri::command]
pub async fn auth_sign_in(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    l.friends.sign_in(&email, &password).await?;
    // Re-reserve this launcher's account names for the signed-in account.
    l.sync_account_names().await;
    Ok(())
}

/// Starts a local-only account without email (K82).
#[tauri::command]
pub async fn auth_create_local(state: State<'_, AppState>) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.create_local_account().await?)
}

#[tauri::command]
pub async fn auth_sign_out(state: State<'_, AppState>) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    Ok(l.friends.sign_out().await?)
}

/// Applies the private texture grants of the signed-in account (K74);
/// `true` if the skin library changed.
#[tauri::command]
pub async fn sync_private_textures(state: State<'_, AppState>) -> CmdResult<bool> {
    let l = state.launcher()?.clone();
    Ok(l.sync_private_textures().await?)
}
