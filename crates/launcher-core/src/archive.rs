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
    map: impl FnMut(&str) -> Option<String>,
) -> Result<usize> {
    extract_zip_with(archive, dest, false, map)
}

/// Like [`extract_zip`]; with `overwrite` every file is rewritten, otherwise
/// files of identical size are skipped (natives may be locked by a game).
pub fn extract_zip_with(
    archive: &Path,
    dest: &Path,
    overwrite: bool,
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
        let name = entry
            .name()
            .map_err(|source| CoreError::Archive {
                path: archive.to_owned(),
                source,
            })?
            .replace('\\', "/");
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
        if !overwrite && fs::metadata(&target).is_ok_and(|m| m.len() == entry.size()) {
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

/// Extracts a `.tar.gz` into `dest`, keeping permission bits so programs stay
/// executable. Same contract as [`extract_zip`]. No link ever reaches the disk:
/// a symbolic link to a file inside the archive becomes a copy of that file.
pub fn extract_tar_gz(
    archive: &Path,
    dest: &Path,
    mut map: impl FnMut(&str) -> Option<String>,
) -> Result<usize> {
    let io_err = |e: io::Error| CoreError::io(archive, e);
    let unsafe_entry = |entry: &str| CoreError::UnsafeArchiveEntry {
        path: archive.to_owned(),
        entry: entry.to_owned(),
    };
    let file = File::open(archive).map_err(io_err)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut links = Vec::new();
    let mut written = 0;
    for entry in tar.entries().map_err(io_err)? {
        let mut entry = entry.map_err(io_err)?;
        let name = entry
            .path()
            .map_err(io_err)?
            .to_string_lossy()
            .replace('\\', "/");
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        let Some(rel) = map(&name) else { continue };
        let Some(target) = safe_join(dest, &rel) else {
            return Err(unsafe_entry(&name));
        };
        if kind.is_symlink() {
            let link = entry
                .link_name()
                .map_err(io_err)?
                .ok_or_else(|| unsafe_entry(&name))?;
            // Relative to the link's own folder, and it may not leave `dest`.
            let folder = Path::new(&rel).parent().unwrap_or(Path::new(""));
            let source = lexical_join(folder, &link.to_string_lossy())
                .and_then(|p| safe_join(dest, &p))
                .ok_or_else(|| unsafe_entry(&name))?;
            links.push((source, target));
            continue;
        }
        if !kind.is_file() {
            return Err(unsafe_entry(&name));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let mut out = File::create(&target).map_err(|e| CoreError::io(&target, e))?;
        io::copy(&mut entry, &mut out).map_err(|e| CoreError::io(&target, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = entry.header().mode().map_err(io_err)? & 0o755;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .map_err(|e| CoreError::io(&target, e))?;
        }
        written += 1;
    }
    for (source, target) in links {
        // Folder links and links to missing files are of no use to a runtime.
        if source.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
            }
            fs::copy(&source, &target).map_err(|e| CoreError::io(&target, e))?;
            written += 1;
        }
    }
    Ok(written)
}

/// `a/b` + `../c` → `a/c`; `None` for an absolute link or one above the root.
fn lexical_join(folder: &Path, link: &str) -> Option<String> {
    if link.starts_with('/') || link.contains('\\') {
        return None;
    }
    let mut parts: Vec<String> = folder
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for part in link.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other.to_owned()),
        }
    }
    Some(parts.join("/"))
}

fn open(archive: &Path) -> Result<zip::ZipArchive<File>> {
    let file = File::open(archive).map_err(|e| CoreError::io(archive, e))?;
    zip::ZipArchive::new(file).map_err(|source| CoreError::Archive {
        path: archive.to_owned(),
        source,
    })
}

/// Names of all file entries (`/` separators).
pub fn entry_names(archive: &Path) -> Result<Vec<String>> {
    let zip = open(archive)?;
    let mut names = Vec::new();
    for name in zip.file_names() {
        let name = name.map_err(|source| CoreError::Archive {
            path: archive.to_owned(),
            source,
        })?;
        if !name.ends_with('/') {
            names.push(name.replace('\\', "/"));
        }
    }
    Ok(names)
}

/// Reads one entry fully into memory; `None` if it does not exist.
pub fn read_entry(archive: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let mut zip = open(archive)?;
    let mut entry = match zip.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(source) => {
            return Err(CoreError::Archive {
                path: archive.to_owned(),
                source,
            });
        }
    };
    let mut buf = Vec::with_capacity(entry.size() as usize);
    io::copy(&mut entry, &mut buf).map_err(|e| CoreError::io(archive, e))?;
    Ok(Some(buf))
}

