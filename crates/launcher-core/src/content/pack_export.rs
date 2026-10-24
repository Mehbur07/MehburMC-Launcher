//! Turns an instance into a Modrinth `.mrpack` (K90), so a profile can be
//! shared as a modpack in the MehburMC Library.
//!
//! Mods, resource packs and shader packs that Modrinth knows (by SHA-1) are
//! listed as downloads from its CDN; everything else in those folders and
//! the config folders goes into `overrides/` (override jars then get the full
//! mod rules in the library scan). Worlds, logs, screenshots, `options.txt`,
//! `servers.dat` and other personal or machine-specific files stay out.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

use super::modrinth;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::instance::{Instance, LoaderKind};
use crate::loader;

/// Folders whose files may be Modrinth downloads, with the file extension.
const LINKABLE: &[(&str, &str)] = &[
    ("mods", ".jar"),
    ("resourcepacks", ".zip"),
    ("shaderpacks", ".zip"),
];

/// Folders copied whole into `overrides/` (mod settings).
const CONFIG_DIRS: &[&str] = &["config", "defaultconfigs"];

/// Only the CDN the library's scan accepts without a warning.
const MODRINTH_CDN: &str = "https://cdn.modrinth.com/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackExport {
    pub path: PathBuf,
    /// Files listed as Modrinth downloads.
    pub linked: usize,
    /// Files copied into the pack.
    pub embedded: usize,
}

/// The `dependencies` of the index; loaders a `.mrpack` cannot describe are
/// refused (`library.packLoader`). No saved loader version means the
/// recommended one, as when the instance is started.
async fn dependencies(ctx: &Ctx, inst: &Instance) -> Result<Value> {
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), json!(inst.mc_version));
    let key = match inst.loader.kind {
        LoaderKind::Vanilla => None,
        LoaderKind::Fabric => Some("fabric-loader"),
        LoaderKind::Quilt => Some("quilt-loader"),
        LoaderKind::Forge => Some("forge"),
        LoaderKind::NeoForge => Some("neoforge"),
        LoaderKind::LegacyFabric | LoaderKind::Optifine => {
            return Err(CoreError::Friends("library.packLoader"));
        }
    };
    if let Some(key) = key {
        let saved = inst
            .loader
            .version
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let version = match saved {
            Some(v) => v.to_owned(),
            None => loader::list_versions(ctx, inst.loader.kind, &inst.mc_version)
                .await?
                .into_iter()
                .find(|v| v.recommended)
                .map(|v| v.id)
                .ok_or(CoreError::Friends("library.packLoader"))?,
        };
        // Modrinth writes Forge without the Minecraft prefix ("47.4.10").
        let version = match inst.loader.kind {
            LoaderKind::Forge => version
                .strip_prefix(&format!("{}-", inst.mc_version))
                .map(str::to_owned)
                .unwrap_or(version),
            _ => version,
        };
        deps.insert(key.into(), json!(version));
    }
    Ok(Value::Object(deps))
}

/// Files of `dir` (not recursive) ending in `ext`; `.disabled` ones are off.
fn linkable_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(ext))
        })
        .collect();
    files.sort();
    files
}

/// Code and programs in config folders (update caches and the like): not
/// needed to play, and the library scan would refuse the pack for them.
const CONFIG_SKIP: &[&str] = &[
    ".jar", ".class", ".exe", ".dll", ".so", ".dylib", ".bat", ".cmd", ".ps1", ".sh", ".vbs",
    ".msi", ".scr",
];

/// Every regular file under `dir` but `CONFIG_SKIP`, as (absolute, relative
/// to `base`).
fn walk(base: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let Ok(ty) = e.file_type() else { continue };
        if ty.is_dir() {
            walk(base, &path, out);
        } else if ty.is_file()
            && !CONFIG_SKIP.iter().any(|x| {
                e.file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(x)
            })
            && let Ok(rel) = path.strip_prefix(base)
        {
            out.push((path.clone(), rel.to_string_lossy().replace('\\', "/")));
        }
    }
}

/// The Modrinth file of `version` with this SHA-1, if it is on the CDN.
fn cdn_file<'a>(v: &'a modrinth::Version, sha1: &str) -> Option<&'a modrinth::VersionFile> {
    v.files.iter().find(|f| {
        f.hashes.sha1.as_deref() == Some(sha1)
            && f.hashes.sha512.is_some()
            && f.url.starts_with(MODRINTH_CDN)
    })
}

