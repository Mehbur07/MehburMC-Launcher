//! UI-independent core of MehburMC Launcher.
//!
//! Every user-facing frontend (the Tauri app, the developer CLI) is a thin
//! wrapper around this crate.

pub mod archive;
pub mod assets;
pub mod auth;
pub mod content;
pub mod crash;
pub mod ctx;
pub mod datamove;
pub mod error;
pub mod events;
pub mod friends;
pub mod fsutil;
pub mod hash;
pub mod instance;
pub mod java;
pub mod launch;
pub mod library;
pub mod loader;
pub mod logging;
pub mod maven;
pub mod natives;
pub mod net;
pub mod news;
pub mod os;
pub mod paths;
pub mod rules;
pub mod servers;
pub mod session;
pub mod settings;
pub mod skin;
pub mod state;
pub mod tasks;
pub mod version;

pub use ctx::Ctx;
pub use error::{CoreError, ErrorPayload, Result};
pub use paths::{DataMode, Paths, PathsInfo};
pub use settings::Settings;

/// Display name used everywhere (window title, `${launcher_name}`, logs).
pub const LAUNCHER_NAME: &str = "MehburMC Launcher";

/// Launcher version, taken from the workspace manifest.
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
