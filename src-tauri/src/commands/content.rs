use std::path::PathBuf;

use launcher_core::CoreError;
use launcher_core::content::icons;
use launcher_core::content::install::{self, InstallRequest, InstallResult};
use launcher_core::content::installed::InstalledItem;
use launcher_core::content::modpack::ImportResult;
use launcher_core::content::modrinth::{
    self, ProjectType, SearchPage, SearchQuery, VersionSummary,
};
use launcher_core::instance::files::Folder;
use tauri::State;

use super::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn search_modrinth(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> CmdResult<SearchPage> {
    let l = state.launcher()?.clone();
    Ok(modrinth::search(&l.ctx, &query).await?)
}

/// Versions of a project; with an instance, only those compatible with it.
#[tauri::command]
pub async fn project_versions(
    state: State<'_, AppState>,
    project: String,
    project_type: ProjectType,
    instance_id: Option<String>,
) -> CmdResult<Vec<VersionSummary>> {
    let l = state.launcher()?.clone();
    let (loaders, game): (Vec<&str>, Option<String>) = match &instance_id {
        Some(id) => {
            let inst = l.instances.get(id)?;
            let loaders = match project_type {
                ProjectType::Mod => install::modrinth_loaders(inst.loader.kind).to_vec(),
                ProjectType::Resourcepack => vec!["minecraft"],
                _ => vec![],
            };
            (loaders, Some(inst.mc_version))
        }
        None => (vec![], None),
    };
    let versions = modrinth::project_versions(&l.ctx, &project, &loaders, game.as_deref()).await?;
    Ok(versions.iter().map(VersionSummary::from).collect())
}

#[tauri::command]
pub async fn install_content(
    state: State<'_, AppState>,
    id: String,
    requests: Vec<InstallRequest>,
) -> CmdResult<InstallResult> {
    let l = state.launcher()?.clone();
    Ok(l.install_content(&id, &requests).await?)
}

#[tauri::command]
pub async fn scan_content(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    check_updates: bool,
) -> CmdResult<Vec<InstalledItem>> {
    let l = state.launcher()?.clone();
    Ok(l.scan_content(&id, folder, check_updates).await?)
}

#[tauri::command]
pub async fn update_content(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    files: Vec<String>,
) -> CmdResult<Vec<String>> {
    let l = state.launcher()?.clone();
    Ok(l.update_content(&id, folder, &files).await?)
}

/// Imports a `.mrpack` or CurseForge `.zip` the user picked.
#[tauri::command]
pub async fn import_modpack(state: State<'_, AppState>, path: String) -> CmdResult<ImportResult> {
    let p = PathBuf::from(&path);
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "mrpack" | "zip"));
    if !p.is_absolute() || !ext_ok {
        return Err(CoreError::ModpackInvalid("expected a .mrpack or .zip file".into()).into());
    }
    let l = state.launcher()?.clone();
    let key = state.settings().curseforge_api_key;
    Ok(l.import_modpack(&p, key.as_deref()).await?)
}

#[tauri::command]
pub async fn install_modrinth_modpack(
    state: State<'_, AppState>,
    version_id: String,
) -> CmdResult<ImportResult> {
    let l = state.launcher()?.clone();
    Ok(l.install_modrinth_modpack(&version_id).await?)
}

#[tauri::command]
pub async fn content_icon(state: State<'_, AppState>, url: String) -> CmdResult<String> {
    let l = state.launcher()?.clone();
    Ok(icons::data_uri(&l.ctx, &url).await?)
}

/// Opens a project/download page in the default browser. Only Modrinth and
/// CurseForge pages are allowed (the webview itself cannot open URLs).
#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let allowed = reqwest_url_host(&url).is_some_and(|h| {
        matches!(
            h.as_str(),
            "modrinth.com"
                | "www.curseforge.com"
                | "curseforge.com"
                | "www.minecraft.net"
                | "minecraft.net"
        )
    });
    if !allowed {
        return Err(CoreError::UrlNotAllowed { url }.into());
    }
    app.opener()
        .open_url(url.clone(), None::<&str>)
        .map_err(|e| CoreError::UrlNotAllowed {
            url: format!("{url} ({e})"),
        })?;
    Ok(())
}

/// Host of an `https` URL.
fn reqwest_url_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.contains(['@', ':'])).then(|| host.to_ascii_lowercase())
}
