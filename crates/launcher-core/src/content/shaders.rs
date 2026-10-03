//! One-click shader support: Iris + Sodium (Fabric, Quilt, NeoForge) or
//! Oculus + Embeddium (Forge) from Modrinth, with their required
//! dependencies, into the instance's `mods/` folder.

use std::collections::{HashSet, VecDeque};
use std::path::Path;

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::modrinth::{self, Version};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{NullSink, Progress, Stage};
use crate::hash::Checksum;
use crate::instance::files::checked_name;
use crate::instance::{Instance, LoaderKind};
use crate::net::download::{DownloadItem, Verify};

/// Safety net against dependency cycles / runaway trees.
const MAX_PROJECTS: usize = 8;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShaderSetup {
    /// Files downloaded into `mods/`.
    pub installed: Vec<String>,
    /// Projects skipped because a matching jar is already present.
    pub already_present: Vec<String>,
}

/// `(modrinth loaders, projects in install order)`.
fn pack(kind: LoaderKind) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match kind {
        LoaderKind::Fabric => Some((&["fabric"], &["iris", "sodium"])),
        LoaderKind::Quilt => Some((&["quilt", "fabric"], &["iris", "sodium"])),
        LoaderKind::NeoForge => Some((&["neoforge"], &["iris", "sodium"])),
        LoaderKind::Forge => Some((&["forge"], &["oculus", "embeddium"])),
        _ => None,
    }
}

pub fn supported(kind: LoaderKind) -> bool {
    pack(kind).is_some()
}

/// `true` if `mods/` already holds a jar of this project (enabled or not).
fn present(mods: &Path, slug: &str) -> bool {
    let Ok(rd) = std::fs::read_dir(mods) else {
        return false;
    };
    let slug = slug.to_ascii_lowercase();
    rd.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        (name.ends_with(".jar") || name.ends_with(".jar.disabled"))
            && name
                .strip_prefix(&slug)
                .is_some_and(|rest| rest.starts_with(['-', '_', '+']))
    })
}

fn pick(versions: Vec<Version>) -> Option<Version> {
    let first = versions.first().cloned();
    versions
        .into_iter()
        .find(|v| v.version_type == "release")
        .or(first)
}

pub async fn install(
    ctx: &Ctx,
    inst: &Instance,
    mods: &Path,
    cancel: &CancellationToken,
) -> Result<ShaderSetup> {
    let loader = crate::loader::display_name(inst.loader.kind);
    let mc = &inst.mc_version;
    let Some((loaders, projects)) = pack(inst.loader.kind) else {
        return Err(CoreError::AddonUnavailable {
            project: "Iris".into(),
            loader: loader.into(),
            mc: mc.clone(),
        });
    };

    // (project slug or id, pinned version)
    let mut queue: VecDeque<(String, Option<String>)> =
        projects.iter().map(|p| ((*p).to_owned(), None)).collect();
    let mut done: HashSet<String> = HashSet::new();
    let mut result = ShaderSetup::default();
    let mut items = Vec::new();

    while let Some((project, pinned)) = queue.pop_front() {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if done.len() >= MAX_PROJECTS {
            break;
        }
        let version = match &pinned {
            Some(id) => modrinth::version(ctx, id).await?,
            None => pick(modrinth::project_versions(ctx, &project, loaders, mc).await?)
                .ok_or_else(|| CoreError::AddonUnavailable {
                    project: project.clone(),
                    loader: loader.into(),
                    mc: mc.clone(),
                })?,
        };
        if !done.insert(version.project_id.clone()) {
            continue;
        }
        let info = modrinth::project(ctx, &version.project_id).await?;
        if present(mods, &info.slug) {
            result.already_present.push(info.title);
            continue;
        }
        // Required dependencies go first so a pinned version wins over a
        // later "latest" lookup of the same project.
        for dep in version
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
            .rev()
        {
            match (&dep.version_id, &dep.project_id) {
                (Some(v), Some(p)) if !done.contains(p) => {
                    queue.push_front((p.clone(), Some(v.clone())))
                }
                (Some(v), None) => queue.push_front((v.clone(), Some(v.clone()))),
                (None, Some(p)) if !done.contains(p) => queue.push_front((p.clone(), None)),
                _ => {}
            }
        }
        let file = version
            .primary_file()
            .ok_or_else(|| CoreError::AddonUnavailable {
                project: info.title.clone(),
                loader: loader.into(),
                mc: mc.clone(),
            })?;
        let name = checked_name(&file.filename)?;
        if !name.ends_with(".jar") {
            return Err(CoreError::InvalidInstance(format!("file name {name:?}")));
        }
        let checksum = match (&file.hashes.sha512, &file.hashes.sha1) {
            (Some(h), _) => Some(Checksum::Sha512(h.clone())),
            (None, Some(h)) => Some(Checksum::Sha1(h.clone())),
            _ => None,
        };
        items.push(DownloadItem {
            url: file.url.clone(),
            dest: mods.join(name),
            checksum,
            size: file.size,
        });
        tracing::info!(project = %info.slug, version = %version.version_number, "shader setup: selected");
        result.installed.push(name.to_owned());
    }

    let progress = Progress::new(std::sync::Arc::new(NullSink), "shaders", Stage::Libraries);
    ctx.downloader()
        .run(items, Verify::Full, &progress, cancel)
        .await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_jars() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sodium-fabric-0.5.13+mc1.20.1.jar"), b"").unwrap();
        std::fs::write(dir.path().join("Iris-1.7.6.jar.disabled"), b"").unwrap();
        std::fs::write(dir.path().join("irisfoo.jar"), b"").unwrap();
        assert!(present(dir.path(), "sodium"));
        assert!(present(dir.path(), "iris"));
        assert!(!present(dir.path(), "embeddium"));
        assert!(!present(dir.path(), "irisf"));
    }

    #[test]
    fn packs_per_loader() {
        assert_eq!(pack(LoaderKind::Forge).unwrap().1, ["oculus", "embeddium"]);
        assert!(supported(LoaderKind::NeoForge));
        assert!(!supported(LoaderKind::Vanilla));
        assert!(!supported(LoaderKind::Optifine));
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
