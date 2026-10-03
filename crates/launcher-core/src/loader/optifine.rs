//! Standalone OptiFine (LaunchWrapper + `optifine.OptiFineTweaker`).
//!
//! OptiFine has no download API and its license forbids redistribution, so
//! the user imports a jar they downloaded themselves. It is copied to
//! `loaders/optifine/` and installed like the official installer does:
//! `optifine.Patcher` builds the `optifine:OptiFine` library from the vanilla
//! client jar, and the bundled `launchwrapper-of` is extracted.
//!
//! On Forge, OptiFine is simply a mod (put the jar into `mods/`).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::LoaderVersion;
use super::processors::library_path;
use crate::archive;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::java;
use crate::launch::ensure_client;
use crate::paths::Paths;

const TWEAKER: &str = "optifine.OptiFineTweaker";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OptifineInfo {
    pub mc_version: String,
    /// `HD_U_I6`, `HD_U_J1_pre9`, …
    pub edition: String,
}

fn invalid(path: &Path) -> CoreError {
    CoreError::OptifineInvalid {
        path: path.to_owned(),
    }
}

/// Parses `OptiFine 1.20.1_HD_U_I6` (first line of `changelog.txt`).
fn parse_title(line: &str) -> Option<OptifineInfo> {
    let rest = line.trim().strip_prefix("OptiFine ")?;
    let (mc, edition) = rest.split_once('_')?;
    let mc_ok = !mc.is_empty() && mc.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
    let ed_ok = edition.starts_with("HD")
        && edition
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    (mc_ok && ed_ok).then(|| OptifineInfo {
        mc_version: mc.to_owned(),
        edition: edition.to_owned(),
    })
}

/// Reads version information from an OptiFine jar.
pub fn inspect(jar: &Path) -> Result<OptifineInfo> {
    let names = archive::entry_names(jar).map_err(|_| invalid(jar))?;
    if !names.iter().any(|n| n == "optifine/OptiFineTweaker.class") {
        return Err(invalid(jar));
    }
    let changelog = archive::read_entry(jar, "changelog.txt")?.ok_or_else(|| invalid(jar))?;
    let text = String::from_utf8_lossy(&changelog);
    text.lines()
        .find(|l| !l.trim().is_empty())
        .and_then(parse_title)
        .ok_or_else(|| invalid(jar))
}

pub fn stored_jar(paths: &Paths, mc: &str, edition: &str) -> PathBuf {
    paths
        .loaders()
        .join("optifine")
        .join(format!("OptiFine_{mc}_{edition}.jar"))
}

/// Copies a user-provided OptiFine jar into the loader cache.
pub fn import(paths: &Paths, src: &Path) -> Result<OptifineInfo> {
    let info = inspect(src)?;
    let dest = stored_jar(paths, &info.mc_version, &info.edition);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    let tmp = dest.with_extension("jar.tmp");
    std::fs::copy(src, &tmp).map_err(|e| CoreError::io(&tmp, e))?;
    std::fs::rename(&tmp, &dest).map_err(|e| CoreError::io(&dest, e))?;
    tracing::info!(mc = %info.mc_version, edition = %info.edition, "OptiFine imported");
    Ok(info)
}

