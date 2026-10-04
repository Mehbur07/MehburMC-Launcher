//! Makes offline skins visible in game through CustomSkinLoader (CSL).
//!
//! Before each launch the account's textures are written under
//! `<game>/CustomSkinLoader/MehburMC/` — a folder only the launcher touches.
//! Two `ExtraList` entries (one per arm model, since a CSL "Legacy" source has
//! a fixed model) point there; CSL prepends ExtraList sources to its load list,
//! so they win over the Mojang API source for names that also exist as
//! registered Mojang accounts. CSL deletes each ExtraList file once it has merged it
//! into `CustomSkinLoader.json`.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::{AccountTextures, SkinModel};
use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

/// Modrinth project id of CustomSkinLoader (GPL-3.0, downloaded on demand).
pub const MODRINTH_PROJECT: &str = "idMHQ4n2";

const CSL_DIR: &str = "CustomSkinLoader";
const OWN_DIR: &str = "MehburMC";

/// `(ExtraList file stem, sub-folder, CSL model name)`.
const SOURCES: &[(&str, &str, &str)] = &[
    ("MehburMC-Slim", "slim", "slim"),
    ("MehburMC-Classic", "classic", "default"),
];

fn sub(model: SkinModel) -> &'static str {
    match model {
        SkinModel::Slim => "slim",
        SkinModel::Classic => "classic",
    }
}

/// Whether the instance has an enabled CustomSkinLoader jar in `mods/`.
pub fn is_installed(game_dir: &Path) -> bool {
    std::fs::read_dir(game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            n.starts_with("customskinloader") && n.ends_with(".jar")
        })
}

/// Player names come from validated offline accounts; re-check anyway since
/// the name becomes a file name.
fn file_name(player: &str) -> Result<String> {
    if crate::auth::offline::validate_name(player).is_err() {
        return Err(CoreError::InvalidPlayerName(player.to_owned()));
    }
    Ok(format!("{player}.png"))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::io(path, e)),
    }
}

/// Writes `bytes` unless the file already has exactly that content (keeps
/// the mtime stable so CSL's texture cache stays valid).
fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    write_atomic(path, bytes)
}

fn ensure_extra_list(csl: &Path) -> Result<()> {
    let config = std::fs::read_to_string(csl.join("CustomSkinLoader.json")).unwrap_or_default();
    for (stem, folder, model) in SOURCES {
        let skin = format!("{OWN_DIR}/{folder}/{{USERNAME}}.png");
        // Already merged by CSL on an earlier start.
        if config.contains(&skin) {
            continue;
        }
        let entry = json!({
            "name": stem,
            "type": "Legacy",
            "checkPNG": false,
            "skin": skin,
            "model": model,
            "cape": format!("{OWN_DIR}/capes/{{USERNAME}}.png"),
            "elytra": format!("{OWN_DIR}/elytras/{{USERNAME}}.png"),
        });
        let file = csl.join("ExtraList").join(format!("{stem}.json"));
        write_if_changed(
            &file,
            serde_json::to_string_pretty(&entry)
                .unwrap_or_default()
                .as_bytes(),
        )?;
    }
    Ok(())
}

/// Places (or removes) `player`'s textures for CSL. Returns the files written.
pub fn sync(game_dir: &Path, player: &str, t: &AccountTextures) -> Result<Vec<PathBuf>> {
    let name = file_name(player)?;
    let csl = game_dir.join(CSL_DIR);
    let own = csl.join(OWN_DIR);
    ensure_extra_list(&csl)?;

    let mut written = Vec::new();
    for (_, folder, _) in SOURCES {
        let path = own.join(folder).join(&name);
        match &t.skin {
            Some((bytes, model)) if sub(*model) == *folder => {
                write_if_changed(&path, bytes)?;
                written.push(path);
            }
            _ => remove_if_exists(&path)?,
        }
    }
    let cape = own.join("capes").join(&name);
    match &t.cape {
        Some(bytes) => {
            write_if_changed(&cape, bytes)?;
            written.push(cape);
        }
        None => remove_if_exists(&cape)?,
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jar() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_installed(tmp.path()));
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("CustomSkinLoader_Fabric-14.28.jar.disabled"), b"").unwrap();
        assert!(!is_installed(tmp.path()));
        std::fs::write(mods.join("CustomSkinLoader_Universal-15.0.1.jar"), b"").unwrap();
        assert!(is_installed(tmp.path()));
    }

    #[test]
    fn writes_moves_and_removes_textures() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        let own = game.join("CustomSkinLoader/MehburMC");
        let t = AccountTextures {
            skin: Some((b"skin".to_vec(), SkinModel::Slim)),
            cape: Some(b"cape".to_vec()),
        };
        let w = sync(game, "Steve", &t).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(std::fs::read(own.join("slim/Steve.png")).unwrap(), b"skin");
        assert_eq!(std::fs::read(own.join("capes/Steve.png")).unwrap(), b"cape");

        let extra = game.join("CustomSkinLoader/ExtraList/MehburMC-Slim.json");
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&extra).unwrap()).unwrap();
        assert_eq!(v["type"], "Legacy");
        assert_eq!(v["model"], "slim");
        assert_eq!(v["skin"], "MehburMC/slim/{USERNAME}.png");

        // Model switch moves the file to the other source.
        let t = AccountTextures {
            skin: Some((b"skin".to_vec(), SkinModel::Classic)),
            cape: None,
        };
        sync(game, "Steve", &t).unwrap();
        assert!(!own.join("slim/Steve.png").exists());
        assert!(own.join("classic/Steve.png").exists());
        assert!(!own.join("capes/Steve.png").exists());

        // Once CSL merged the sources into its config, no ExtraList is written.
        std::fs::remove_dir_all(game.join("CustomSkinLoader/ExtraList")).unwrap();
        std::fs::write(
            game.join("CustomSkinLoader/CustomSkinLoader.json"),
            r#"{"loadlist":[{"skin":"MehburMC/slim/{USERNAME}.png"},{"skin":"MehburMC/classic/{USERNAME}.png"}]}"#,
        )
        .unwrap();
        sync(
            game,
            "Steve",
            &AccountTextures {
                skin: None,
                cape: None,
            },
        )
        .unwrap();
        assert!(!game.join("CustomSkinLoader/ExtraList").exists());
        assert!(!own.join("classic/Steve.png").exists());

        assert!(sync(game, "../evil", &t).is_err());
    }
}
