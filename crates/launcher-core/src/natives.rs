//! Extracts native libraries into `versions/<id>/natives/`.

use std::path::Path;

use crate::archive::extract_zip;
use crate::error::Result;
use crate::library::NativeJar;

const NATIVE_EXTS: &[&str] = &[".dll", ".so", ".dylib", ".jnilib"];

pub fn extract_all(jars: &[NativeJar], dest: &Path) -> Result<usize> {
    std::fs::create_dir_all(dest).map_err(|e| crate::CoreError::io(dest, e))?;
    let mut total = 0;
    for jar in jars {
        let excluded = |name: &str| jar.exclude.iter().any(|ex| name.starts_with(ex.as_str()));
        total += extract_zip(&jar.artifact.path, dest, |name| {
            if excluded(name) {
                return None;
            }
            if jar.flatten {
                // Keep only native binaries, dropped flat into `dest`.
                let lower = name.to_ascii_lowercase();
                if !NATIVE_EXTS.iter().any(|ext| lower.ends_with(ext)) {
                    return None;
                }
                return name.rsplit('/').next().map(str::to_owned);
            }
            Some(name.to_owned())
        })?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;

    use super::*;
    use crate::library::LibArtifact;

    fn jar(path: &Path, entries: &[&str]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for e in entries {
            w.start_file(*e, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"bin").unwrap();
        }
        w.finish().unwrap();
    }

    fn native(path: &Path, flatten: bool) -> NativeJar {
        NativeJar {
            artifact: LibArtifact {
                name: "x".into(),
                path: path.to_owned(),
                url: None,
                sha1: None,
                size: None,
            },
            exclude: vec!["META-INF/".into()],
            flatten,
        }
    }

    #[test]
    fn legacy_keeps_layout_modern_flattens() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("legacy.jar");
        jar(&legacy, &["lwjgl64.dll", "META-INF/MANIFEST.MF"]);
        let modern = dir.path().join("modern.jar");
        jar(
            &modern,
            &[
                "windows/x64/org/lwjgl/lwjgl.dll",
                "windows/x64/org/lwjgl/lwjgl.dll.sha1",
                "META-INF/x",
            ],
        );
        let out = dir.path().join("natives");
        let n = extract_all(&[native(&legacy, false), native(&modern, true)], &out).unwrap();
        assert_eq!(n, 2);
        assert!(out.join("lwjgl64.dll").exists());
        assert!(out.join("lwjgl.dll").exists());
        assert!(!out.join("META-INF").exists());
        assert!(!out.join("lwjgl.dll.sha1").exists());
    }
}
