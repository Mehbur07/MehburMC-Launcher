//! UI-independent core of MehburMC Launcher.
//!
//! Every user-facing frontend (the Tauri app, the developer CLI) is a thin
//! wrapper around this crate.

pub mod error;
pub mod fsutil;
pub mod logging;
pub mod paths;
pub mod settings;

pub use error::{CoreError, ErrorPayload, Result};
pub use paths::{DataMode, Paths, PathsInfo};
pub use settings::Settings;

/// Display name used everywhere (window title, `${launcher_name}`, logs).
pub const LAUNCHER_NAME: &str = "MehburMC Launcher";

/// Launcher version, taken from the workspace manifest.
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
