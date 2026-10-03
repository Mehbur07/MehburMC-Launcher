//! Asset index + objects (`assets/indexes`, `assets/objects/xx/<hash>`), and
//! the legacy layouts: `virtual` (1.6–1.7, `assets/virtual/<id>/<path>`) and
//! `map_to_resources` (pre-1.6, `<gameDir>/resources/<path>`).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::hash::Checksum;
use crate::net::download::DownloadItem;
use crate::paths::Paths;
use crate::version::profile::AssetIndexRef;

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: BTreeMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Where the game should look for assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLayout {
    /// `${assets_root}`
    pub assets_root: PathBuf,
    /// `${game_assets}` (legacy argument)
    pub game_assets: PathBuf,
}

pub fn object_path(paths: &Paths, hash: &str) -> PathBuf {
    paths.assets().join("objects").join(&hash[..2]).join(hash)
}

pub fn index_path(paths: &Paths, id: &str) -> PathBuf {
    paths.assets().join("indexes").join(format!("{id}.json"))
}

/// The index file itself as a download item.
pub fn index_item(paths: &Paths, r: &AssetIndexRef) -> DownloadItem {
    DownloadItem {
        url: r.url.clone(),
        dest: index_path(paths, &r.id),
        checksum: r.sha1.clone().map(Checksum::Sha1),
        size: r.size,
    }
}

pub fn read_index(paths: &Paths, id: &str) -> Result<AssetIndex> {
    let p = index_path(paths, id);
    let bytes = std::fs::read(&p).map_err(|e| CoreError::io(&p, e))?;
    serde_json::from_slice(&bytes).map_err(|source| CoreError::Json { path: p, source })
}

/// One download per unique object hash.
pub fn object_items(ctx: &Ctx, index: &AssetIndex) -> Vec<DownloadItem> {
    let mut seen = HashSet::new();
    index
        .objects
        .values()
        .filter(|o| o.hash.len() >= 2 && seen.insert(o.hash.clone()))
        .map(|o| DownloadItem {
            url: format!("{}/{}/{}", ctx.endpoints.resources, &o.hash[..2], o.hash),
            dest: object_path(&ctx.paths, &o.hash),
            checksum: Some(Checksum::Sha1(o.hash.clone())),
            size: Some(o.size),
        })
        .collect()
}

pub fn layout(paths: &Paths, id: &str, index: &AssetIndex, game_dir: &Path) -> AssetLayout {
    let assets_root = paths.assets();
    let game_assets = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        assets_root.join("virtual").join(id)
    } else {
        assets_root.clone()
    };
    AssetLayout {
        assets_root,
        game_assets,
    }
}

/// Copies objects to their named paths for legacy layouts (no-op otherwise).
pub fn materialize(paths: &Paths, index: &AssetIndex, layout: &AssetLayout) -> Result<usize> {
    if !index.map_to_resources && !index.is_virtual {
        return Ok(0);
    }
    let mut copied = 0;
    for (name, obj) in &index.objects {
        let Some(target) = crate::archive::safe_join(&layout.game_assets, name) else {
            tracing::warn!(%name, "skipping unsafe asset name");
            continue;
        };
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == obj.size) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let src = object_path(paths, &obj.hash);
        std::fs::copy(&src, &target).map_err(|e| CoreError::io(&target, e))?;
        copied += 1;
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(json: &str) -> AssetIndex {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn layouts() {
        let paths = Paths::at("/d/MehburMC");
        let game = Path::new("/g");
        let modern = index(r#"{"objects":{}}"#);
        assert_eq!(
            layout(&paths, "34", &modern, game).game_assets,
            paths.assets()
        );
        let virt = index(r#"{"virtual":true,"objects":{}}"#);
        assert_eq!(
            layout(&paths, "legacy", &virt, game).game_assets,
            paths.assets().join("virtual").join("legacy")
        );
        let res = index(r#"{"map_to_resources":true,"objects":{}}"#);
        assert_eq!(
            layout(&paths, "pre-1.6", &res, game).game_assets,
            game.join("resources")
        );
    }

    #[test]
    fn materializes_map_to_resources() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path().join("MehburMC"));
        let hash = "0d000710b71ca9aafabd8f587768431d0b560b32";
        let obj = object_path(&paths, hash);
        std::fs::create_dir_all(obj.parent().unwrap()).unwrap();
        std::fs::write(&obj, b"abc").unwrap();
        let idx = index(&format!(
            r#"{{"map_to_resources":true,"objects":{{"sound/a.ogg":{{"hash":"{hash}","size":3}}}}}}"#
        ));
        let game = dir.path().join("game");
        let l = layout(&paths, "pre-1.6", &idx, &game);
        assert_eq!(materialize(&paths, &idx, &l).unwrap(), 1);
        assert_eq!(
            std::fs::read(game.join("resources/sound/a.ogg")).unwrap(),
            b"abc"
        );
        assert_eq!(
            materialize(&paths, &idx, &l).unwrap(),
            0,
            "second run is a no-op"
        );
    }
}
