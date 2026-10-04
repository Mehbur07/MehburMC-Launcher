use launcher_core::CoreError;
use launcher_core::content::install::InstallResult;
use launcher_core::instance::LoaderKind;
use launcher_core::loader::optifine::{self, OptifineInfo};
use launcher_core::loader::{self, LoaderVersion};
use tauri::State;

use super::{CmdResult, blocking};
use crate::state::AppState;

#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    kind: LoaderKind,
    mc: String,
) -> CmdResult<Vec<LoaderVersion>> {
    let l = state.launcher()?.clone();
    Ok(loader::list_versions(&l.ctx, kind, &mc).await?)
}

/// Copies an OptiFine jar the user picked into the loader cache.
#[tauri::command]
pub async fn import_optifine(state: State<'_, AppState>) -> CmdResult<OptifineInfo> {
    let src = super::dialogs::take_pick(&state, super::dialogs::PickPurpose::Optifine)?;
    let is_jar = src
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
    if !src.is_absolute() || !is_jar {
        return Err(CoreError::OptifineInvalid { path: src }.into());
    }
    let l = state.launcher()?.clone();
    blocking(move || optifine::import(&l.ctx.paths, &src)).await
}

#[tauri::command]
pub async fn install_shader_support(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<InstallResult> {
    let l = state.launcher()?.clone();
    Ok(l.install_shader_support(&id).await?)
}
