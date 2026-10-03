//! Progress and game events, independent of any UI.
//!
//! Core code reports through an [`EventSink`]; the Tauri app forwards events
//! to the webview, the CLI prints them.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Stage {
    Metadata,
    /// Installing a mod loader (downloads + installer processors).
    Loader,
    /// Downloading mods/packs (modpack import, content install).
    Content,
    Libraries,
    Natives,
    Assets,
    Java,
    Launching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "TRACE" | "FINEST" | "FINER" => Self::Trace,
            "DEBUG" | "FINE" => Self::Debug,
            "INFO" | "CONFIG" => Self::Info,
            "WARN" | "WARNING" => Self::Warn,
            "ERROR" | "SEVERE" => Self::Error,
            "FATAL" => Self::Fatal,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum CoreEvent {
    #[serde(rename_all = "camelCase")]
    Task { task: crate::tasks::TaskInfo },
    #[serde(rename_all = "camelCase")]
    Progress {
        task: String,
        stage: Stage,
        #[ts(type = "number")]
        done: u64,
        #[ts(type = "number")]
        total: u64,
        #[ts(type = "number")]
        bytes_done: u64,
        #[ts(type = "number")]
        bytes_total: u64,
    },
    #[serde(rename_all = "camelCase")]
    GameLog {
        task: String,
        stream: LogStream,
        line: String,
        level: Option<LogLevel>,
        /// Unix milliseconds from the log4j event, if the game provided one.
        #[ts(type = "number | null")]
        time_ms: Option<u64>,
        thread: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    GameExited {
        task: String,
        code: Option<i32>,
        crash_report: Option<String>,
    },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: CoreEvent);
}

/// Discards everything (tests, background work nobody watches).
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: CoreEvent) {}
}

const EMIT_INTERVAL: Duration = Duration::from_millis(100);

/// Thread-safe progress counter for one stage; emits at most every 100 ms.
pub struct Progress {
    sink: Arc<dyn EventSink>,
    task: String,
    stage: Stage,
    total: AtomicU64,
    done: AtomicU64,
    bytes_total: AtomicU64,
    bytes_done: AtomicU64,
    last_emit: Mutex<Option<Instant>>,
}

impl Progress {
    pub fn new(sink: Arc<dyn EventSink>, task: &str, stage: Stage) -> Self {
        Self {
            sink,
            task: task.to_owned(),
            stage,
            total: AtomicU64::new(0),
            done: AtomicU64::new(0),
            bytes_total: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            last_emit: Mutex::new(None),
        }
    }

    pub fn set_totals(&self, items: u64, bytes: u64) {
        self.total.store(items, Ordering::Relaxed);
        self.bytes_total.store(bytes, Ordering::Relaxed);
        self.emit(true);
    }

    pub fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
        self.emit(false);
    }

    /// Undo bytes counted for a transfer attempt that failed.
    pub fn sub_bytes(&self, n: u64) {
        let mut cur = self.bytes_done.load(Ordering::Relaxed);
        while let Err(actual) = self.bytes_done.compare_exchange_weak(
            cur,
            cur.saturating_sub(n),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            cur = actual;
        }
    }

    pub fn item_done(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
        self.emit(false);
    }

    pub fn finish(&self) {
        self.done
            .store(self.total.load(Ordering::Relaxed), Ordering::Relaxed);
        self.emit(true);
    }

    fn emit(&self, force: bool) {
        {
            let mut last = self.last_emit.lock().expect("progress lock");
            let now = Instant::now();
            if !force && last.is_some_and(|t| now.duration_since(t) < EMIT_INTERVAL) {
                return;
            }
            *last = Some(now);
        }
        self.sink.emit(CoreEvent::Progress {
            task: self.task.clone(),
            stage: self.stage,
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
        });
    }
}
