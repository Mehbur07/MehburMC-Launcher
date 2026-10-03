//! Zip extraction with zip-slip protection.
//!
//! Every entry name is normalised and rejected if it is absolute, contains a
//! drive/ADS colon, a `..` component, or is a symlink; the final path must stay
//! inside the destination root.

use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::error::{CoreError, Result};

/// Joins a zip entry name onto `root`, or `None` if the name is unsafe.
pub fn safe_join(root: &Path, entry: &str) -> Option<PathBuf> {
    let normalized = entry.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') || normalized.contains('\0') {
        return None;
    }
    let mut out = root.to_path_buf();
    let mut depth = 0usize;
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(c) => {
                out.push(c);
                depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (depth > 0).then_some(out)
}

/// Extracts `archive` into `dest`. `map` receives each file entry name (with
/// `/` separators) and returns the relative output path, or `None` to skip it.
/// Returns the number of files written.
pub fn extract_zip(
    archive: &Path,
    dest: &Path,
    mut map: impl FnMut(&str) -> Option<String>,
) -> Result<usize> {
    let file = File::open(archive).map_err(|e| CoreError::io(archive, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|source| CoreError::Archive {
        path: archive.to_owned(),
        source,
    })?;
    let mut written = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|source| CoreError::Archive {
            path: archive.to_owned(),
            source,
        })?;
        let name = entry.name().replace('\\', "/");
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            return Err(CoreError::UnsafeArchiveEntry {
                path: archive.to_owned(),
                entry: name,
            });
        }
        let Some(rel) = map(&name) else { continue };
        let Some(target) = safe_join(dest, &rel) else {
            return Err(CoreError::UnsafeArchiveEntry {
                path: archive.to_owned(),
                entry: name,
            });
        };
        // Skip identical files (natives may be locked by a running game).
        if fs::metadata(&target).is_ok_and(|m| m.len() == entry.size()) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let mut out = File::create(&target).map_err(|e| CoreError::io(&target, e))?;
        io::copy(&mut entry, &mut out).map_err(|e| CoreError::io(&target, e))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn safe_join_rejects_traversal() {
        let root = Path::new("/r");
        assert_eq!(
            safe_join(root, "a/b.dll"),
            Some(PathBuf::from("/r/a/b.dll"))
        );
        assert_eq!(safe_join(root, "./a.dll"), Some(PathBuf::from("/r/a.dll")));
        assert!(safe_join(root, "../evil.dll").is_none());
        assert!(safe_join(root, "a/../../evil").is_none());
        assert!(safe_join(root, "..\\evil").is_none());
        assert!(safe_join(root, "/etc/passwd").is_none());
        assert!(safe_join(root, "C:/Windows/x").is_none());
        assert!(safe_join(root, "file.txt:stream").is_none());
        assert!(safe_join(root, "").is_none());
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn extracts_and_blocks_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.zip");
        make_zip(
            &good,
            &[("lwjgl.dll", b"x"), ("META-INF/MANIFEST.MF", b"m")],
        );
        let out = dir.path().join("out");
        let n = extract_zip(&good, &out, |n| {
            (!n.starts_with("META-INF/")).then(|| n.to_owned())
        })
        .unwrap();
        assert_eq!(n, 1);
        assert!(out.join("lwjgl.dll").exists());
        assert!(!out.join("META-INF").exists());

        let evil = dir.path().join("evil.zip");
        make_zip(&evil, &[("../../escaped.txt", b"pwn")]);
        let err = extract_zip(&evil, &out, |n| Some(n.to_owned())).unwrap_err();
        assert_eq!(err.code(), "archive.unsafe");
        assert!(!dir.path().join("escaped.txt").exists());
    }
}