/// Imported editions for `mc`, newest first.
pub fn list(paths: &Paths, mc: &str) -> Vec<LoaderVersion> {
    let prefix = format!("OptiFine_{mc}_");
    let Ok(rd) = std::fs::read_dir(paths.loaders().join("optifine")) else {
        return vec![];
    };
    let mut out: Vec<LoaderVersion> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let edition = name.strip_prefix(&prefix)?.strip_suffix(".jar")?.to_owned();
            Some(LoaderVersion {
                label: edition.replace('_', " "),
                stable: !edition.contains("pre"),
                id: edition,
                recommended: false,
            })
        })
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub async fn install(
    ctx: &Ctx,
    mc: &str,
    edition: &str,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<serde_json::Value> {
    let jar = stored_jar(&ctx.paths, mc, edition);
    if !jar.is_file() {
        return Err(CoreError::OptifineMissing {
            mc: mc.to_owned(),
            edition: edition.to_owned(),
        });
    }
    let names = archive::entry_names(&jar)?;
    let libraries = ctx.paths.libraries();
    let (vanilla, client_jar) = ensure_client(ctx, mc, progress, cancel).await?;
    progress.set_totals(2, 0);

    // 1. optifine:OptiFine library = jar patched against the vanilla client.
    let lib_coord = format!("optifine:OptiFine:{mc}_{edition}");
    let lib_path = library_path(&libraries, &lib_coord).ok_or_else(|| invalid(&jar))?;
    if let Some(parent) = lib_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    if names.iter().any(|n| n == "optifine/Patcher.class") {
        let java = java::resolve(ctx, vanilla.json.java_major(), None, true, cancel).await?;
        let mut cmd = Command::new(java.executable_path());
        cmd.arg("-cp")
            .arg(&jar)
            .arg("optifine.Patcher")
            .arg(&client_jar)
            .arg(&jar)
            .arg(&lib_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let child = cmd.spawn().map_err(|source| CoreError::Spawn {
            program: java.executable_path(),
            source,
        })?;
        let out = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            o = child.wait_with_output() => o.map_err(|e| CoreError::io(&jar, e))?,
        };
        if !out.status.success() || !lib_path.is_file() {
            let text = String::from_utf8_lossy(&out.stderr);
            return Err(CoreError::LoaderInstall {
                loader: "OptiFine".into(),
                reason: format!(
                    "optifine.Patcher exited with {:?}: {}",
                    out.status.code(),
                    text.lines().rev().take(5).collect::<Vec<_>>().join(" | ")
                ),
            });
        }
    } else {
        std::fs::copy(&jar, &lib_path).map_err(|e| CoreError::io(&lib_path, e))?;
    }
    progress.item_done();

    // 2. LaunchWrapper: OptiFine's fork if bundled (Java 9+ compatible).
    let mut libs = vec![local_library(&lib_coord)];
    let lw_version = archive::read_entry(&jar, "launchwrapper-of.txt")?
        .map(|b| String::from_utf8_lossy(&b).trim().to_owned())
        .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'));
    if let Some(v) = lw_version {
        let coord = format!("optifine:launchwrapper-of:{v}");
        let dest = library_path(&libraries, &coord).ok_or_else(|| invalid(&jar))?;
        if !archive::extract_entry(&jar, &format!("launchwrapper-of-{v}.jar"), &dest)? {
            return Err(invalid(&jar));
        }
        libs.push(local_library(&coord));
    } else if names.iter().any(|n| n == "launchwrapper-2.0.jar") {
        let coord = "optifine:launchwrapper:2.0";
        let dest = library_path(&libraries, coord).ok_or_else(|| invalid(&jar))?;
        archive::extract_entry(&jar, "launchwrapper-2.0.jar", &dest)?;
        libs.push(local_library(coord));
    } else {
        libs.push(serde_json::json!({ "name": "net.minecraft:launchwrapper:1.12" }));
    }
    progress.item_done();

    let mut json = serde_json::json!({
        "type": vanilla.json.kind.clone().unwrap_or_else(|| "release".into()),
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "libraries": libs,
    });
    match &vanilla.json.minecraft_arguments {
        // Legacy: the child string replaces the parent's, so repeat it.
        Some(args) if vanilla.json.arguments.is_none() => {
            json["minecraftArguments"] = format!("{args} --tweakClass {TWEAKER}").into();
        }
        _ => {
            json["arguments"] = serde_json::json!({ "game": ["--tweakClass", TWEAKER] });
        }
    }
    Ok(json)
}

/// Library that already exists locally (no download).
fn local_library(coord: &str) -> serde_json::Value {
    let path = crate::maven::Coordinate::parse(coord)
        .map(|c| c.path())
        .unwrap_or_default();
    serde_json::json!({ "name": coord, "downloads": { "artifact": { "path": path, "url": "" } } })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn title_parsing() {
        assert_eq!(
            parse_title("OptiFine 1.20.1_HD_U_I6"),
            Some(OptifineInfo {
                mc_version: "1.20.1".into(),
                edition: "HD_U_I6".into()
            })
        );
        assert_eq!(
            parse_title("OptiFine 1.21.1_HD_U_J1_pre9").unwrap().edition,
            "HD_U_J1_pre9"
        );
        assert!(parse_title("OptiFine ../x_HD").is_none());
        assert!(parse_title("Something else").is_none());
    }

    #[test]
    fn import_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path().join("MehburMC"));
        let jar = dir.path().join("of.jar");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("optifine/OptiFineTweaker.class", o).unwrap();
            w.write_all(b"x").unwrap();
            w.start_file("changelog.txt", o).unwrap();
            w.write_all(b"OptiFine 1.20.1_HD_U_I6\n - fixes\n").unwrap();
            w.finish().unwrap();
        }
        let info = import(&paths, &jar).unwrap();
        assert_eq!(info.edition, "HD_U_I6");
        assert!(stored_jar(&paths, "1.20.1", "HD_U_I6").is_file());
        let l = list(&paths, "1.20.1");
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].id, "HD_U_I6");
        assert!(list(&paths, "1.12.2").is_empty());

        let not_of = dir.path().join("x.jar");
        std::fs::write(&not_of, b"not a zip").unwrap();
        assert_eq!(
            import(&paths, &not_of).unwrap_err().code(),
            "loader.optifineInvalid"
        );
    }
}
