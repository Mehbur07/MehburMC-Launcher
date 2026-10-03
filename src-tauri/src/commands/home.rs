//! Home screen extras: news and crash analysis.

use launcher_core::CoreError;
use launcher_core::crash::{self, CrashInfo};
use launcher_core::instance::files::{self, Folder};
use launcher_core::news::{self, NewsItem};
use tauri::{AppHandle, State};

use super::{CmdResult, blocking, open_path};
use crate::state::AppState;

/// Java Edition news; never fails (empty list when offline).
#[tauri::command]
pub async fn list_news(state: State<'_, AppState>) -> CmdResult<Vec<NewsItem>> {
    let launcher = state.launcher()?.clone();
    Ok(news::java_news(&launcher.ctx).await)
}

/// Diagnoses a crash report from the instance's `crash-reports/` folder.
#[tauri::command]
pub async fn analyze_crash_report(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CmdResult<CrashInfo> {
    let l = state.launcher()?.clone();
    blocking(move || {
        let inst = l.instances.get(&id)?;
        let path = files::file_path(&l.instances, &id, Folder::CrashReports, &name)?;
        if !path.is_file() {
            return Err(CoreError::io(&path, std::io::ErrorKind::NotFound.into()));
        }
        Ok(crash::analyze_file(&inst.id, &inst.name, &path))
    })
    .await
}

/// Opens a crash report / log with the default app. Only files inside the
/// data folder are accepted (paths come from crash analysis events).
#[tauri::command]
pub async fn open_data_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<()> {
    let root = state.usable_paths()?.content().to_path_buf();
    let target = blocking(move || {
        let p = std::path::PathBuf::from(&path);
        let canon = p.canonicalize().map_err(|e| CoreError::io(&p, e))?;
        let root = root.canonicalize().map_err(|e| CoreError::io(&root, e))?;
        let ext_ok = canon
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "txt" | "log"));
        if !canon.starts_with(&root) || !canon.is_file() || !ext_ok {
            return Err(CoreError::UrlNotAllowed { url: path });
        }
        Ok(canon)
    })
    .await?;
    open_path(&app, &target)
}