/// Writes `<dest>` (a `.mrpack`) for the instance in `dir`; stops with
/// `library.packTooLarge` once the pack would be over `max_bytes`.
pub async fn export_mrpack(
    ctx: &Ctx,
    inst: &Instance,
    dir: &Path,
    dest: &Path,
    max_bytes: u64,
) -> Result<PackExport> {
    let deps = dependencies(ctx, inst).await?;
    let mut candidates = Vec::new();
    for (folder, ext) in LINKABLE {
        for path in linkable_files(&dir.join(folder), ext) {
            let sha1 = crate::hash::sha1_file(&path)?;
            candidates.push((path, format!("{folder}/"), sha1));
        }
    }
    let hashes: Vec<String> = candidates.iter().map(|c| c.2.clone()).collect();
    let known = modrinth::versions_by_sha1(ctx, &hashes).await?;

    let mut files = Vec::new();
    let mut embed: Vec<(PathBuf, String)> = Vec::new();
    for (path, folder, sha1) in &candidates {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel = format!("{folder}{name}");
        match known.get(sha1).and_then(|v| cdn_file(v, sha1)) {
            Some(f) => {
                let size = std::fs::metadata(path)
                    .map(|m| m.len())
                    .unwrap_or(f.size.unwrap_or(0));
                files.push(json!({
                    "path": rel,
                    "hashes": { "sha1": sha1, "sha512": f.hashes.sha512 },
                    "downloads": [f.url],
                    "fileSize": size,
                }));
            }
            None => embed.push((path.clone(), rel)),
        }
    }
    for d in CONFIG_DIRS {
        walk(dir, &dir.join(d), &mut embed);
    }
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1",
        "name": inst.name,
        "files": files,
        "dependencies": deps,
    });

    let (linked, embedded) = (files.len(), embed.len());
    let dest_owned = dest.to_owned();
    let index_bytes = serde_json::to_vec_pretty(&index).expect("json serializes");
    tokio::task::spawn_blocking(move || write_pack(&dest_owned, &index_bytes, &embed, max_bytes))
        .await
        .map_err(|e| CoreError::io(dest, io::Error::other(e.to_string())))??;
    tracing::info!(id = %inst.id, linked, embedded, "instance packed as mrpack");
    Ok(PackExport {
        path: dest.to_owned(),
        linked,
        embedded,
    })
}

