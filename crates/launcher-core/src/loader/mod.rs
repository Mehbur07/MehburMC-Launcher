//! Mod loaders: version listing and installation.
//!
//! Every loader ends up as a regular version JSON in `versions/<id>/<id>.json`
//! that inherits from the vanilla version, so the launch pipeline treats it
//! like any other version. Ids are derived from the loader spec alone, which
//! lets us detect an installed loader without network access.

pub mod early_window;
pub mod fabric;
pub mod forge;
pub mod optifine;
mod processors;

use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{Progress, Stage};
use crate::fsutil::write_json_atomic;
use crate::instance::{LoaderKind, LoaderSpec};
use crate::version::VersionJson;

/// Loader version lists change a few times a day at most.
pub(crate) const LIST_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoaderVersion {
    /// Value stored in `LoaderSpec.version`.
    pub id: String,
    /// Short label for the UI (`47.4.26` for Forge `1.20.1-47.4.26`).
    pub label: String,
    pub stable: bool,
    /// The version picked when the user does not choose one.
    pub recommended: bool,
}

/// Result of [`ensure_installed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Version id to launch.
    pub version_id: String,
    /// Loader version actually used (resolved if the spec had none).
    pub loader_version: Option<String>,
}

pub fn display_name(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Vanilla => "Vanilla",
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::LegacyFabric => "Legacy Fabric",
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
        LoaderKind::Optifine => "OptiFine",
    }
}

/// Available loader versions for `mc`, newest first. Empty if the loader
/// does not support that Minecraft version.
pub async fn list_versions(ctx: &Ctx, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    let mut list = match kind {
        LoaderKind::Vanilla => return Ok(vec![]),
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric => {
            fabric::list(ctx, kind, mc).await?
        }
        LoaderKind::Forge => forge::list(ctx, forge::Flavor::Forge, mc).await?,
        LoaderKind::NeoForge => forge::list(ctx, forge::Flavor::NeoForge, mc).await?,
        LoaderKind::Optifine => optifine::list(&ctx.paths, mc),
    };
    mark_recommended(&mut list);
    Ok(list)
}

/// Keeps an explicit recommendation, otherwise recommends the newest stable
/// (or simply the newest) version.
fn mark_recommended(list: &mut [LoaderVersion]) {
    if list.iter().any(|v| v.recommended) {
        return;
    }
    let pick = list
        .iter()
        .position(|v| v.stable)
        .or(if list.is_empty() { None } else { Some(0) });
    if let Some(i) = pick {
        list[i].recommended = true;
    }
}

/// Deterministic version id for an installed loader.
pub fn version_id(kind: LoaderKind, mc: &str, loader_version: &str) -> String {
    match kind {
        LoaderKind::Vanilla => mc.to_owned(),
        LoaderKind::Fabric => format!("fabric-loader-{loader_version}-{mc}"),
        LoaderKind::Quilt => format!("quilt-loader-{loader_version}-{mc}"),
        LoaderKind::LegacyFabric => format!("legacyfabric-loader-{loader_version}-{mc}"),
        LoaderKind::Forge => format!("forge-{loader_version}"),
        LoaderKind::NeoForge => format!("neoforge-{loader_version}"),
        LoaderKind::Optifine => format!("optifine-{mc}_{loader_version}"),
    }
}

/// Serialises loader installs: Forge installers write shared libraries and
/// must not run twice for the same files at once.
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Installs the loader for `mc` if needed (or always with `force`) and
/// returns the version id to launch.
pub async fn ensure_installed(
    ctx: &Ctx,
    mc: &str,
    spec: &LoaderSpec,
    force: bool,
    task: &str,
    cancel: &CancellationToken,
) -> Result<Installed> {
    if spec.kind == LoaderKind::Vanilla {
        return Ok(Installed {
            version_id: mc.to_owned(),
            loader_version: None,
        });
    }
    let name = display_name(spec.kind);
    let loader_version = match spec.version.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(v) => v.to_owned(),
        None => list_versions(ctx, spec.kind, mc)
            .await?
            .into_iter()
            .find(|v| v.recommended)
            .map(|v| v.id)
            .ok_or_else(|| CoreError::LoaderUnavailable {
                loader: name.into(),
                mc: mc.to_owned(),
            })?,
    };
    validate_loader_version(&loader_version)?;
    let id = version_id(spec.kind, mc, &loader_version);
    let installed = Installed {
        version_id: id.clone(),
        loader_version: Some(loader_version.clone()),
    };

    let _lock = tokio::select! {
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        l = INSTALL_LOCK.lock() => l,
    };
    if !force && version_file(ctx, &id).is_file() {
        return Ok(installed);
    }

    tracing::info!(loader = name, %mc, version = %loader_version, %id, "installing loader");
    let progress = Progress::new(ctx.events.clone(), task, Stage::Loader);
    let started = std::time::Instant::now();
    let json = match spec.kind {
        LoaderKind::Vanilla => unreachable!("handled above"),
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::LegacyFabric => {
            fabric::install(ctx, spec.kind, mc, &loader_version, &progress).await?
        }
        LoaderKind::Forge => {
            forge::install(
                ctx,
                forge::Flavor::Forge,
                mc,
                &loader_version,
                &progress,
                cancel,
            )
            .await?
        }
        LoaderKind::NeoForge => {
            forge::install(
                ctx,
                forge::Flavor::NeoForge,
                mc,
                &loader_version,
                &progress,
                cancel,
            )
            .await?
        }
        LoaderKind::Optifine => {
            optifine::install(ctx, mc, &loader_version, &progress, cancel).await?
        }
    };
    write_version(ctx, &id, mc, json)?;
    progress.finish();
    tracing::info!(%id, secs = started.elapsed().as_secs_f32(), "loader installed");
    Ok(installed)
}

