//! Small filesystem helpers shared across modules.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{CoreError, Result};

/// Writes `bytes` to `path` atomically: write to a sibling temp file, flush,
/// then rename over the target. A crash never leaves a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::io(path, std::io::Error::other("path has no parent")))?;
    fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = parent.join(format!(".{file_name}.tmp"));
    {
        let mut f = fs::File::create(&tmp).map_err(|e| CoreError::io(&tmp, e))?;
        f.write_all(bytes).map_err(|e| CoreError::io(&tmp, e))?;
        f.sync_all().map_err(|e| CoreError::io(&tmp, e))?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        CoreError::io(path, e)
    })
}

/// Serializes `value` as pretty JSON and writes it atomically.
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| CoreError::Json {
        path: path.to_owned(),
        source,
    })?;
    write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a").join("f.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }
}
