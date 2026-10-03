//! Process-wide state built before the window opens.

use std::path::Path;
use std::sync::{Arc, Mutex};

use launcher_core::auth::secrets::KeyringStore;
use launcher_core::events::CoreEvent;
use launcher_core::session::Launcher;
use launcher_core::{CoreError, ErrorPayload, Paths, Settings, logging};
use tokio::sync::mpsc;
use tracing_appender::non_blocking::WorkerGuard;

use crate::bridge;

pub struct AppState {
    /// `None` only if the platform data directory could not be determined.
    pub paths: Option<Paths>,
    pub settings: Mutex<Settings>,
    /// Fatal startup problem (e.g. data folder not writable) shown by the UI
    /// instead of the normal shell.
    pub startup_error: Option<ErrorPayload>,
    pub launcher: Option<Arc<Launcher>>,
    /// Taken once by `setup` to start the event forwarder.
    pub events_rx: Mutex<Option<mpsc::UnboundedReceiver<CoreEvent>>>,
    _log_guard: Option<WorkerGuard>,
}

impl AppState {
    pub fn init(exe_dir: Option<&Path>) -> Self {
        let paths = match Paths::resolve(exe_dir) {
            Ok(p) => p,
            Err(e) => return Self::failed(None, e),
        };
        if let Err(e) = paths.ensure_layout() {
            return Self::failed(Some(paths), e);
        }

        let (settings, settings_err) = match Settings::load(&paths) {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        let log_guard = logging::init(&paths.logs(), settings.debug_logging)
            .inspect_err(|e| eprintln!("logging disabled: {e}"))
            .ok();

        tracing::info!(
            version = launcher_core::LAUNCHER_VERSION,
            mode = ?paths.mode(),
            data = %paths.mc().display(),
            "{} starting",
            launcher_core::LAUNCHER_NAME
        );
        if let Some(e) = settings_err {
            tracing::warn!(error = %e.detail(), "could not read settings, using defaults");
        }

        let (sink, rx) = bridge::channel();
        let launcher = match Launcher::new(
            paths.clone(),
            Arc::new(sink),
            settings.download_concurrency as usize,
            Arc::new(KeyringStore),
        ) {
            Ok(l) => l,
            Err(e) => return Self::failed(Some(paths), e),
        };

        Self {
            paths: Some(paths),
            settings: Mutex::new(settings),
            startup_error: None,
            launcher: Some(launcher),
            events_rx: Mutex::new(Some(rx)),
            _log_guard: log_guard,
        }
    }

    fn failed(paths: Option<Paths>, err: CoreError) -> Self {
        eprintln!("startup failed: {}", err.detail());
        Self {
            paths,
            settings: Mutex::new(Settings::default()),
            startup_error: Some(err.to_payload()),
            launcher: None,
            events_rx: Mutex::new(None),
            _log_guard: None,
        }
    }

    /// Paths, or the startup error if the data folder is unusable.
    pub fn usable_paths(&self) -> Result<&Paths, ErrorPayload> {
        match (&self.startup_error, &self.paths) {
            (None, Some(p)) => Ok(p),
            (Some(e), _) => Err(e.clone()),
            (None, None) => Err(CoreError::NoDataDir.to_payload()),
        }
    }

    pub fn launcher(&self) -> Result<&Arc<Launcher>, ErrorPayload> {
        self.usable_paths()?;
        self.launcher
            .as_ref()
            .ok_or_else(|| CoreError::NoDataDir.to_payload())
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings lock").clone()
    }
}
