//! Long-running task registry (prepare/download/play) backing the Downloads
//! screen. Pausing cancels the transfer but keeps `.part` files, so resuming
//! (starting the task again) continues where it stopped.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{CoreError, ErrorPayload, Result};
use crate::events::{CoreEvent, EventSink, Stage};

/// Finished tasks kept for the Downloads screen.
const MAX_FINISHED: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskKind {
    /// Prepare + play.
    Launch,
    /// Prepare with full checksum verification, no play.
    Repair,
    /// Modpack import / content download.
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskStatus {
    Preparing,
    /// The game process is running.
    Playing,
    Completed,
    Failed,
    Cancelled,
    Paused,
}

impl TaskStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Preparing | Self::Playing)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskInfo {
    pub id: String,
    pub kind: TaskKind,
    pub instance_id: Option<String>,
    pub title: String,
    /// Account name a game task plays with (several accounts may play one
    /// instance at once).
    pub account: Option<String>,
    pub status: TaskStatus,
    pub stage: Option<Stage>,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    pub error: Option<ErrorPayload>,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number | null")]
    pub finished_at: Option<u64>,
}

struct Entry {
    info: TaskInfo,
    cancel: CancellationToken,
    pause_requested: bool,
}

pub struct TaskRegistry {
    sink: Arc<dyn EventSink>,
    entries: Mutex<Vec<Entry>>,
    counter: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

impl TaskRegistry {
    /// `sink` receives `CoreEvent::Task` updates (use the UI sink here, not a
    /// sink that feeds back into this registry).
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self {
            sink,
            entries: Mutex::new(Vec::new()),
            counter: AtomicU64::new(1),
        }
    }

    pub fn create(
        &self,
        kind: TaskKind,
        title: &str,
        instance_id: Option<&str>,
    ) -> (String, CancellationToken) {
        let id = format!("task-{}", self.counter.fetch_add(1, Ordering::Relaxed));
        let cancel = CancellationToken::new();
        let info = TaskInfo {
            id: id.clone(),
            kind,
            instance_id: instance_id.map(str::to_owned),
            title: title.to_owned(),
            account: None,
            status: TaskStatus::Preparing,
            stage: None,
            done: 0,
            total: 0,
            bytes_done: 0,
            bytes_total: 0,
            error: None,
            started_at: now_ms(),
            finished_at: None,
        };
        {
            let mut e = self.entries.lock().expect("tasks lock");
            // Starting again replaces older finished entries for the same instance.
            if let Some(inst) = instance_id {
                e.retain(|x| {
                    x.info.status.is_active() || x.info.instance_id.as_deref() != Some(inst)
                });
            }
            e.push(Entry {
                info: info.clone(),
                cancel: cancel.clone(),
                pause_requested: false,
            });
            prune(&mut e);
        }
        self.sink.emit(CoreEvent::Task { task: info });
        (id, cancel)
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut TaskInfo)) {
        let snapshot = {
            let mut e = self.entries.lock().expect("tasks lock");
            let Some(entry) = e.iter_mut().find(|x| x.info.id == id) else {
                return;
            };
            f(&mut entry.info);
            entry.info.clone()
        };
        self.sink.emit(CoreEvent::Task { task: snapshot });
    }

    pub fn set_account(&self, id: &str, account: &str) {
        self.update(id, |t| t.account = Some(account.to_owned()));
    }

    pub fn set_status(&self, id: &str, status: TaskStatus) {
        self.update(id, |t| t.status = status);
    }

    /// Records progress silently (progress events reach the UI separately).
    pub fn record_progress(&self, event: &CoreEvent) {
        let CoreEvent::Progress {
            task,
            stage,
            done,
            total,
            bytes_done,
            bytes_total,
        } = event
        else {
            return;
        };
        let mut e = self.entries.lock().expect("tasks lock");
        if let Some(entry) = e.iter_mut().find(|x| &x.info.id == task) {
            let t = &mut entry.info;
            t.stage = Some(*stage);
            t.done = *done;
            t.total = *total;
            t.bytes_done = *bytes_done;
            t.bytes_total = *bytes_total;
        }
    }

    /// Stops a task. With `pause`, the result is reported as `Paused`.
    pub fn cancel(&self, id: &str, pause: bool) -> bool {
        let mut e = self.entries.lock().expect("tasks lock");
        match e
            .iter_mut()
            .find(|x| x.info.id == id && x.info.status.is_active())
        {
            Some(entry) => {
                entry.pause_requested = pause;
                entry.cancel.cancel();
                true
            }
            None => false,
        }
    }

    pub fn finish(&self, id: &str, result: &Result<()>) {
        self.finish_ref(id, result.as_ref().map(|_| ()));
    }

    /// [`finish`](Self::finish) for callers that keep ownership of the error.
    pub fn finish_ref(&self, id: &str, result: std::result::Result<(), &CoreError>) {
        let paused = self
            .entries
            .lock()
            .expect("tasks lock")
            .iter()
            .find(|x| x.info.id == id)
            .is_some_and(|x| x.pause_requested);
        self.update(id, |t| {
            t.finished_at = Some(now_ms());
            match result {
                Ok(()) => t.status = TaskStatus::Completed,
                Err(CoreError::Cancelled) if paused => t.status = TaskStatus::Paused,
                Err(CoreError::Cancelled) => t.status = TaskStatus::Cancelled,
                Err(e) => {
                    t.status = TaskStatus::Failed;
                    t.error = Some(e.to_payload());
                }
            }
        });
    }

    pub fn active_for_instance(&self, instance_id: &str) -> Option<String> {
        self.entries
            .lock()
            .expect("tasks lock")
            .iter()
            .find(|x| {
                x.info.status.is_active() && x.info.instance_id.as_deref() == Some(instance_id)
            })
            .map(|x| x.info.id.clone())
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        self.entries
            .lock()
            .expect("tasks lock")
            .iter()
            .rev()
            .map(|x| x.info.clone())
            .collect()
    }

    /// Removes finished tasks from the list.
    pub fn clear_finished(&self) {
        self.entries
            .lock()
            .expect("tasks lock")
            .retain(|x| x.info.status.is_active());
    }
}

