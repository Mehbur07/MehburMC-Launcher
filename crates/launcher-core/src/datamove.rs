//! "Move data folder" (ARCHITECTURE.md K4): copy → verify → write
//! `redirect.json` → delete the old copy. Works across drives. `launcher/`
//! never moves, so the redirect can always be found again.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::paths::Paths;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub files: u64,
    pub bytes: u64,
}

/// Locked while the launcher runs (portable WebView2 profile); recreated.
fn skipped(rel: &Path) -> bool {
    rel.starts_with(Path::new("cache").join("webview"))
}

fn walk(root: &Path, dir: &Path, f: &mut dyn FnMut(&Path, &Path, u64) -> Result<()>) -> Result<()> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(CoreError::io(dir, e)),
    };
    for entry in rd {
        let entry = entry.map_err(|e| CoreError::io(dir, e))?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        if skipped(&rel) {
            continue;
        }
        let ft = entry.file_type().map_err(|e| CoreError::io(&path, e))?;
        if ft.is_symlink() {
            // Never follow links out of the data folder.
            continue;
        }
        if ft.is_dir() {
            walk(root, &path, f)?;
        } else {
            let len = entry.metadata().map_err(|e| CoreError::io(&path, e))?.len();
            f(&path, &rel, len)?;
        }
    }
    Ok(())
}

fn measure(root: &Path) -> Result<Totals> {
    let mut t = Totals::default();
    for d in Paths::content_dir_names() {
        walk(root, &root.join(d), &mut |_, _, len| {
            t.files += 1;
            t.bytes += len;
            Ok(())
        })?;
    }
    Ok(t)
}

fn is_empty_dir(p: &Path) -> bool {
    fs::read_dir(p)
        .map(|mut r| r.next().is_none())
        .unwrap_or(true)
}

/// `\\?\C:\x` → `C:\x` (what `canonicalize` returns on Windows); UNC and
/// other verbatim forms are kept as they are.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

fn normalize(p: &Path) -> PathBuf {
    // `canonicalize` fails for paths that do not exist yet; canonicalize the
    // deepest existing ancestor instead.
    let mut existing = p;
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => break,
        }
    }
    let mut out = existing
        .canonicalize()
        .map(strip_verbatim)
        .unwrap_or_else(|_| existing.to_path_buf());
    for name in rest.into_iter().rev() {
        out.push(name);
    }
    out
}

fn invalid(reason: &str) -> CoreError {
    CoreError::DataMove(reason.to_owned())
}

/// Checks a destination; returns it normalised. Moving back to the default
/// location is allowed although that folder holds `launcher/`.
pub fn check_destination(paths: &Paths, dest: &Path) -> Result<PathBuf> {
    if !dest.is_absolute() {
        return Err(invalid("not absolute"));
    }
    let dest = normalize(dest);
    let current = normalize(paths.content());
    let default = normalize(paths.mc());
    if dest == current {
        return Err(invalid("same folder"));
    }
    if dest != default && (dest.starts_with(&current) || current.starts_with(&dest)) {
        return Err(invalid("nested folder"));
    }
    if dest == default {
        // Leftover empty folders and the (skipped) WebView2 profile are fine.
        if measure(&dest).map_or(true, |t| t.files > 0) {
            return Err(invalid("not empty"));
        }
    } else if dest.exists() && !is_empty_dir(&dest) {
        return Err(invalid("not empty"));
    }
    Ok(dest)
}

