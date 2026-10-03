//! Forwards core events to the webview without flooding IPC: game log lines
//! are batched and progress events are coalesced, flushed every 50 ms.

use std::collections::HashMap;
use std::time::Duration;

use launcher_core::events::{CoreEvent, EventSink};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

pub const EVENT_CHANNEL: &str = "core://event";
pub const LOG_CHANNEL: &str = "core://logs";
const FLUSH_EVERY: Duration = Duration::from_millis(50);
/// Lines beyond this per flush are dropped to keep the UI responsive.
const MAX_LINES_PER_FLUSH: usize = 2000;

pub struct UiSink {
    tx: mpsc::UnboundedSender<CoreEvent>,
}

impl EventSink for UiSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.tx.send(event);
    }
}

pub fn channel() -> (UiSink, mpsc::UnboundedReceiver<CoreEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (UiSink { tx }, rx)
}

/// Runs until the sender side is dropped.
pub async fn forward(app: AppHandle, mut rx: mpsc::UnboundedReceiver<CoreEvent>) {
    let mut logs: Vec<CoreEvent> = Vec::new();
    let mut progress: HashMap<String, CoreEvent> = HashMap::new();
    let mut others: Vec<CoreEvent> = Vec::new();
    let mut ticker = tokio::time::interval(FLUSH_EVERY);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Some(e @ CoreEvent::GameLog { .. }) => {
                    if logs.len() < MAX_LINES_PER_FLUSH {
                        logs.push(e);
                    }
                }
                Some(e @ CoreEvent::Progress { .. }) => {
                    let CoreEvent::Progress { task, stage, .. } = &e else { unreachable!() };
                    progress.insert(format!("{task}:{stage:?}"), e);
                }
                Some(e) => others.push(e),
                None => break,
            },
            _ = ticker.tick() => {
                for e in progress.drain().map(|(_, e)| e).chain(others.drain(..)) {
                    let _ = app.emit(EVENT_CHANNEL, e);
                }
                if !logs.is_empty() {
                    let _ = app.emit(LOG_CHANNEL, std::mem::take(&mut logs));
                }
            }
        }
    }
}
