//! NeoForge's early loading screen ("fmlearlywindow") creates the game
//! window before Minecraft does. With some graphics drivers the hand-over
//! crashes natively (e.g. `0xC000041D`). NeoForge's own guidance
//! (<https://neoforged.net/meta/displayerrors/>) is to set
//! `earlyWindowControl=false` in `config/fml.toml`; we do that automatically
//! after such a crash and relaunch once.

use std::path::Path;
use std::time::Duration;

use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;
use crate::instance::LoaderKind;
use crate::launch::process::GameExit;

/// A crash this early happens before (or right at) the window hand-over.
const EARLY: Duration = Duration::from_secs(90);

/// `true` for a native crash (Windows NTSTATUS codes are negative as i32)
/// shortly after start of a NeoForge game the user did not stop.
pub fn looks_like_early_window_crash(kind: LoaderKind, exit: &GameExit) -> bool {
    kind == LoaderKind::NeoForge
        && !exit.killed
        && exit.code.is_some_and(|c| c < 0)
        && exit.duration < EARLY
}

/// Switches `earlyWindowControl` off. Returns `false` if it already was
/// (then the crash has another cause and retrying is pointless).
pub fn disable(game_dir: &Path) -> Result<bool> {
    let file = game_dir.join("config").join("fml.toml");
    let text = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(CoreError::io(&file, e)),
    };
    let mut found = false;
    let mut changed = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|l| {
            let key = l.trim_start();
            if key.starts_with("earlyWindowControl") && key[18..].trim_start().starts_with('=') {
                found = true;
                if key.contains("true") {
                    changed = true;
                    return "earlyWindowControl = false".to_owned();
                }
            }
            l.to_owned()
        })
        .collect();
    if !found {
        out.push("earlyWindowControl = false".to_owned());
        changed = true;
    }
    if changed {
        write_atomic(&file, (out.join("\n") + "\n").as_bytes())?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(code: i32, secs: u64, killed: bool) -> GameExit {
        GameExit {
            code: Some(code),
            crash_report: None,
            duration: Duration::from_secs(secs),
            killed,
        }
    }

    #[test]
    fn detection() {
        assert!(looks_like_early_window_crash(
            LoaderKind::NeoForge,
            &exit(-1073740771, 7, false)
        ));
        assert!(!looks_like_early_window_crash(
            LoaderKind::NeoForge,
            &exit(1, 7, false)
        ));
        assert!(!looks_like_early_window_crash(
            LoaderKind::NeoForge,
            &exit(-1, 7, true)
        ));
        assert!(!looks_like_early_window_crash(
            LoaderKind::NeoForge,
            &exit(-1, 600, false)
        ));
        assert!(!looks_like_early_window_crash(
            LoaderKind::Fabric,
            &exit(-1, 7, false)
        ));
    }

    #[test]
    fn flips_the_setting_once() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("config");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("fml.toml"),
            "#comment\nearlyWindowControl = true\nmaxThreads = -1\n",
        )
        .unwrap();
        assert!(disable(dir.path()).unwrap());
        let t = std::fs::read_to_string(cfg.join("fml.toml")).unwrap();
        assert!(t.contains("earlyWindowControl = false") && t.contains("maxThreads = -1"));
        assert!(!disable(dir.path()).unwrap());

        let fresh = tempfile::tempdir().unwrap();
        assert!(disable(fresh.path()).unwrap());
        assert!(fresh.path().join("config/fml.toml").is_file());
    }
}
