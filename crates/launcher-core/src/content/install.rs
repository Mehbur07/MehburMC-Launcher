//! Installs Modrinth projects (mods, resource packs, shader packs) into an
//! instance, with required dependencies for mods.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::installed;
use super::modrinth::{self, ProjectType, Version};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{Progress, Stage};
use crate::hash::Checksum;
use crate::instance::files::{Folder, checked_name};
use crate::instance::{Instance, LoaderKind};
use crate::net::download::{DownloadItem, Verify};

/// Safety net against dependency cycles / runaway trees.
const MAX_PROJECTS: usize = 32;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallRequest {
    /// Project id or slug.
    pub project: String,
    pub project_type: ProjectType,
    /// Exact version; otherwise the newest compatible one.
    #[ts(optional)]
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallResult {
    /// Files downloaded.
    pub installed: Vec<String>,
    /// Projects skipped because they are already in the folder.
    pub already_present: Vec<String>,
    /// Installed projects that declare an installed mod incompatible.
    pub incompatible: Vec<String>,
}

/// Modrinth loader names whose mods run on `kind`.
pub fn modrinth_loaders(kind: LoaderKind) -> &'static [&'static str] {
    match kind {
        LoaderKind::Fabric => &["fabric"],
        LoaderKind::Quilt => &["quilt", "fabric"],
        LoaderKind::LegacyFabric => &["legacy-fabric"],
        LoaderKind::Forge => &["forge"],
        LoaderKind::NeoForge => &["neoforge"],
        LoaderKind::Vanilla | LoaderKind::Optifine => &[],
    }
}

pub fn folder_for(t: ProjectType) -> Option<Folder> {
    match t {
        ProjectType::Mod => Some(Folder::Mods),
        ProjectType::Resourcepack => Some(Folder::ResourcePacks),
        ProjectType::Shader => Some(Folder::ShaderPacks),
        ProjectType::Modpack => None,
    }
}

/// Version filter for a project type on this instance.
fn version_loaders(t: ProjectType, inst: &Instance) -> Result<Vec<&'static str>> {
    match t {
        ProjectType::Mod => {
            let l = modrinth_loaders(inst.loader.kind);
            if l.is_empty() {
                return Err(CoreError::AddonUnavailable {
                    project: "mods".into(),
                    loader: crate::loader::display_name(inst.loader.kind).into(),
                    mc: inst.mc_version.clone(),
                });
            }
            Ok(l.to_vec())
        }
        ProjectType::Resourcepack => Ok(vec!["minecraft"]),
        // Shader packs are tagged with the shader loader (iris, optifine…).
        ProjectType::Shader | ProjectType::Modpack => Ok(vec![]),
    }
}

/// Prefers the newest release, falling back to the newest beta/alpha.
pub fn pick(versions: Vec<Version>) -> Option<Version> {
    let first = versions.first().cloned();
    versions
        .into_iter()
        .find(|v| v.version_type == "release")
        .or(first)
}

/// `true` if `dir` already holds a jar/zip whose name starts with `slug`.
fn present_by_name(dir: &Path, slug: &str) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let slug = slug.to_ascii_lowercase();
    rd.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        name.strip_prefix(&slug)
            .is_some_and(|rest| rest.starts_with(['-', '_', '+', '.']))
    })
}