fn version_file(ctx: &Ctx, id: &str) -> PathBuf {
    ctx.paths.versions().join(id).join(format!("{id}.json"))
}

/// Loader versions end up in file and URL paths; reject anything odd.
pub(crate) fn validate_loader_version(v: &str) -> Result<()> {
    let ok = !v.is_empty()
        && v.len() <= 80
        && !v.contains("..")
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'));
    if ok {
        Ok(())
    } else {
        Err(CoreError::InvalidInstance(format!("loader version {v:?}")))
    }
}

/// Writes the final profile last, so a half-finished install is never
/// mistaken for a complete one.
fn write_version(ctx: &Ctx, id: &str, mc: &str, mut json: serde_json::Value) -> Result<()> {
    json["id"] = id.into();
    json["inheritsFrom"] = mc.into();
    let path = version_file(ctx, id);
    // Validate the shape before committing it.
    serde_json::from_value::<VersionJson>(json.clone()).map_err(|source| CoreError::Json {
        path: path.clone(),
        source,
    })?;
    write_json_atomic(&path, &json)
}

/// Orders loader versions like `0.30.1-beta.4 < 0.30.1 < 0.31.0-beta.1`.
pub fn compare_loader_versions(a: &str, b: &str) -> Ordering {
    fn split(v: &str) -> (Vec<u64>, Option<&str>) {
        let (core, pre) = match v.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (v, None),
        };
        let core = core.split('+').next().unwrap_or(core);
        (
            core.split('.').map(|p| p.parse().unwrap_or(0)).collect(),
            pre,
        )
    }
    fn pre_key(p: &str) -> (String, u64) {
        let (name, num) = p.split_once('.').unwrap_or((p, "0"));
        (name.to_owned(), num.parse().unwrap_or(0))
    }
    let (ca, pa) = split(a);
    let (cb, pb) = split(b);
    let len = ca.len().max(cb.len());
    for i in 0..len {
        match ca.get(i).unwrap_or(&0).cmp(cb.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    match (pa, pb) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => pre_key(x).cmp(&pre_key(y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_deterministic() {
        assert_eq!(
            version_id(LoaderKind::Fabric, "26.3", "0.19.5"),
            "fabric-loader-0.19.5-26.3"
        );
        assert_eq!(
            version_id(LoaderKind::Forge, "1.20.1", "1.20.1-47.4.26"),
            "forge-1.20.1-47.4.26"
        );
        assert_eq!(
            version_id(LoaderKind::NeoForge, "26.3", "26.3.0.46-beta"),
            "neoforge-26.3.0.46-beta"
        );
        assert_eq!(
            version_id(LoaderKind::Optifine, "1.20.1", "HD_U_I6"),
            "optifine-1.20.1_HD_U_I6"
        );
        assert_eq!(version_id(LoaderKind::Vanilla, "26.3", "x"), "26.3");
    }

    #[test]
    fn loader_version_validation() {
        for ok in [
            "0.19.5",
            "1.20.1-47.4.26",
            "26.3.0.46-beta",
            "HD_U_I6",
            "0.17.0+build.1",
        ] {
            assert!(validate_loader_version(ok).is_ok(), "{ok}");
        }
        for bad in ["", "../x", "a/b", "a\\b", "a b", "x:y"] {
            assert!(validate_loader_version(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn loader_version_order() {
        let mut v = vec![
            "0.30.1",
            "0.31.0-beta.1",
            "0.30.1-beta.4",
            "0.29.2-beta.4",
            "0.31.0-beta.10",
            "0.31.0-beta.2",
            "0.4.0",
        ];
        v.sort_by(|a, b| compare_loader_versions(a, b));
        assert_eq!(
            v,
            [
                "0.4.0",
                "0.29.2-beta.4",
                "0.30.1-beta.4",
                "0.30.1",
                "0.31.0-beta.1",
                "0.31.0-beta.2",
                "0.31.0-beta.10"
            ]
        );
    }

    #[test]
    fn recommendation_fallbacks() {
        let v = |id: &str, stable: bool| LoaderVersion {
            id: id.into(),
            label: id.into(),
            stable,
            recommended: false,
        };
        let mut l = vec![v("2-beta", false), v("1", true)];
        mark_recommended(&mut l);
        assert!(l[1].recommended && !l[0].recommended);
        let mut l = vec![v("2-beta", false)];
        mark_recommended(&mut l);
        assert!(l[0].recommended);
        let mut l: Vec<LoaderVersion> = vec![];
        mark_recommended(&mut l);
    }
}
