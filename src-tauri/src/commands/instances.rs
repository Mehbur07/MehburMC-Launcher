use std::path::PathBuf;

use launcher_core::CoreError;
use launcher_core::instance::files::{self, Folder};
use launcher_core::instance::{Instance, InstancePatch, NewInstance, transfer};
use launcher_core::state::LauncherState;
use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::{CmdResult, blocking, open_path};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstancesView {
    pub instances: Vec<Instance>,
    pub selected: Option<String>,
}

fn view(state: &AppState) -> CmdResult<InstancesView> {
    let l = state.launcher()?;
    let instances = l.instances.list();
    let mut selected = LauncherState::load(l.instances.paths()).selected_instance;
    // Fall back to the first instance if the selection is gone.
    if !selected
        .as_ref()
        .is_some_and(|s| instances.iter().any(|i| &i.id == s))
    {
        selected = instances.first().map(|i| i.id.clone());
    }
    Ok(InstancesView {
        instances,
        selected,
    })
}

#[tauri::command]
pub fn list_instances(state: State<'_, AppState>) -> CmdResult<InstancesView> {
    view(&state)
}

#[tauri::command]
pub async fn create_instance(state: State<'_, AppState>, req: NewInstance) -> CmdResult<Instance> {
    let l = state.launcher()?.clone();
    let inst = blocking(move || l.instances.create(req)).await?;
    select_instance(state, inst.id.clone())?;
    Ok(inst)
}

#[tauri::command]
pub fn update_instance(
    state: State<'_, AppState>,
    id: String,
    patch: InstancePatch,
) -> CmdResult<Instance> {
    Ok(state.launcher()?.instances.update(&id, patch)?)
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, id: String) -> CmdResult<InstancesView> {
    let l = state.launcher()?.clone();
    blocking(move || l.delete_instance(&id)).await?;
    view(&state)
}

#[tauri::command]
pub async fn copy_instance(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CmdResult<Instance> {
    let l = state.launcher()?.clone();
    blocking(move || l.instances.copy(&id, &name)).await
}

#[tauri::command]
pub fn reorder_instances(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    Ok(state.launcher()?.instances.reorder(ids)?)
}

#[tauri::command]
pub fn select_instance(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let l = state.launcher()?;
    l.instances.get(&id)?;
    LauncherState::update(l.instances.paths(), |s| s.selected_instance = Some(id))?;
    Ok(())
}

#[tauri::command]
pub async fn export_instance(
    state: State<'_, AppState>,
    id: String,
    dest: String,
) -> CmdResult<()> {
    let dest = PathBuf::from(dest);
    if !dest.is_absolute() || dest.extension().and_then(|e| e.to_str()) != Some("zip") {
        return Err(CoreError::InvalidInstance("export path must be a .zip".into()).into());
    }
    let l = state.launcher()?.clone();
    blocking(move || transfer::export(&l.instances, &id, &dest)).await
}

#[tauri::command]
pub async fn import_instance(state: State<'_, AppState>, src: String) -> CmdResult<Instance> {
    let src = PathBuf::from(src);
    let l = state.launcher()?.clone();
    blocking(move || transfer::import(&l.instances, &src)).await
}

#[tauri::command]
pub fn open_instance_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    folder: Option<Folder>,
) -> CmdResult<()> {
    let l = state.launcher()?;
    l.instances.get(&id)?;
    let dir = files::folder_path(&l.instances, &id, folder.unwrap_or(Folder::Root))?;
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;
    open_path(&app, &dir)
}
