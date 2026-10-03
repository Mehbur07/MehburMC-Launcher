//! `launcher/state.json` — UI state that is not a user preference
//! (selected instance, instance order).

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Result;
use crate::fsutil::write_json_atomic;
use crate::paths::Paths;

static WRITE: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct LauncherState {
    pub selected_instance: Option<String>,
    pub instance_order: Vec<String>,
}

impl LauncherState {
    /// Missing or corrupt files yield the default state.
    pub fn load(paths: &Paths) -> Self {
        std::fs::read(paths.state_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Read-modify-write under a process-wide lock.
    pub fn update(paths: &Paths, f: impl FnOnce(&mut Self)) -> Result<()> {
        let _g = WRITE.lock().expect("state lock");
        let mut s = Self::load(paths);
        f(&mut s);
        write_json_atomic(&paths.state_file(), &s)
    }
}