/// Reads several entries with one pass over the central directory; missing
/// entries are `None`. Much faster than repeated [`read_entry`] on big jars.
pub fn read_entries(archive: &Path, names: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
    let mut zip = open(archive)?;
    names
        .iter()
        .map(|name| {
            let mut entry = match zip.by_name(name) {
                Ok(e) => e,
                Err(zip::result::ZipError::FileNotFound) => return Ok(None),
                Err(source) => {
                    return Err(CoreError::Archive {
                        path: archive.to_owned(),
                        source,
                    });
                }
            };
            let mut buf = Vec::with_capacity(entry.size() as usize);
            io::copy(&mut entry, &mut buf).map_err(|e| CoreError::io(archive, e))?;
            Ok(Some(buf))
        })
        .collect()
}

/// Copies one entry to `dest` (atomically). Returns `false` if it is missing.
pub fn extract_entry(archive: &Path, name: &str, dest: &Path) -> Result<bool> {
    match read_entry(archive, name)? {
        Some(bytes) => {
            crate::fsutil::write_atomic(dest, &bytes)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// `Main-Class` from a jar's manifest.
pub fn main_class(jar: &Path) -> Result<Option<String>> {
    let Some(bytes) = read_entry(jar, "META-INF/MANIFEST.MF")? else {
        return Ok(None);
    };
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:"))
        .map(|s| s.trim().to_owned()))
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

        assert_eq!(read_entry(&good, "lwjgl.dll").unwrap().unwrap(), b"x");
        assert!(read_entry(&good, "nope").unwrap().is_none());
        assert_eq!(entry_names(&good).unwrap().len(), 2);
        assert!(extract_entry(&good, "lwjgl.dll", &out.join("copy.dll")).unwrap());

        let evil = dir.path().join("evil.zip");
        make_zip(&evil, &[("../../escaped.txt", b"pwn")]);
        let err = extract_zip(&evil, &out, |n| Some(n.to_owned())).unwrap_err();
        assert_eq!(err.code(), "archive.unsafe");
        assert!(!dir.path().join("escaped.txt").exists());
    }

    /// `(name, mode, data)`, or a symbolic link when `data` starts with `->`.
    fn make_tar_gz(path: &Path, entries: &[(&str, u32, &[u8])]) {
        let gz =
            flate2::write::GzEncoder::new(File::create(path).unwrap(), flate2::Compression::fast());
        let mut w = tar::Builder::new(gz);
        for (name, mode, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_mode(*mode);
            // Written by hand: the builder's helpers refuse the evil names.
            h.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
            match data.strip_prefix(b"->") {
                Some(link) => {
                    h.set_entry_type(tar::EntryType::Symlink);
                    h.as_old_mut().linkname[..link.len()].copy_from_slice(link);
                    h.set_size(0);
                    h.set_cksum();
                    w.append(&h, io::empty()).unwrap();
                }
                None => {
                    h.set_size(data.len() as u64);
                    h.set_cksum();
                    w.append(&h, *data).unwrap();
                }
            }
        }
        w.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn extracts_tar_gz_like_a_java_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("jre.tar.gz");
        make_tar_gz(
            &good,
            &[
                ("jdk-25/bin/java", 0o755, b"elf"),
                ("jdk-25/lib/libjsig.so", 0o644, b"so"),
                ("jdk-25/lib/server/libjsig.so", 0o777, b"->../libjsig.so"),
                ("jdk-25/legal/LICENSE", 0o777, b"->gone"),
            ],
        );
        let out = dir.path().join("out");
        let strip = |n: &str| {
            n.split_once('/')
                .map(|(_, rest)| rest.to_owned())
                .filter(|r| !r.is_empty())
        };
        assert_eq!(extract_tar_gz(&good, &out, strip).unwrap(), 3);
        assert_eq!(fs::read(out.join("bin/java")).unwrap(), b"elf");
        let copy = out.join("lib/server/libjsig.so");
        assert_eq!(fs::read(&copy).unwrap(), b"so");
        assert!(!fs::symlink_metadata(&copy).unwrap().is_symlink());
        assert!(!out.join("legal/LICENSE").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &str| fs::metadata(out.join(p)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode("bin/java"), 0o755);
            assert_eq!(mode("lib/libjsig.so"), 0o644);
        }
    }

    #[test]
    fn tar_gz_cannot_leave_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let secret = dir.path().join("secret");
        fs::write(&secret, b"s").unwrap();
        let cases: [(&str, u32, &[u8]); 5] = [
            ("../escaped", 0o644, b"x"),
            ("/abs", 0o644, b"x"),
            ("link", 0o777, b"->../secret"),
            ("link", 0o777, b"->/etc/passwd"),
            ("a/link", 0o777, b"->../../secret"),
        ];
        for (i, entry) in cases.into_iter().enumerate() {
            let evil = dir.path().join(format!("evil{i}.tar.gz"));
            make_tar_gz(&evil, &[entry]);
            let err = extract_tar_gz(&evil, &out, |n| Some(n.to_owned())).unwrap_err();
            assert_eq!(err.code(), "archive.unsafe", "{entry:?}");
        }
        assert!(!dir.path().join("escaped").exists());
        assert!(!out.join("link").exists());
    }
}
