use std::collections::BTreeMap;

use launcher_core::skin::defaults::{self, DefaultSkin};
use launcher_core::skin::{Assignment, LibraryView, SkinModel, TextureKind};
use tauri::State;

use super::dialogs::{PickPurpose, take_pick};
use super::{CmdResult, blocking};
use crate::state::AppState;

#[tauri::command]
pub async fn list_skins(state: State<'_, AppState>) -> CmdResult<LibraryView> {
    let launcher = state.launcher()?.clone();
    blocking(move || {
        if let Err(e) = launcher.skins.seed_builtins() {
            tracing::warn!(error = %e, "could not add the built-in skins");
        }
        Ok(launcher.skins.view())
    })
    .await
}

/// Imports a PNG picked in the file dialog; returns the texture id.
#[tauri::command]
pub async fn import_skin_file(
    state: State<'_, AppState>,
    kind: TextureKind,
    model: Option<SkinModel>,
) -> CmdResult<String> {
    let purpose = match kind {
        TextureKind::Skin => PickPurpose::Skin,
        TextureKind::Cape => PickPurpose::Cape,
    };
    let path = take_pick(&state, purpose)?;
    let launcher = state.launcher()?.clone();
    blocking(move || launcher.skins.import_file(kind, &path, model)).await
}

/// Adds a texture drawn in the editor or taken from the presets (base64 PNG);
/// returns the texture id.
#[tauri::command]
pub async fn add_skin_bytes(
    state: State<'_, AppState>,
    kind: TextureKind,
    name: String,
    model: Option<SkinModel>,
    png_base64: String,
) -> CmdResult<String> {
    let launcher = state.launcher()?.clone();
    blocking(move || launcher.skins.add_base64(kind, &name, model, &png_base64)).await
}

/// The game's own default skins, read from an installed client jar.
#[tauri::command]
pub async fn list_default_skins(state: State<'_, AppState>) -> CmdResult<Vec<DefaultSkin>> {
    let launcher = state.launcher()?.clone();
    blocking(move || Ok(defaults::list(&launcher.ctx.paths))).await
}

#[tauri::command]
pub async fn update_skin(
    state: State<'_, AppState>,
    kind: TextureKind,
    id: String,
    name: Option<String>,
    model: Option<SkinModel>,
) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    blocking(move || match kind {
        TextureKind::Skin => launcher
            .skins
            .update_skin(&id, name.as_deref(), model)
            .map(drop),
        TextureKind::Cape => match name {
            Some(n) => launcher.skins.rename_cape(&id, &n).map(drop),
            None => Ok(()),
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_skin(
    state: State<'_, AppState>,
    kind: TextureKind,
    id: String,
) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    blocking(move || launcher.skins.delete(kind, &id)).await
}

#[tauri::command]
pub async fn assign_skin(
    state: State<'_, AppState>,
    account_id: String,
    kind: TextureKind,
    id: Option<String>,
) -> CmdResult<BTreeMap<String, Assignment>> {
    let launcher = state.launcher()?.clone();
    blocking(move || {
        // Only real accounts get assignments.
        if !launcher
            .accounts
            .view()
            .accounts
            .iter()
            .any(|a| a.id == account_id)
        {
            return Err(launcher_core::CoreError::AccountNotFound(account_id));
        }
        launcher.skins.assign(&account_id, kind, id.as_deref())
    })
    .await
}

/// Saves a texture to a path picked in the save dialog.
#[tauri::command]
pub async fn export_skin(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let dest = take_pick(&state, PickPurpose::SkinExport)?;
    let launcher = state.launcher()?.clone();
    blocking(move || launcher.skins.export(&id, &dest)).await
}
