use std::sync::Arc;

use launcher_core::instance::Instance;
use launcher_core::launch::process::GameExit;
use launcher_core::session::{LaunchHooks, StartMode};
use launcher_core::settings::LaunchBehavior;
use launcher_core::tasks::TaskInfo;
use tauri::{AppHandle, Manager, State};

use super::CmdResult;
use crate::state::AppState;

/// Minimises the launcher while the game runs and brings it back afterwards.
struct WindowHooks {
    app: AppHandle,
    behavior: LaunchBehavior,
}

impl LaunchHooks for WindowHooks {
    fn game_started(&self, _instance: &Instance) {
        // `Close` is treated like `Minimize` until detached launching lands.
        if self.behavior != LaunchBehavior::KeepOpen
            && let Some(w) = self.app.get_webview_window("main")
        {
            let _ = w.minimize();
        }
    }

    fn game_exited(&self, _instance: &Instance, _exit: &GameExit) {
        if self.behavior != LaunchBehavior::KeepOpen
            && let Some(w) = self.app.get_webview_window("main")
        {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }
}

fn start(app: AppHandle, state: &AppState, id: &str, mode: StartMode) -> CmdResult<String> {
    let settings = state.settings();
    let hooks = Arc::new(WindowHooks {
        app,
        behavior: settings.launch_behavior,
    });
    Ok(state.launcher()?.start(id, settings, mode, hooks)?)
}

/// Must be async: the task is spawned on the Tokio runtime.
#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<String> {
    start(app, &state, &id, StartMode::Play)
}

#[tauri::command]
pub async fn repair_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<String> {
    start(app, &state, &id, StartMode::Repair)
}

#[tauri::command]
pub fn stop_instance(state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    Ok(state.launcher()?.stop(&id))
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>) -> CmdResult<Vec<TaskInfo>> {
    Ok(state.launcher()?.tasks.list())
}

/// `pause` keeps partial downloads so starting again resumes them.
#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, id: String, pause: bool) -> CmdResult<bool> {
    Ok(state.launcher()?.tasks.cancel(&id, pause))
}

#[tauri::command]
pub fn clear_tasks(state: State<'_, AppState>) -> CmdResult<Vec<TaskInfo>> {
    let l = state.launcher()?;
    l.tasks.clear_finished();
    Ok(l.tasks.list())
}
