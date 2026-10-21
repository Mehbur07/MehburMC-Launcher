//! Voice calls (K86). The webview runs WebRTC; these commands relay the
//! call state and set-up messages. Polling runs here rather than in the
//! webview: a minimised webview's timers are throttled (down to once a
//! minute), which would delay incoming calls while the user plays.

use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use launcher_core::friends::calls::{CallMemberState, CallPoll, IceConfig, OutgoingSignal};
use launcher_core::session::Launcher;
use tauri::{AppHandle, Emitter, Manager, State, UserAttentionType};
use tokio::sync::Notify;

use super::CmdResult;
use crate::state::AppState;

pub const CALLS_CHANNEL: &str = "calls://poll";
/// While ringing or in a call: set-up messages must arrive quickly.
const FAST: Duration = Duration::from_millis(700);
/// Otherwise only incoming calls are waited for.
const IDLE: Duration = Duration::from_millis(2500);
/// Friends off, signed out or the server unreachable.
const PAUSED: Duration = Duration::from_secs(5);

/// Wakes the poll loop right after starting, joining or leaving a call.
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// Last calls seen (no signals), for a webview that (re)loads mid-call.
static LAST: LazyLock<Mutex<CallPoll>> = LazyLock::new(Default::default);

fn ringing(p: &CallPoll) -> bool {
    p.calls
        .iter()
        .any(|c| c.my_state == Some(CallMemberState::Ringing))
}

/// Signed out or friends off: no call is shown any more.
fn forget(app: &AppHandle) {
    let mut last = LAST.lock().expect("calls lock");
    if !last.calls.is_empty() {
        *last = CallPoll::default();
        let _ = app.emit(CALLS_CHANNEL, &*last);
    }
}

/// Polls the server and forwards changes (and every signal) to the webview.
pub async fn poll_loop(app: AppHandle, launcher: Arc<Launcher>) {
    let mut after = 0_i64;
    let mut was_ringing = false;
    loop {
        let wait = if !launcher.friends.is_enabled().await {
            after = 0;
            forget(&app);
            PAUSED
        } else {
            match launcher.friends.poll_calls(after).await {
                Ok(mut poll) => {
                    if let Some(last) = poll.signals.iter().map(|s| s.id).max() {
                        after = after.max(last);
                    }
                    let rings = ringing(&poll);
                    if rings != was_ringing
                        && let Some(w) = app.get_webview_window("main")
                    {
                        let _ =
                            w.request_user_attention(rings.then_some(UserAttentionType::Critical));
                    }
                    was_ringing = rings;
                    let changed = {
                        let mut last = LAST.lock().expect("calls lock");
                        let changed = last.calls != poll.calls;
                        if changed {
                            last.calls = poll.calls.clone();
                        }
                        changed
                    };
                    let busy = rings || launcher.friends.active_call().is_some();
                    if changed || !poll.signals.is_empty() {
                        poll.signals.sort_by_key(|s| s.id);
                        let _ = app.emit(CALLS_CHANNEL, &poll);
                    }
                    if busy { FAST } else { IDLE }
                }
                Err(e) => {
                    tracing::debug!(error = %e.detail(), "call poll failed");
                    if e.code().starts_with("auth.") || e.code() == "friends.disabled" {
                        after = 0;
                        forget(&app);
                    }
                    PAUSED
                }
            }
        };
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = WAKE.notified() => {}
        }
    }
}

/// Leaves the active call on exit; bounded by the caller's time-out.
pub async fn leave_on_exit(launcher: &Launcher) {
    if let Err(e) = launcher.friends.leave_active_call().await {
        tracing::debug!(error = %e.detail(), "could not leave the call on exit");
    }
}

/// The launcher is in a voice call (keeps it open when a game starts).
pub fn in_call(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|s| s.launcher.clone())
        .is_some_and(|l| l.friends.active_call().is_some())
}

#[tauri::command]
pub async fn call_start(
    state: State<'_, AppState>,
    friend: Option<String>,
    group: Option<i64>,
) -> CmdResult<i64> {
    let launcher = state.launcher()?.clone();
    let id = launcher
        .friends
        .start_call(friend.as_deref(), group)
        .await?;
    WAKE.notify_one();
    Ok(id)
}

#[tauri::command]
pub async fn call_join(state: State<'_, AppState>, call: i64) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    launcher.friends.join_call(call).await?;
    WAKE.notify_one();
    Ok(())
}

/// Leaves a call, or declines it while it rings.
#[tauri::command]
pub async fn call_leave(state: State<'_, AppState>, call: i64) -> CmdResult<()> {
    let launcher = state.launcher()?.clone();
    launcher.friends.leave_call(call).await?;
    WAKE.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn call_signal(
    state: State<'_, AppState>,
    call: i64,
    signals: Vec<OutgoingSignal>,
) -> CmdResult<u32> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.send_signals(call, &signals).await?)
}

#[tauri::command]
pub async fn call_ice_config(state: State<'_, AppState>) -> CmdResult<IceConfig> {
    let launcher = state.launcher()?.clone();
    Ok(launcher.friends.ice_config().await)
}

/// Live calls as last polled (without signals).
#[tauri::command]
pub fn calls_snapshot() -> CallPoll {
    CallPoll {
        calls: LAST.lock().expect("calls lock").calls.clone(),
        signals: Vec::new(),
    }
}
