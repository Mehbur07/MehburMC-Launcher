//! Skin and cape library for offline accounts.
//!
//! Textures are content-addressed (`skins/textures/<sha1>.png`), metadata and
//! per-account assignments live in `skins/library.json`. Offline skins are
//! invisible to the vanilla client; they show in the launcher preview and, in
//! instances with CustomSkinLoader, in game (see [`csl`], ARCHITECTURE.md R12).

pub mod csl;
pub mod defaults;
pub mod image;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CoreError, Result};
use crate::fsutil::{write_atomic, write_json_atomic};
use crate::instance::now_secs;
use crate::paths::Paths;

const MAX_NAME_CHARS: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SkinModel {
    /// Steve, 4px arms.
    Classic,
    /// Alex, 3px arms.
    Slim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinEntry {
    /// SHA-1 of the PNG.
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    #[ts(type = "number")]
    pub added_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapeEntry {
    /// SHA-1 of the PNG.
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub added_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Assignment {
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cape: Option<String>,
}

impl Assignment {
    fn is_empty(&self) -> bool {
        self.skin.is_none() && self.cape.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Library {
    skins: Vec<SkinEntry>,
    capes: Vec<CapeEntry>,
    /// Account id → chosen textures.
    assignments: BTreeMap<String, Assignment>,
    /// Keys of the retired built-in textures (K59/K60); kept so old files
    /// round-trip, no longer used.
    builtin: Vec<String>,
    /// Ids added from private grants (K74); they leave when the grant does.
    private: Vec<String>,
}

/// SHA-1s of the retired public MehburMC skin and cape (K74). They are
/// removed from every library and never offered again; the new design is
/// only handed out privately.
pub const RETIRED: &[&str] = &[
    "15f36107b3e370aa9b39b7327f421b0fc371f0b0",
    "550a2572b75449fd44f33723a4bea217ba767805",
];

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkinItem {
    #[serde(flatten)]
    pub entry: SkinEntry,
    /// `data:image/png;base64,…` (the webview may not read files).
    pub data_uri: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapeItem {
    #[serde(flatten)]
    pub entry: CapeEntry,
    pub data_uri: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryView {
    pub skins: Vec<SkinItem>,
    pub capes: Vec<CapeItem>,
    pub assignments: BTreeMap<String, Assignment>,
    /// Ids granted privately by the MehburMC team: not shareable.
    pub private: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TextureKind {
    Skin,
    Cape,
}

/// Textures handed to the game for one account.
pub struct AccountTextures {
    pub skin: Option<(Vec<u8>, SkinModel)>,
    pub cape: Option<Vec<u8>>,
}

pub struct SkinStore {
    paths: Paths,
    lock: Mutex<()>,
}

fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(bytes))
}

fn data_uri(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Ids are SHA-1 hex; anything else could escape the textures folder.
fn check_id(id: &str) -> Result<()> {
    if id.len() == 40 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(CoreError::SkinNotFound(id.to_owned()))
    }
}

fn clean_name(name: &str, fallback: &str) -> String {
    let n: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if n.is_empty() { fallback.to_owned() } else { n }
}

impl SkinStore {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            lock: Mutex::new(()),
        }
    }

    fn library_file(&self) -> PathBuf {
        self.paths.skins().join("library.json")
    }

    pub fn texture_path(&self, id: &str) -> Result<PathBuf> {
        check_id(id)?;
        Ok(self
            .paths
            .skins()
            .join("textures")
            .join(format!("{id}.png")))
    }

    fn read(&self) -> Library {
        std::fs::read(self.library_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write(&self, lib: &Library) -> Result<()> {
        write_json_atomic(&self.library_file(), lib)
    }

    fn texture(&self, id: &str) -> Option<Vec<u8>> {
        std::fs::read(self.texture_path(id).ok()?).ok()
    }

    /// Entries whose texture file vanished are left out.
    pub fn view(&self) -> LibraryView {
        let lib = self.read();
        let skins = lib
            .skins
            .into_iter()
            .filter_map(|entry| {
                let bytes = self.texture(&entry.id)?;
                Some(SkinItem {
                    entry,
                    data_uri: data_uri(&bytes),
                })
            })
            .collect();
        let capes = lib
            .capes
            .into_iter()
            .filter_map(|entry| {
                let bytes = self.texture(&entry.id)?;
                Some(CapeItem {
                    entry,
                    data_uri: data_uri(&bytes),
                })
            })
            .collect();
        LibraryView {
            skins,
            capes,
            assignments: lib.assignments,
            private: lib.private,
        }
    }

    fn store_texture(&self, bytes: &[u8]) -> Result<String> {
        let id = sha1_hex(bytes);
        let path = self.texture_path(&id)?;
        if !path.is_file() {
            write_atomic(&path, bytes)?;
        }
        Ok(id)
    }

    /// Removes the retired MehburMC textures (K74): library entries,
    /// assignments (the game falls back to the default skin) and files.
    /// Returns whether anything was removed.
    pub fn retire_builtins(&self) -> Result<bool> {
        let ids: Vec<String> = RETIRED.iter().map(|s| (*s).to_owned()).collect();
        Ok(!self.remove_ids(&ids)?.is_empty())
    }

    /// Ids of privately granted textures in this library.
    pub fn private_ids(&self) -> Vec<String> {
        self.read().private
    }

    /// Adds a privately granted texture and remembers it as private.
    pub fn add_private(
        &self,
        kind: TextureKind,
        bytes: &[u8],
        name: &str,
        model: SkinModel,
    ) -> Result<String> {
        let id = match kind {
            TextureKind::Skin => self.add_skin(bytes, name, Some(model))?.id,
            TextureKind::Cape => self.add_cape(bytes, name)?.id,
        };
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        if !lib.private.contains(&id) {
            lib.private.push(id.clone());
            self.write(&lib)?;
        }
        Ok(id)
    }

    /// Removes the given ids everywhere (entries, assignments, private
    /// list, files). Returns the ids that were present.
    pub fn remove_ids(&self, ids: &[String]) -> Result<Vec<String>> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let present: Vec<String> = ids
            .iter()
            .filter(|id| {
                lib.skins.iter().any(|e| &e.id == *id)
                    || lib.capes.iter().any(|e| &e.id == *id)
                    || lib.private.contains(id)
            })
            .cloned()
            .collect();
        if present.is_empty() {
            return Ok(present);
        }
        lib.skins.retain(|e| !present.contains(&e.id));
        lib.capes.retain(|e| !present.contains(&e.id));
        lib.private.retain(|id| !present.contains(id));
        for a in lib.assignments.values_mut() {
            for slot in [&mut a.skin, &mut a.cape] {
                if slot.as_ref().is_some_and(|id| present.contains(id)) {
                    *slot = None;
                }
            }
        }
        lib.assignments.retain(|_, a| !a.is_empty());
        self.write(&lib)?;
        for id in &present {
            let path = self.texture_path(id)?;
            if let Err(e) = std::fs::remove_file(&path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(CoreError::io(path, e));
            }
        }
        Ok(present)
    }

    /// Validates and adds a skin; the same image twice returns the existing
    /// entry. `model: None` detects slim arms from the pixels.
    pub fn add_skin(
        &self,
        bytes: &[u8],
        name: &str,
        model: Option<SkinModel>,
    ) -> Result<SkinEntry> {
        let img = image::decode(bytes)?;
        image::check_skin(&img)?;
        let model = model.unwrap_or_else(|| image::detect_model(&img));
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let id = self.store_texture(bytes)?;
        if let Some(e) = lib.skins.iter().find(|e| e.id == id) {
            return Ok(e.clone());
        }
        let entry = SkinEntry {
            id,
            name: clean_name(name, "Skin"),
            model,
            added_at: now_secs(),
        };
        lib.skins.push(entry.clone());
        self.write(&lib)?;
        Ok(entry)
    }

    pub fn add_cape(&self, bytes: &[u8], name: &str) -> Result<CapeEntry> {
        let img = image::decode(bytes)?;
        image::check_cape(&img)?;
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let id = self.store_texture(bytes)?;
        if let Some(e) = lib.capes.iter().find(|e| e.id == id) {
            return Ok(e.clone());
        }
        let entry = CapeEntry {
            id,
            name: clean_name(name, "Cape"),
            added_at: now_secs(),
        };
        lib.capes.push(entry.clone());
        self.write(&lib)?;
        Ok(entry)
    }

    /// Reads a PNG the user picked in a file dialog.
    pub fn read_import_file(path: &Path) -> Result<Vec<u8>> {
        let is_png = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if !path.is_absolute() || !is_png {
            return Err(CoreError::SkinInvalid("not a PNG file".into()));
        }
        let meta = std::fs::metadata(path).map_err(|e| CoreError::io(path, e))?;
        if meta.len() > image::MAX_FILE_BYTES as u64 {
            return Err(CoreError::SkinInvalid("file is too large".into()));
        }
        std::fs::read(path).map_err(|e| CoreError::io(path, e))
    }

    pub fn import_file(
        &self,
        kind: TextureKind,
        path: &Path,
        model: Option<SkinModel>,
    ) -> Result<String> {
        let bytes = Self::read_import_file(path)?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        Ok(match kind {
            TextureKind::Skin => self.add_skin(&bytes, name, model)?.id,
            TextureKind::Cape => self.add_cape(&bytes, name)?.id,
        })
    }

    /// Adds a texture drawn in the editor or picked from the presets, sent
    /// as base64 PNG (no `data:` prefix). Returns the texture id.
    pub fn add_base64(
        &self,
        kind: TextureKind,
        name: &str,
        model: Option<SkinModel>,
        png_base64: &str,
    ) -> Result<String> {
        // 4 base64 chars encode 3 bytes; reject before decoding.
        if png_base64.len() > image::MAX_FILE_BYTES / 3 * 4 + 4 {
            return Err(CoreError::SkinInvalid("file is too large".into()));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(png_base64.trim())
            .map_err(|_| CoreError::SkinInvalid("not a PNG file".into()))?;
        Ok(match kind {
            TextureKind::Skin => self.add_skin(&bytes, name, model)?.id,
            TextureKind::Cape => self.add_cape(&bytes, name)?.id,
        })
    }

    pub fn update_skin(
        &self,
        id: &str,
        name: Option<&str>,
        model: Option<SkinModel>,
    ) -> Result<SkinEntry> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let e = lib
            .skins
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| CoreError::SkinNotFound(id.to_owned()))?;
        if let Some(n) = name {
            e.name = clean_name(n, &e.name);
        }
        if let Some(m) = model {
            e.model = m;
        }
        let out = e.clone();
        self.write(&lib)?;
        Ok(out)
    }

    pub fn rename_cape(&self, id: &str, name: &str) -> Result<CapeEntry> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let e = lib
            .capes
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| CoreError::SkinNotFound(id.to_owned()))?;
        e.name = clean_name(name, &e.name);
        let out = e.clone();
        self.write(&lib)?;
        Ok(out)
    }

    /// Removes the entry, clears it from every account and deletes the file
    /// unless the same image is still used as the other texture kind.
    pub fn delete(&self, kind: TextureKind, id: &str) -> Result<()> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        let before = lib.skins.len() + lib.capes.len();
        match kind {
            TextureKind::Skin => lib.skins.retain(|e| e.id != id),
            TextureKind::Cape => lib.capes.retain(|e| e.id != id),
        }
        if lib.skins.len() + lib.capes.len() == before {
            return Err(CoreError::SkinNotFound(id.to_owned()));
        }
        for a in lib.assignments.values_mut() {
            let slot = match kind {
                TextureKind::Skin => &mut a.skin,
                TextureKind::Cape => &mut a.cape,
            };
            if slot.as_deref() == Some(id) {
                *slot = None;
            }
        }
        lib.assignments.retain(|_, a| !a.is_empty());
        self.write(&lib)?;
        let still_used =
            lib.skins.iter().any(|e| e.id == id) || lib.capes.iter().any(|e| e.id == id);
        if !still_used {
            let path = self.texture_path(id)?;
            if let Err(e) = std::fs::remove_file(&path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(CoreError::io(path, e));
            }
        }
        Ok(())
    }

    /// Sets (or clears with `None`) an account's skin or cape.
    pub fn assign(
        &self,
        account_id: &str,
        kind: TextureKind,
        id: Option<&str>,
    ) -> Result<BTreeMap<String, Assignment>> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        if let Some(id) = id {
            let known = match kind {
                TextureKind::Skin => lib.skins.iter().any(|e| e.id == id),
                TextureKind::Cape => lib.capes.iter().any(|e| e.id == id),
            };
            if !known {
                return Err(CoreError::SkinNotFound(id.to_owned()));
            }
        }
        let a = lib.assignments.entry(account_id.to_owned()).or_default();
        let slot = match kind {
            TextureKind::Skin => &mut a.skin,
            TextureKind::Cape => &mut a.cape,
        };
        *slot = id.map(str::to_owned);
        lib.assignments.retain(|_, a| !a.is_empty());
        self.write(&lib)?;
        Ok(lib.assignments)
    }

    /// Drops assignments of accounts that no longer exist.
    pub fn forget_account(&self, account_id: &str) -> Result<()> {
        let _g = self.lock.lock().expect("skins lock");
        let mut lib = self.read();
        if lib.assignments.remove(account_id).is_some() {
            self.write(&lib)?;
        }
        Ok(())
    }

    pub fn textures_for(&self, account_id: &str) -> AccountTextures {
        let lib = self.read();
        let a = lib.assignments.get(account_id).cloned().unwrap_or_default();
        let skin = a.skin.and_then(|id| {
            let model = lib.skins.iter().find(|e| e.id == id)?.model;
            Some((self.texture(&id)?, model))
        });
        let cape = a
            .cape
            .filter(|id| lib.capes.iter().any(|e| e.id == *id))
            .and_then(|id| self.texture(&id));
        AccountTextures { skin, cape }
    }

    /// Copies a texture to a path picked in a save dialog.
    pub fn export(&self, id: &str, dest: &Path) -> Result<()> {
        let is_png = dest
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if !dest.is_absolute() || !is_png {
            return Err(CoreError::InvalidSetting("path".into()));
        }
        let bytes = self
            .texture(id)
            .ok_or_else(|| CoreError::SkinNotFound(id.to_owned()))?;
        write_atomic(dest, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::image::tests::png;
    use super::*;

    fn store() -> (tempfile::TempDir, SkinStore) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        (tmp, SkinStore::new(paths))
    }

    #[test]
    fn retires_the_old_mehbur_textures() {
        let (_tmp, s) = store();
        let mine = s
            .add_skin(&png(64, 64, |_, _| false), "Mine", None)
            .unwrap();
        // Simulate a library seeded by an older launcher: the retired ids.
        let old_skin = RETIRED[0].to_owned();
        let old_cape = RETIRED[1].to_owned();
        {
            let mut lib = s.read();
            lib.skins.push(SkinEntry {
                id: old_skin.clone(),
                name: "MehburMC".into(),
                model: SkinModel::Classic,
                added_at: 1,
            });
            lib.capes.push(CapeEntry {
                id: old_cape.clone(),
                name: "MehburMC".into(),
                added_at: 1,
            });
            lib.builtin = vec!["mehbur-skin".into(), "mehbur-cape".into()];
            s.write(&lib).unwrap();
        }
        std::fs::write(s.texture_path(&old_skin).unwrap(), b"x").unwrap();
        s.assign("acc", TextureKind::Skin, Some(&old_skin)).unwrap();
        s.assign("acc", TextureKind::Cape, Some(&old_cape)).unwrap();
        s.assign("other", TextureKind::Skin, Some(&mine.id))
            .unwrap();

        assert!(s.retire_builtins().unwrap());
        let v = s.view();
        assert_eq!(v.skins.len(), 1);
        assert!(v.capes.is_empty());
        // The account falls back to the default skin; others keep theirs.
        assert!(!v.assignments.contains_key("acc"));
        assert_eq!(
            v.assignments["other"].skin.as_deref(),
            Some(mine.id.as_str())
        );
        assert!(!s.texture_path(&old_skin).unwrap().exists());
        // Nothing left to do the second time.
        assert!(!s.retire_builtins().unwrap());
    }

    #[test]
    fn private_textures_come_and_go() {
        let (_tmp, s) = store();
        let skin = png(64, 64, |x, _| x == 3);
        let id = s
            .add_private(TextureKind::Skin, &skin, "MehburMC", SkinModel::Slim)
            .unwrap();
        assert_eq!(s.private_ids(), std::slice::from_ref(&id));
        let v = s.view();
        assert_eq!(v.private, std::slice::from_ref(&id));
        assert_eq!(v.skins[0].entry.model, SkinModel::Slim);
        s.assign("acc", TextureKind::Skin, Some(&id)).unwrap();
        assert_eq!(s.remove_ids(std::slice::from_ref(&id)).unwrap(), [id]);
        assert!(s.view().skins.is_empty() && s.private_ids().is_empty());
        assert!(s.view().assignments.is_empty());
    }

    #[test]
    fn adds_base64_textures() {
        let (_tmp, s) = store();
        let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
        let id = s
            .add_base64(
                TextureKind::Skin,
                "Drawn",
                Some(SkinModel::Slim),
                &b64(&png(64, 64, |_, _| false)),
            )
            .unwrap();
        let v = s.view();
        assert_eq!(v.skins[0].entry.id, id);
        assert_eq!(v.skins[0].entry.model, SkinModel::Slim);
        s.add_base64(
            TextureKind::Cape,
            "c",
            None,
            &b64(&png(64, 32, |_, _| false)),
        )
        .unwrap();
        assert_eq!(s.view().capes.len(), 1);

        for bad in ["%%%".to_owned(), b64(b"not a png"), "A".repeat(3_000_000)] {
            let e = s
                .add_base64(TextureKind::Skin, "x", None, &bad)
                .unwrap_err();
            assert_eq!(e.code(), "skin.invalid");
        }
        // A cape-sized image is not a valid skin.
        let e = s
            .add_base64(
                TextureKind::Cape,
                "x",
                None,
                &b64(&png(64, 64, |_, _| false)),
            )
            .unwrap_err();
        assert_eq!(e.code(), "skin.invalid");
    }

    #[test]
    fn library_lifecycle() {
        let (_tmp, s) = store();
        let skin = png(64, 64, |_, _| false);
        let a = s.add_skin(&skin, "  My skin ", None).unwrap();
        assert_eq!(a.name, "My skin");
        assert_eq!(a.model, SkinModel::Classic);
        assert_eq!(s.add_skin(&skin, "dup", None).unwrap().id, a.id);

        let cape = png(64, 32, |_, _| false);
        let c = s.add_cape(&cape, "").unwrap();
        assert_eq!(c.name, "Cape");
        assert_eq!(
            s.add_skin(&cape, "legacy", None).unwrap().model,
            SkinModel::Classic
        );

        let v = s.view();
        assert_eq!(v.skins.len(), 2);
        assert!(v.skins[0].data_uri.starts_with("data:image/png;base64,"));

        s.assign("acc", TextureKind::Skin, Some(&a.id)).unwrap();
        s.assign("acc", TextureKind::Cape, Some(&c.id)).unwrap();
        let t = s.textures_for("acc");
        assert_eq!(t.skin.unwrap().0, skin);
        assert_eq!(t.cape.unwrap(), cape);
        assert!(s.assign("acc", TextureKind::Cape, Some(&a.id)).is_err());

        s.update_skin(&a.id, Some("Renamed"), Some(SkinModel::Slim))
            .unwrap();
        assert_eq!(s.textures_for("acc").skin.unwrap().1, SkinModel::Slim);

        // The cape image is also a skin entry: deleting the cape keeps the file.
        s.delete(TextureKind::Cape, &c.id).unwrap();
        assert!(s.texture_path(&c.id).unwrap().is_file());
        assert!(s.textures_for("acc").cape.is_none());
        s.delete(TextureKind::Skin, &c.id).unwrap();
        assert!(!s.texture_path(&c.id).unwrap().is_file());

        s.delete(TextureKind::Skin, &a.id).unwrap();
        assert!(
            s.view().assignments.is_empty(),
            "empty assignments are dropped"
        );
        assert_eq!(
            s.delete(TextureKind::Skin, &a.id).unwrap_err().code(),
            "skin.notFound"
        );
    }

    #[test]
    fn rejects_bad_input() {
        let (tmp, s) = store();
        assert!(s.add_skin(&png(64, 48, |_, _| false), "x", None).is_err());
        assert!(s.add_cape(&png(64, 64, |_, _| false), "x").is_err());
        assert!(s.texture_path("../../evil").is_err());
        let txt = tmp.path().join("skin.txt");
        std::fs::write(&txt, png(64, 64, |_, _| false)).unwrap();
        assert!(s.import_file(TextureKind::Skin, &txt, None).is_err());
        assert!(
            s.import_file(TextureKind::Skin, Path::new("rel.png"), None)
                .is_err()
        );
        let ok = tmp.path().join("Steve2.png");
        std::fs::write(&ok, png(64, 64, |_, _| false)).unwrap();
        let id = s.import_file(TextureKind::Skin, &ok, None).unwrap();
        assert_eq!(s.view().skins[0].entry.name, "Steve2");
        let out = tmp.path().join("out.png");
        s.export(&id, &out).unwrap();
        assert!(s.export(&id, &tmp.path().join("out.exe")).is_err());
    }
}