/// Downloads `roots` (and, for mods, their required dependencies) into the
/// matching folder of `inst` (`game_dir`).
pub async fn install(
    ctx: &Ctx,
    inst: &Instance,
    game_dir: &Path,
    roots: &[InstallRequest],
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<InstallResult> {
    let mut result = InstallResult::default();
    let mut items = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    let mc = inst.mc_version.as_str();
    let loader_name = crate::loader::display_name(inst.loader.kind);

    // Projects already installed, identified by file hash (offline-tolerant).
    let mut present: HashSet<String> = HashSet::new();
    for t in [
        ProjectType::Mod,
        ProjectType::Resourcepack,
        ProjectType::Shader,
    ] {
        if roots.iter().any(|r| r.project_type == t) {
            let dir = game_dir.join(folder_for(t).expect("content type").dir_name());
            if let Ok(found) = installed::identify(ctx, &dir).await {
                present.extend(found.into_values().map(|v| v.project_id));
            }
        }
    }

    for root in roots {
        let folder = folder_for(root.project_type).ok_or_else(|| {
            CoreError::InvalidInstance("modpacks are installed as new instances".into())
        })?;
        let dir: PathBuf = game_dir.join(folder.dir_name());
        let loaders = version_loaders(root.project_type, inst)?;
        let mut queue: VecDeque<(String, Option<String>)> =
            VecDeque::from([(root.project.clone(), root.version_id.clone())]);

        while let Some((project, pinned)) = queue.pop_front() {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            if done.len() >= MAX_PROJECTS {
                break;
            }
            let version = match &pinned {
                Some(id) => modrinth::version(ctx, id).await?,
                None => pick(modrinth::project_versions(ctx, &project, &loaders, Some(mc)).await?)
                    .ok_or_else(|| CoreError::AddonUnavailable {
                        project: project.clone(),
                        loader: loader_name.into(),
                        mc: mc.to_owned(),
                    })?,
            };
            if !done.insert(version.project_id.clone()) {
                continue;
            }
            let info = modrinth::project(ctx, &version.project_id).await?;
            if present.contains(&info.id) || present_by_name(&dir, &info.slug) {
                result.already_present.push(info.title);
                continue;
            }
            if root.project_type == ProjectType::Mod {
                for dep in version.dependencies.iter().rev() {
                    match dep.dependency_type.as_str() {
                        "required" => match (&dep.version_id, &dep.project_id) {
                            (Some(v), Some(p)) if !done.contains(p) => {
                                queue.push_front((p.clone(), Some(v.clone())))
                            }
                            (Some(v), None) => queue.push_front((v.clone(), Some(v.clone()))),
                            (None, Some(p)) if !done.contains(p) => {
                                queue.push_front((p.clone(), None))
                            }
                            _ => {}
                        },
                        "incompatible"
                            if dep.project_id.as_ref().is_some_and(|p| present.contains(p)) =>
                        {
                            result.incompatible.push(info.title.clone());
                        }
                        _ => {}
                    }
                }
            }
            let file = version
                .primary_file()
                .ok_or_else(|| CoreError::AddonUnavailable {
                    project: info.title.clone(),
                    loader: loader_name.into(),
                    mc: mc.to_owned(),
                })?;
            let name = checked_name(&file.filename)?.to_owned();
            items.push(download_item(&dir.join(&name), file));
            tracing::info!(project = %info.slug, version = %version.version_number, "content selected");
            result.installed.push(name);
        }
    }

    ctx.downloader()
        .run(items, Verify::Full, progress, cancel)
        .await?;
    Ok(result)
}

pub fn download_item(dest: &Path, file: &modrinth::VersionFile) -> DownloadItem {
    let checksum = match (&file.hashes.sha512, &file.hashes.sha1) {
        (Some(h), _) => Some(Checksum::Sha512(h.clone())),
        (None, Some(h)) => Some(Checksum::Sha1(h.clone())),
        _ => None,
    };
    DownloadItem {
        url: file.url.clone(),
        dest: dest.to_owned(),
        checksum,
        size: file.size,
    }
}

/// Progress for one-off content downloads that have no task in the UI.
pub fn quiet_progress() -> Progress {
    Progress::new(
        std::sync::Arc::new(crate::events::NullSink),
        "content",
        Stage::Libraries,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_detection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sodium-fabric-0.5.13+mc1.20.1.jar"), b"").unwrap();
        std::fs::write(dir.path().join("Iris-1.7.6.jar.disabled"), b"").unwrap();
        std::fs::write(dir.path().join("irisfoo.jar"), b"").unwrap();
        assert!(present_by_name(dir.path(), "sodium"));
        assert!(present_by_name(dir.path(), "iris"));
        assert!(!present_by_name(dir.path(), "embeddium"));
        assert!(!present_by_name(dir.path(), "irisf"));
    }

    #[test]
    fn loader_and_folder_mapping() {
        assert_eq!(modrinth_loaders(LoaderKind::Quilt), ["quilt", "fabric"]);
        assert!(modrinth_loaders(LoaderKind::Vanilla).is_empty());
        assert_eq!(folder_for(ProjectType::Shader), Some(Folder::ShaderPacks));
        assert_eq!(folder_for(ProjectType::Modpack), None);
    }

    #[test]
    fn prefers_releases() {
        let v: Vec<Version> = serde_json::from_str(
            r#"[{"id":"a","project_id":"p","version_number":"2","version_type":"beta"},
                {"id":"b","project_id":"p","version_number":"1","version_type":"release"}]"#,
        )
        .unwrap();
        assert_eq!(pick(v).unwrap().id, "b");
    }
}
