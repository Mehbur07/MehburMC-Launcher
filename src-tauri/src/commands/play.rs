use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use launcher_core::instance::Instance;
use launcher_core::launch::process::GameExit;
use launcher_core::servers::ServerAddress;
use launcher_core::session::{LaunchHooks, StartMode};
use launcher_core::settings::LaunchBehavior;
use launcher_core::tasks::TaskInfo;
use tauri::{AppHandle, Manager, State};

use super::CmdResult;
use crate::state::AppState;

/// `Close` without a main-menu line (unusual logging): quit after this long
/// if the game is still running.
const CLOSE_FALLBACK: Duration = Duration::from_secs(120);

/// Minimises the launcher while the game runs and brings it back afterwards.
/// With `Close`, the launcher quits once the game reached the main menu; the
/// game keeps running on its own (Windows does not kill child processes and
/// the launcher never terminates the game on exit).
struct WindowHooks {
    app: AppHandle,
    behavior: LaunchBehavior,
    exited: Arc<AtomicBool>,
}

impl WindowHooks {
    fn quit(app: &AppHandle) {
        tracing::info!("game is running; closing the launcher (launch behaviour: close)");
        app.exit(0);
    }
}

impl LaunchHooks for WindowHooks {
    fn game_started(&self, _instance: &Instance) {
        if self.behavior != LaunchBehavior::KeepOpen
            && let Some(w) = self.app.get_webview_window("main")
        {
            let _ = w.minimize();
        }
        if self.behavior == LaunchBehavior::Close {
            let (app, exited) = (self.app.clone(), self.exited.clone());
            std::thread::spawn(move || {
                std::thread::sleep(CLOSE_FALLBACK);
                if !exited.load(Ordering::SeqCst) {
                    Self::quit(&app);
                }
            });
        }
    }

    fn game_ready(&self, _instance: &Instance) {
        if self.behavior == LaunchBehavior::Close {
            Self::quit(&self.app);
        }
    }

    fn game_exited(&self, _instance: &Instance, _exit: &GameExit) {
        self.exited.store(true, Ordering::SeqCst);
        if self.behavior != LaunchBehavior::KeepOpen
            && let Some(w) = self.app.get_webview_window("main")
        {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }
}

fn start(
    app: AppHandle,
    state: &AppState,
    id: &str,
    mode: StartMode,
    join: Option<ServerAddress>,
) -> CmdResult<String> {
    let settings = state.settings();
    let hooks = Arc::new(WindowHooks {
        app,
        behavior: settings.launch_behavior,
        exited: Arc::new(AtomicBool::new(false)),
    });
    Ok(state
        .launcher()?
        .start_with(id, settings, mode, hooks, join)?)
}

/// Must be async: the task is spawned on the Tokio runtime.
#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<String> {
    start(app, &state, &id, StartMode::Play, None)
}

/// Starts the instance and joins `address` from the main menu.
#[tauri::command]
pub async fn join_server(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    address: String,
) -> CmdResult<String> {
    let addr = ServerAddress::parse(&address)?;
    start(app, &state, &id, StartMode::Play, Some(addr))
}

#[tauri::command]
pub async fn repair_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<String> {
    start(app, &state, &id, StartMode::Repair, None)
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