fn prune(e: &mut Vec<Entry>) {
    let finished = e.iter().filter(|x| !x.info.status.is_active()).count();
    let mut excess = finished.saturating_sub(MAX_FINISHED);
    e.retain(|x| {
        if excess > 0 && !x.info.status.is_active() {
            excess -= 1;
            false
        } else {
            true
        }
    });
}

/// Event sink that records progress in the registry before forwarding.
pub struct TrackingSink {
    pub tasks: Arc<TaskRegistry>,
    pub inner: Arc<dyn EventSink>,
}

impl EventSink for TrackingSink {
    fn emit(&self, event: CoreEvent) {
        self.tasks.record_progress(&event);
        self.inner.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NullSink;

    #[test]
    fn lifecycle_and_pause() {
        let r = TaskRegistry::new(Arc::new(NullSink));
        let (id, token) = r.create(TaskKind::Launch, "A", Some("a"));
        assert_eq!(r.active_for_instance("a").as_deref(), Some(id.as_str()));

        r.record_progress(&CoreEvent::Progress {
            task: id.clone(),
            stage: Stage::Assets,
            done: 3,
            total: 10,
            bytes_done: 30,
            bytes_total: 100,
        });
        assert_eq!(r.list()[0].done, 3);

        assert!(r.cancel(&id, true));
        assert!(token.is_cancelled());
        r.finish(&id, &Err(CoreError::Cancelled));
        assert_eq!(r.list()[0].status, TaskStatus::Paused);
        assert!(r.active_for_instance("a").is_none());
        assert!(!r.cancel(&id, false), "finished tasks cannot be cancelled");

        // Restarting replaces the old finished entry for the instance.
        let (id2, _) = r.create(TaskKind::Launch, "A", Some("a"));
        assert_eq!(r.list().len(), 1);
        r.finish(&id2, &Err(CoreError::VersionNotFound("x".into())));
        let t = &r.list()[0];
        assert_eq!(t.status, TaskStatus::Failed);
        assert_eq!(t.error.as_ref().unwrap().code, "version.notFound");

        r.clear_finished();
        assert!(r.list().is_empty());
    }
}
