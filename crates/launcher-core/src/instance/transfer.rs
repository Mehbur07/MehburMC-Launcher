//! Instance export/import as a zip with `instance.json` at the root.
//! Import extracts into a staging folder with zip-slip protection, validates
//! the manifest and only then moves the folder into place under a new id.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;

use super::{INSTANCE_FILE, Instance, InstanceStore, SUBDIRS, now_secs};
use crate::archive::extract_zip;
use crate::error::{CoreError, Result};
use crate::state::LauncherState;

/// Top-level entries never exported (regenerated or machine-specific).
const SKIP: &[&str] = &["logs", "crash-reports", ".mehbur.lock", "natives"];

pub fn export(store: &InstanceStore, id: &str, dest: &Path) -> Result<()> {
    let inst = store.get(id)?;
    let root = store.dir(&inst.id)?;
    let tmp = dest.with_extension("zip.part");
    let file = File::create(&tmp).map_err(|e| CoreError::io(&tmp, e))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let result = (|| -> Result<()> {
        add_dir(&mut zip, &root, &root, opts, &tmp)?;
        zip.finish().map_err(|source| CoreError::Archive {
            path: tmp.clone(),
            source,
        })?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest).map_err(|e| CoreError::io(dest, e))?;
    tracing::info!(%id, dest = %dest.display(), "instance exported");
    Ok(())
}

fn add_dir(
    zip: &mut zip::ZipWriter<File>,
    root: &Path,
    dir: &Path,
    opts: SimpleFileOptions,
    archive: &Path,
) -> Result<()> {
    let zerr = |source| CoreError::Archive {
        path: archive.to_owned(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(|e| CoreError::io(dir, e))? {
        let entry = entry.map_err(|e| CoreError::io(dir, e))?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if dir == root && SKIP.contains(&rel_str.as_str()) {
            continue;
        }
        let ty = entry.file_type().map_err(|e| CoreError::io(&path, e))?;
        if ty.is_dir() {
            zip.add_directory(format!("{rel_str}/"), opts)
                .map_err(zerr)?;
            add_dir(zip, root, &path, opts, archive)?;
        } else if ty.is_file() {
            zip.start_file(rel_str, opts).map_err(zerr)?;
            let mut f = File::open(&path).map_err(|e| CoreError::io(&path, e))?;
            io::copy(&mut f, zip).map_err(|e| CoreError::io(&path, e))?;
        }
    }
    zip.flush().map_err(|e| CoreError::io(archive, e))
}

pub fn import(store: &InstanceStore, src: &Path) -> Result<Instance> {
    let instances = store.paths().instances();
    let staging = instances.join(format!(".import-{}", now_secs()));
    let _ = std::fs::remove_dir_all(&staging);

    let result = (|| -> Result<Instance> {
        extract_zip(src, &staging, |name| Some(name.to_owned()))?;
        let manifest = staging.join(INSTANCE_FILE);
        let bytes = std::fs::read(&manifest)
            .map_err(|_| CoreError::InvalidInstance("archive has no instance.json".into()))?;
        let imported: Instance =
            serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
                path: manifest.clone(),
                source,
            })?;
        if imported.name.trim().is_empty() || imported.mc_version.trim().is_empty() {
            return Err(CoreError::InvalidInstance(
                "instance.json is incomplete".into(),
            ));
        }
        Ok(imported)
    })();
    let imported = match result {
        Ok(i) => i,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };

    let id = store.unique_id(&imported.name);
    let target = instances.join(&id);
    std::fs::rename(&staging, &target).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        CoreError::io(&target, e)
    })?;
    for sub in SUBDIRS {
        let _ = std::fs::create_dir_all(target.join(sub));
    }
    let inst = Instance {
        id: id.clone(),
        created_at: now_secs(),
        ..imported
    };
    store.save(&inst)?;
    LauncherState::update(store.paths(), |s| s.instance_order.insert(0, id.clone()))?;
    tracing::info!(%id, src = %src.display(), "instance imported");
    Ok(inst)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{new, store};
    use super::*;

    #[test]
    fn export_import_roundtrip() {
        let (tmp, s) = store();
        let a = s.create(new("Export Me")).unwrap();
        let dir = s.dir(&a.id).unwrap();
        std::fs::create_dir_all(dir.join("saves/World1")).unwrap();
        std::fs::write(dir.join("saves/World1/level.dat"), b"lvl").unwrap();
        std::fs::write(dir.join("logs/latest.log"), b"skip me").unwrap();

        let zip = tmp.path().join("out.zip");
        export(&s, &a.id, &zip).unwrap();
        let b = import(&s, &zip).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(b.name, "Export Me");
        let bdir = s.dir(&b.id).unwrap();
        assert_eq!(
            std::fs::read(bdir.join("saves/World1/level.dat")).unwrap(),
            b"lvl"
        );
        assert!(!bdir.join("logs/latest.log").exists());
        assert!(bdir.join("logs").is_dir());
        assert_eq!(s.list().len(), 2);
    }

    #[test]
    fn rejects_archives_without_manifest() {
        let (tmp, s) = store();
        let zip = tmp.path().join("bad.zip");
        let mut w = zip::ZipWriter::new(File::create(&zip).unwrap());
        w.start_file("random.txt", SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
        assert_eq!(import(&s, &zip).unwrap_err().code(), "instance.invalid");
        // Staging folder is cleaned up.
        let leftovers = std::fs::read_dir(s.paths().instances())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".import")
            })
            .count();
        assert_eq!(leftovers, 0);
    }
}
