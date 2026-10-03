//! The single source of truth for every on-disk location.
//!
//! Layout (see ARCHITECTURE.md §5):
//!
//! ```text
//! <base>/MehburMC/game/mc/
//!   launcher/   settings.json, accounts.json, redirect.json   (never moves)
//!   versions/ libraries/ assets/ runtime/ instances/ loaders/
//!   skins/ modpacks/ cache/ logs/                              (content, may be redirected)
//! ```
//!
//! Resolution order: `portable.flag` next to the executable → platform data
//! dir (`%APPDATA%` on Windows). `launcher/redirect.json` then optionally
//! moves the content folders elsewhere ("Move data folder").

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CoreError, Result};

const ROOT_DIR: &str = "MehburMC";
const GAME_DIR: &str = "game";
const MC_DIR: &str = "mc";
const LAUNCHER_DIR: &str = "launcher";
const PORTABLE_FLAG: &str = "portable.flag";
const REDIRECT_FILE: &str = "redirect.json";

/// Content folders created under the (possibly redirected) content root.
const CONTENT_DIRS: &[&str] = &[
    "versions",
    "libraries",
    "assets",
    "runtime",
    "instances",
    "loaders",
    "skins",
    "modpacks",
    "cache",
    "logs",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DataMode {
    /// Data lives in the platform data directory (`%APPDATA%`).
    Standard,
    /// `portable.flag` found next to the executable.
    Portable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Redirect {
    content_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Paths {
    mode: DataMode,
    /// `<base>/MehburMC`
    root: PathBuf,
    /// `<root>/game/mc` – home of `launcher/`.
    mc: PathBuf,
    /// Where content folders live; equals `mc` unless redirected.
    content: PathBuf,
}

/// Serializable summary for the UI.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PathsInfo {
    pub mode: DataMode,
    pub root: String,
    pub mc: String,
    pub content: String,
    pub redirected: bool,
}

impl Paths {
    /// Resolves paths for the running executable.
    ///
    /// `exe_dir` is the directory containing the launcher executable; pass
    /// `None` to skip the portable check (e.g. in the CLI).
    pub fn resolve(exe_dir: Option<&Path>) -> Result<Self> {
        if let Some(dir) = exe_dir
            && dir.join(PORTABLE_FLAG).is_file()
        {
            return Self::from_root(dir.join(ROOT_DIR), DataMode::Portable);
        }
        let base = directories::BaseDirs::new().ok_or(CoreError::NoDataDir)?;
        // `data_dir` is Roaming AppData on Windows, ~/.local/share on Linux,
        // ~/Library/Application Support on macOS.
        Self::from_root(base.data_dir().join(ROOT_DIR), DataMode::Standard)
    }

    /// Builds paths below an explicit `MehburMC` root, honouring a redirect.
    pub fn from_root(root: PathBuf, mode: DataMode) -> Result<Self> {
        let mc = root.join(GAME_DIR).join(MC_DIR);
        let redirect_path = mc.join(LAUNCHER_DIR).join(REDIRECT_FILE);
        let content = match read_redirect(&redirect_path)? {
            Some(target) => target,
            None => mc.clone(),
        };
        Ok(Self {
            mode,
            root,
            mc,
            content,
        })
    }

    /// Test/dev helper: standard layout rooted at `root`, no redirect lookup.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let mc = root.join(GAME_DIR).join(MC_DIR);
        Self {
            mode: DataMode::Standard,
            content: mc.clone(),
            mc,
            root,
        }
    }

    /// Creates every folder of the layout and verifies it is writable.
    pub fn ensure_layout(&self) -> Result<()> {
        let mut dirs = vec![self.launcher_dir()];
        dirs.extend(CONTENT_DIRS.iter().map(|d| self.content.join(d)));
        for dir in &dirs {
            fs::create_dir_all(dir).map_err(|source| CoreError::DataDirNotWritable {
                path: dir.clone(),
                source,
            })?;
        }
        for dir in [self.launcher_dir(), self.content.clone()] {
            probe_writable(&dir)?;
        }
        Ok(())
    }

    pub fn info(&self) -> PathsInfo {
        PathsInfo {
            mode: self.mode,
            root: self.root.display().to_string(),
            mc: self.mc.display().to_string(),
            content: self.content.display().to_string(),
            redirected: self.content != self.mc,
        }
    }

    pub fn mode(&self) -> DataMode {
        self.mode
    }
    /// `<base>/MehburMC`
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// `<root>/game/mc`
    pub fn mc(&self) -> &Path {
        &self.mc
    }
    /// Root of the content folders (honours redirect).
    pub fn content(&self) -> &Path {
        &self.content
    }

    // --- launcher/ (never redirected) ---
    pub fn launcher_dir(&self) -> PathBuf {
        self.mc.join(LAUNCHER_DIR)
    }
    pub fn settings_file(&self) -> PathBuf {
        self.launcher_dir().join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.launcher_dir().join("accounts.json")
    }
    pub fn redirect_file(&self) -> PathBuf {
        self.launcher_dir().join(REDIRECT_FILE)
    }

    // --- content ---
    pub fn versions(&self) -> PathBuf {
        self.content.join("versions")
    }
    pub fn libraries(&self) -> PathBuf {
        self.content.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.content.join("assets")
    }
    pub fn runtime(&self) -> PathBuf {
        self.content.join("runtime")
    }
    pub fn instances(&self) -> PathBuf {
        self.content.join("instances")
    }
    pub fn loaders(&self) -> PathBuf {
        self.content.join("loaders")
    }
    pub fn skins(&self) -> PathBuf {
        self.content.join("skins")
    }
    pub fn modpacks(&self) -> PathBuf {
        self.content.join("modpacks")
    }
    pub fn cache(&self) -> PathBuf {
        self.content.join("cache")
    }
    pub fn logs(&self) -> PathBuf {
        self.content.join("logs")
    }
    /// WebView2 user data folder used in portable mode.
    pub fn webview_data(&self) -> PathBuf {
        self.cache().join("webview")
    }
}

fn read_redirect(path: &Path) -> Result<Option<PathBuf>> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(CoreError::io(path, e)),
    };
    let invalid = |reason: &str| CoreError::InvalidRedirect {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    let r: Redirect = serde_json::from_slice(&raw).map_err(|e| invalid(&e.to_string()))?;
    if !r.content_root.is_absolute() {
        return Err(invalid("contentRoot must be an absolute path"));
    }
    Ok(Some(r.content_root))
}