fn write_pack(dest: &Path, index: &[u8], embed: &[(PathBuf, String)], max: u64) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    let tmp = dest.with_extension("mrpack.part");
    let result = (|| -> Result<()> {
        let file = File::create(&tmp).map_err(|e| CoreError::io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let zerr = |source| CoreError::Archive {
            path: tmp.clone(),
            source,
        };
        zip.start_file("modrinth.index.json", opts).map_err(zerr)?;
        zip.write_all(index).map_err(|e| CoreError::io(&tmp, e))?;
        // Uncompressed sizes are an upper bound for the check below.
        let mut total = index.len() as u64;
        for (path, rel) in embed {
            total += std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            if total > max {
                return Err(CoreError::Friends("library.packTooLarge"));
            }
            zip.start_file(format!("overrides/{rel}"), opts)
                .map_err(zerr)?;
            let mut f = File::open(path).map_err(|e| CoreError::io(path, e))?;
            io::copy(&mut f, &mut zip).map_err(|e| CoreError::io(path, e))?;
        }
        zip.finish().map_err(zerr)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest).map_err(|e| CoreError::io(dest, e))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};
    use std::sync::Arc;

    use serde_json::json;
    use sha2::Digest;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::content::scan::tests::{fabric_json, jar};
    use crate::content::scan::{LibraryKind, Verdict, scan_as};
    use crate::events::NullSink;
    use crate::net::Http;
    use crate::net::allowlist::Allowlist;
    use crate::paths::Paths;

    fn sha1(b: &[u8]) -> String {
        hex::encode(sha1::Sha1::digest(b))
    }

    fn instance(kind: &str, version: Option<&str>) -> Instance {
        serde_json::from_value(json!({
            "id": "golem", "name": "Iron Golem", "mcVersion": "1.21.4",
            "loader": { "kind": kind, "version": version },
        }))
        .unwrap()
    }

    async fn ctx(server: &MockServer, tmp: &Path) -> Ctx {
        let paths = Paths::at(tmp.join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.modrinth = server.uri();
        ctx
    }

    fn entries(pack: &Path) -> (Value, Vec<String>) {
        let bytes = std::fs::read(pack).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut index = String::new();
        zip.by_name("modrinth.index.json")
            .unwrap()
            .read_to_string(&mut index)
            .unwrap();
        let mut names: Vec<String> = zip.file_names().map(str::to_owned).collect();
        names.sort();
        (serde_json::from_str(&index).unwrap(), names)
    }

    #[tokio::test]
    async fn links_modrinth_files_embeds_the_rest_and_leaves_personal_files_out() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx(&server, tmp.path()).await;
        let dir = tmp.path().join("inst");
        for d in [
            "mods",
            "resourcepacks",
            "shaderpacks",
            "config/sub",
            "saves/w",
            "logs",
        ] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        let sodium = b"sodium bytes".to_vec();
        let own = jar(&[("fabric.mod.json", fabric_json())]);
        std::fs::write(dir.join("mods/sodium.jar"), &sodium).unwrap();
        std::fs::write(dir.join("mods/own.jar"), &own).unwrap();
        std::fs::write(dir.join("mods/off.jar.disabled"), b"off").unwrap();
        std::fs::write(
            dir.join("resourcepacks/rp.zip"),
            jar(&[("pack.mcmeta", b"{}".to_vec())]),
        )
        .unwrap();
        std::fs::write(dir.join("config/sub/a.toml"), b"x = 1").unwrap();
        std::fs::write(dir.join("config/sub/update.jar"), b"cache").unwrap();
        std::fs::write(dir.join("saves/w/level.dat"), b"world").unwrap();
        std::fs::write(dir.join("logs/latest.log"), b"log").unwrap();
        std::fs::write(dir.join("options.txt"), b"keys").unwrap();
        std::fs::write(dir.join("servers.dat"), b"servers").unwrap();
        let sha = sha1(&sodium);
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                sha.clone(): {
                    "id": "v1", "project_id": "AANobbMI", "version_number": "0.6",
                    "version_type": "release",
                    "files": [{
                        "url": "https://cdn.modrinth.com/data/AANobbMI/versions/v1/sodium.jar",
                        "filename": "sodium.jar", "primary": true, "size": 12,
                        "hashes": { "sha1": sha, "sha512": "ab".repeat(64) }
                    }]
                }
            })))
            .mount(&server)
            .await;

        let dest = tmp.path().join("out/Iron Golem.mrpack");
        let out = export_mrpack(
            &ctx,
            &instance("fabric", Some("0.16.9")),
            &dir,
            &dest,
            50 << 20,
        )
        .await
        .unwrap();
        assert_eq!((out.linked, out.embedded), (1, 3));
        let (index, names) = entries(&dest);
        assert_eq!(
            names,
            [
                "modrinth.index.json",
                "overrides/config/sub/a.toml",
                "overrides/mods/own.jar",
                "overrides/resourcepacks/rp.zip",
            ]
        );
        assert_eq!(index["name"], "Iron Golem");
        assert_eq!(
            index["dependencies"],
            json!({ "minecraft": "1.21.4", "fabric-loader": "0.16.9" })
        );
        assert_eq!(index["files"][0]["path"], "mods/sodium.jar");
        assert_eq!(index["files"][0]["fileSize"], 12);

        // The library accepts it as a modpack; the embedded jar got the mod rules.
        let report = scan_as(&std::fs::read(&dest).unwrap(), LibraryKind::Modpack);
        assert_ne!(report.verdict, Verdict::Block, "{:?}", report.findings);
        assert_eq!(report.descriptor.unwrap().name, "Iron Golem");
    }

    #[tokio::test]
    async fn refuses_loaders_a_pack_cannot_describe_and_oversized_packs() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx(&server, tmp.path()).await;
        let dir = tmp.path().join("inst");
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::write(dir.join("mods/big.jar"), vec![7u8; 4096]).unwrap();
        let dest = tmp.path().join("p.mrpack");
        for inst in [
            instance("legacyFabric", Some("0.15")),
            instance("optifine", Some("HD_U_J1")),
        ] {
            let e = export_mrpack(&ctx, &inst, &dir, &dest, 50 << 20)
                .await
                .unwrap_err();
            assert_eq!(e.code(), "library.packLoader");
        }
        // Forge is written the Modrinth way, without the Minecraft prefix.
        export_mrpack(
            &ctx,
            &instance("forge", Some("1.21.4-54.0.1")),
            &dir,
            &dest,
            50 << 20,
        )
        .await
        .unwrap();
        assert_eq!(
            entries(&dest).0["dependencies"],
            json!({ "minecraft": "1.21.4", "forge": "54.0.1" })
        );
        std::fs::remove_file(&dest).unwrap();
        let e = export_mrpack(&ctx, &instance("vanilla", None), &dir, &dest, 1024)
            .await
            .unwrap_err();
        assert_eq!(e.code(), "library.packTooLarge");
        assert!(!dest.exists());
        assert!(!dest.with_extension("mrpack.part").exists());
    }

    /// Real instance + real Modrinth (read only; the pack goes to a temp
    /// folder): `MEHBUR_INSTANCE=<instance dir> cargo test -p launcher-core
    /// --lib real_instance -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn real_instance() {
        let dir = PathBuf::from(std::env::var("MEHBUR_INSTANCE").expect("MEHBUR_INSTANCE"));
        let inst: Instance =
            serde_json::from_slice(&std::fs::read(dir.join("instance.json")).unwrap()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        let dest = tmp.path().join("real.mrpack");
        let out = export_mrpack(&ctx, &inst, &dir, &dest, 50 << 20)
            .await
            .unwrap();
        let bytes = std::fs::read(&dest).unwrap();
        let report = scan_as(&bytes, LibraryKind::Modpack);
        let (_, names) = entries(&dest);
        println!(
            "{} ({} {:?}): linked {}, embedded {}, {} bytes, verdict {:?}",
            inst.name,
            inst.mc_version,
            inst.loader,
            out.linked,
            out.embedded,
            bytes.len(),
            report.verdict
        );
        for f in &report.findings {
            println!("  {:?} {} {:?}", f.severity, f.code, f.examples);
        }
        for n in names.iter().filter(|n| !n.starts_with("overrides/config/")) {
            println!("  {n}");
        }
    }
}
