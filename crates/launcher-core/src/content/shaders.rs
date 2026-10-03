//! One-click shader support: Iris + Sodium (Fabric, Quilt, NeoForge) or
//! Oculus + Embeddium (Forge) from Modrinth, with their required
//! dependencies, into the instance's `mods/` folder.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::install::{self, InstallRequest, InstallResult};
use super::modrinth::ProjectType;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::instance::{Instance, LoaderKind};

/// Projects in install order (dependents first so their pinned dependency
/// versions win).
fn pack(kind: LoaderKind) -> Option<&'static [&'static str]> {
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt | LoaderKind::NeoForge => Some(&["iris", "sodium"]),
        LoaderKind::Forge => Some(&["oculus", "embeddium"]),
        _ => None,
    }
}

pub fn supported(kind: LoaderKind) -> bool {
    pack(kind).is_some()
}

pub async fn install(
    ctx: &Ctx,
    inst: &Instance,
    game_dir: &Path,
    cancel: &CancellationToken,
) -> Result<InstallResult> {
    let Some(projects) = pack(inst.loader.kind) else {
        return Err(CoreError::AddonUnavailable {
            project: "Iris".into(),
            loader: crate::loader::display_name(inst.loader.kind).into(),
            mc: inst.mc_version.clone(),
        });
    };
    let roots: Vec<InstallRequest> = projects
        .iter()
        .map(|p| InstallRequest {
            project: (*p).to_owned(),
            project_type: ProjectType::Mod,
            version_id: None,
        })
        .collect();
    install::install(
        ctx,
        inst,
        game_dir,
        &roots,
        &install::quiet_progress(),
        cancel,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_per_loader() {
        assert_eq!(pack(LoaderKind::Forge).unwrap(), ["oculus", "embeddium"]);
        assert!(supported(LoaderKind::NeoForge));
        assert!(!supported(LoaderKind::Vanilla));
        assert!(!supported(LoaderKind::Optifine));
    }
}