fn probe_writable(dir: &Path) -> Result<()> {
    let probe = dir.join(".write-test");
    fs::write(&probe, b"ok")
        .and_then(|()| fs::remove_file(&probe))
        .map_err(|source| CoreError::DataDirNotWritable {
            path: dir.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn layout_matches_spec() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join(ROOT_DIR));
        paths.ensure_layout().unwrap();

        // Root holds only `game`, `game` holds only `mc`.
        assert_eq!(names(paths.root()), ["game"]);
        assert_eq!(names(&paths.root().join("game")), ["mc"]);

        let mut expected: Vec<String> = CONTENT_DIRS.iter().map(|s| s.to_string()).collect();
        expected.push("launcher".into());
        expected.sort();
        assert_eq!(names(paths.mc()), expected);
    }

    #[test]
    fn accessors_are_under_mc() {
        let paths = Paths::at("/data/MehburMC");
        let mc = Path::new("/data/MehburMC/game/mc");
        assert_eq!(paths.settings_file(), mc.join("launcher/settings.json"));
        assert_eq!(paths.accounts_file(), mc.join("launcher/accounts.json"));
        assert_eq!(paths.versions(), mc.join("versions"));
        assert_eq!(paths.instances(), mc.join("instances"));
        assert_eq!(paths.webview_data(), mc.join("cache/webview"));
    }

    #[test]
    fn portable_flag_switches_root() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(PORTABLE_FLAG), b"").unwrap();
        let paths = Paths::resolve(Some(tmp.path())).unwrap();
        assert_eq!(paths.mode(), DataMode::Portable);
        assert_eq!(paths.root(), tmp.path().join(ROOT_DIR));
    }

    #[test]
    fn no_flag_uses_standard_mode() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(tmp.path())).unwrap();
        assert_eq!(paths.mode(), DataMode::Standard);
        assert!(paths.root().ends_with(ROOT_DIR));
    }

    #[test]
    fn redirect_moves_content_but_not_launcher() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(ROOT_DIR);
        let target = tmp.path().join("elsewhere");
        let base = Paths::at(&root);
        crate::fsutil::write_json_atomic(
            &base.redirect_file(),
            &Redirect {
                content_root: target.clone(),
            },
        )
        .unwrap();

        let paths = Paths::from_root(root, DataMode::Standard).unwrap();
        assert_eq!(paths.versions(), target.join("versions"));
        assert_eq!(paths.settings_file(), base.settings_file());
        assert!(paths.info().redirected);
    }

    #[test]
    fn relative_redirect_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(ROOT_DIR);
        let base = Paths::at(&root);
        crate::fsutil::write_atomic(&base.redirect_file(), br#"{"contentRoot":"rel/dir"}"#)
            .unwrap();
        let err = Paths::from_root(root, DataMode::Standard).unwrap_err();
        assert_eq!(err.code(), "paths.invalidRedirect");
    }
}
