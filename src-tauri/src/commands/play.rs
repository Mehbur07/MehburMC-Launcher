use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

/// Set while the window is closed for a running game (`CloseReopen`, K93):
/// the app keeps running without a window and opens it when the game exits.
static WINDOW_AWAY: AtomicBool = AtomicBool::new(false);
/// Orders closing the window against the game exiting at the same moment.
static AWAY_GATE: Mutex<()> = Mutex::new(());

pub fn window_away() -> bool {
    WINDOW_AWAY.load(Ordering::SeqCst)
}

/// Shows the main window, creating it again if it was closed for a game.
pub fn reopen_window(app: &AppHandle) {
    WINDOW_AWAY.store(false, Ordering::SeqCst);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let window = match handle.get_webview_window("main") {
            Some(w) => w,
            None => {
                let dir = handle
                    .try_state::<AppState>()
                    .and_then(|s| s.paths.as_ref().map(|p| p.webview_data()));
                match crate::main_window(&handle, dir) {
                    Ok(w) => w,
                    Err(e) => {
                        // Never stay alive without a window.
                        tracing::error!("could not open the launcher window again: {e}");
                        handle.exit(1);
                        return;
                    }
                }
            }
        };
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    });
}

/// Minimises the launcher while the game runs and brings it back afterwards.
/// With `Close`, the launcher quits once the game reached the main menu; the
/// game keeps running on its own (Windows does not kill child processes and
/// the launcher never terminates the game on exit). With `CloseReopen`, only
/// the window closes (freeing the WebView2 memory) and comes back when the
/// game exits.
#[derive(Clone)]
struct WindowHooks {
    app: AppHandle,
    behavior: LaunchBehavior,
    exited: Arc<AtomicBool>,
}

impl WindowHooks {
    fn closes(&self) -> bool {
        matches!(
            self.behavior,
            LaunchBehavior::Close | LaunchBehavior::CloseReopen
        )
    }

    fn quit(&self) {
        // Closing would end the voice call (K86); stay minimised instead.
        if super::calls::in_call(&self.app) {
            tracing::info!("in a voice call; keeping the launcher open (minimised)");
            return;
        }
        if self.behavior == LaunchBehavior::CloseReopen {
            let _gate = AWAY_GATE.lock().expect("away lock");
            if self.exited.load(Ordering::SeqCst) {
                return;
            }
            tracing::info!("game is running; closing the window until it exits");
            WINDOW_AWAY.store(true, Ordering::SeqCst);
            if let Some(w) = self.app.get_webview_window("main") {
                let _ = w.destroy();
            }
            return;
        }
        tracing::info!("game is running; closing the launcher (launch behaviour: close)");
        self.app.exit(0);
    }
}

impl LaunchHooks for WindowHooks {
    fn game_started(&self, _instance: &Instance) {
        if self.behavior != LaunchBehavior::KeepOpen
            && let Some(w) = self.app.get_webview_window("main")
        {
            let _ = w.minimize();
        }
        if self.closes() {
            let hooks = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(CLOSE_FALLBACK);
                if !hooks.exited.load(Ordering::SeqCst) {
                    hooks.quit();
                }
            });
        }
    }

    fn game_ready(&self, _instance: &Instance) {
        if self.closes() {
            self.quit();
        }
    }

    fn game_exited(&self, _instance: &Instance, _exit: &GameExit) {
        self.exited.store(true, Ordering::SeqCst);
        let away = {
            let _gate = AWAY_GATE.lock().expect("away lock");
            window_away()
        };
        if away {
            tracing::info!("game exited; opening the launcher window again");
            reopen_window(&self.app);
        } else if self.behavior != LaunchBehavior::KeepOpen
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

/// Playing needs a signed-in, unbanned MehburMC account (K73). The server
/// enforces bans on everything it serves; this keeps the UI rule honest.
async fn require_account(state: &AppState) -> CmdResult<()> {
    let friends = &state.launcher()?.friends;
    if friends.can_play().await {
        return Ok(());
    }
    let code = if friends.auth_status(false).await.ban.is_some() {
        "auth.banned"
    } else {
        "auth.required"
    };
    Err(launcher_core::CoreError::Friends(code).to_payload())
}

/// Must be async: the task is spawned on the Tokio runtime.
#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<String> {
    require_account(&state).await?;
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
    require_account(&state).await?;
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
