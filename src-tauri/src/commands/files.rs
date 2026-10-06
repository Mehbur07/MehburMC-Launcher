use launcher_core::instance::files::{self, FileEntry, Folder};
use tauri::{AppHandle, Manager, State};

use super::{CmdResult, blocking};
use crate::state::AppState;

#[tauri::command]
pub async fn list_instance_files(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
) -> CmdResult<Vec<FileEntry>> {
    let l = state.launcher()?.clone();
    if folder == Folder::Screenshots {
        // Thumbnails are served through the asset protocol; scope it to this
        // instance's screenshots only.
        let dir = files::folder_path(&l.instances, &id, Folder::Screenshots)?;
        let _ = app.asset_protocol_scope().allow_directory(&dir, false);
    }
    blocking(move || files::list(&l.instances, &id, folder)).await
}

/// Absolute path of an instance folder (for thumbnails via `convertFileSrc`).
#[tauri::command]
pub fn instance_folder_path(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
) -> CmdResult<String> {
    let l = state.launcher()?;
    l.instances.get(&id)?;
    Ok(files::folder_path(&l.instances, &id, folder)?
        .display()
        .to_string())
}

#[tauri::command]
pub fn toggle_instance_file(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    name: String,
) -> CmdResult<FileEntry> {
    Ok(state.launcher()?.toggle_instance_file(&id, folder, &name)?)
}

/// Turns several content files on or off at once (Mod Toggle page).
#[tauri::command]
pub async fn set_content_enabled(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    names: Vec<String>,
    enabled: bool,
) -> CmdResult<u32> {
    let l = state.launcher()?.clone();
    blocking(move || l.set_content_enabled(&id, folder, &names, enabled)).await
}

#[tauri::command]
pub async fn delete_instance_file(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    name: String,
) -> CmdResult<()> {
    let l = state.launcher()?.clone();
    blocking(move || l.delete_instance_file(&id, folder, &name)).await
}

#[tauri::command]
pub async fn read_instance_log(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    name: String,
) -> CmdResult<String> {
    let l = state.launcher()?.clone();
    blocking(move || files::read_text(&l.instances, &id, folder, &name)).await
}