/// Moves all content folders to `dest`. `progress(done_bytes, total_bytes)`.
/// Returns files of the old copy that could not be deleted (e.g. the open
/// launcher log); the move itself has succeeded at that point.
pub fn move_content(
    paths: &Paths,
    dest: &Path,
    progress: &dyn Fn(u64, u64),
) -> Result<Vec<PathBuf>> {
    let dest = check_destination(paths, dest)?;
    let src = paths.content().to_path_buf();
    let total = measure(&src)?;

    fs::create_dir_all(&dest).map_err(|e| CoreError::io(&dest, e))?;
    if let Ok(avail) = fs4::available_space(&dest)
        && avail < total.bytes + 64 * 1024 * 1024
    {
        return Err(CoreError::DiskFull {
            path: dest,
            needed: total.bytes,
            available: avail,
        });
    }

    // 1. Copy. Verification compares against what was actually copied: the
    //    source may still grow meanwhile (the launcher's own log file).
    let mut copied = Totals::default();
    let copy = (|| -> Result<()> {
        for d in Paths::content_dir_names() {
            fs::create_dir_all(dest.join(d)).map_err(|e| CoreError::io(dest.join(d), e))?;
            walk(&src, &src.join(d), &mut |from, rel, _| {
                let to = dest.join(rel);
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
                }
                let n = fs::copy(from, &to).map_err(|e| CoreError::io(&to, e))?;
                copied.files += 1;
                copied.bytes += n;
                progress(copied.bytes.min(total.bytes), total.bytes);
                Ok(())
            })?;
        }
        Ok(())
    })();

    // 2. Verify (count + size); on any failure the new copy is removed and
    //    the old one stays authoritative.
    let verified = copy.and_then(|()| {
        let found = measure(&dest)?;
        if found == copied {
            Ok(())
        } else {
            Err(invalid(&format!(
                "verification failed: {} files / {} bytes found, {} / {} copied",
                found.files, found.bytes, copied.files, copied.bytes
            )))
        }
    });
    if let Err(e) = verified {
        if dest != normalize(paths.mc()) {
            let _ = fs::remove_dir_all(&dest);
        }
        return Err(e);
    }

    // 3. Switch.
    let default = normalize(paths.mc());
    paths.set_redirect((dest != default).then_some(dest.as_path()))?;
    tracing::info!(from = %src.display(), to = %dest.display(), files = copied.files, "data folder moved");

    // 4. Delete the old copy (best effort).
    let mut leftovers = Vec::new();
    for d in Paths::content_dir_names() {
        let dir = src.join(d);
        if fs::remove_dir_all(&dir).is_err() {
            walk(&src, &dir, &mut |p, _, _| {
                if fs::remove_file(p).is_err() {
                    leftovers.push(p.to_path_buf());
                }
                Ok(())
            })
            .ok();
        }
    }
    if !leftovers.is_empty() {
        tracing::warn!(
            count = leftovers.len(),
            "some old files could not be deleted"
        );
    }
    Ok(leftovers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        fs::create_dir_all(paths.instances().join("a/saves")).unwrap();
        fs::write(paths.instances().join("a/saves/level.dat"), b"world").unwrap();
        fs::write(paths.versions().join("v.json"), b"{}").unwrap();
        fs::write(paths.settings_file(), b"{}").unwrap();
        (tmp, paths)
    }

    #[test]
    fn moves_and_moves_back() {
        let (tmp, paths) = setup();
        let dest = tmp.path().join("D").join("Mehbur data");
        let last = AtomicU64::new(0);
        let left = move_content(&paths, &dest, &|d, t| {
            assert!(d <= t);
            last.store(d, Ordering::SeqCst);
        })
        .unwrap();
        assert!(left.is_empty());
        assert_eq!(last.load(Ordering::SeqCst), 7);
        assert_eq!(
            fs::read(dest.join("instances/a/saves/level.dat")).unwrap(),
            b"world"
        );
        assert!(!paths.instances().join("a").exists(), "old copy deleted");
        assert!(paths.settings_file().exists(), "launcher/ never moves");

        let moved = Paths::from_root(paths.root().to_path_buf(), paths.mode()).unwrap();
        assert_eq!(normalize(moved.content()), normalize(&dest));

        // Back to the default location removes the redirect.
        move_content(&moved, paths.mc(), &|_, _| {}).unwrap();
        let back = Paths::from_root(paths.root().to_path_buf(), paths.mode()).unwrap();
        assert!(!back.info().redirected);
        assert!(back.instances().join("a/saves/level.dat").exists());
    }

    #[test]
    fn strips_windows_verbatim_prefix() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\Games\Data")),
            PathBuf::from(r"C:\Games\Data")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
    }

    #[test]
    fn rejects_bad_destinations() {
        let (tmp, paths) = setup();
        assert!(check_destination(&paths, Path::new("relative")).is_err());
        assert!(check_destination(&paths, paths.content()).is_err());
        assert!(check_destination(&paths, &paths.instances().join("x")).is_err());
        let full = tmp.path().join("full");
        fs::create_dir_all(&full).unwrap();
        fs::write(full.join("f"), b"").unwrap();
        assert!(check_destination(&paths, &full).is_err());
        // Default location with content already there.
        assert!(check_destination(&paths, paths.mc()).is_err());
        // ... but a leftover WebView2 profile does not count.
        let moved_paths = {
            let dest = tmp.path().join("elsewhere");
            move_content(&paths, &dest, &|_, _| {}).unwrap();
            Paths::from_root(paths.root().to_path_buf(), paths.mode()).unwrap()
        };
        fs::create_dir_all(paths.mc().join("cache/webview/EBWebView")).unwrap();
        fs::write(paths.mc().join("cache/webview/EBWebView/lock"), b"x").unwrap();
        assert!(check_destination(&moved_paths, paths.mc()).is_ok());
        assert!(check_destination(&paths, &tmp.path().join("new")).is_ok());
    }
}
