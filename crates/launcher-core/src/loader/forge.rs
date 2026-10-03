//! Forge and NeoForge.
//!
//! Both ship an installer jar. Three generations are handled:
//! - **legacy** (≤ 1.12.2 early builds): `install_profile.json` with
//!   `install` + `versionInfo`; the universal jar sits at the installer root.
//! - **spec 0 with version.json** (late 1.12.2): `version.json` plus a
//!   `maven/` folder, no processors.
//! - **spec 1** (1.13+, NeoForge): `version.json`, installer libraries and
//!   processors that patch the vanilla client jar (see [`super::processors`]).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::LoaderVersion;
use super::processors::{self, Processor, SidedValue};
use crate::archive;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::hash::Checksum;
use crate::java;
use crate::launch::ensure_client;
use crate::library;
use crate::maven::Coordinate;
use crate::net::cache::get_json_cached;
use crate::net::download::{DownloadItem, Verify};
use crate::rules::RuleEnv;
use crate::version::profile::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Forge,
    NeoForge,
}

impl Flavor {
    fn name(self) -> &'static str {
        match self {
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }
}

// ---------------------------------------------------------------- listing

#[derive(Debug, Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

pub async fn list(ctx: &Ctx, flavor: Flavor, mc: &str) -> Result<Vec<LoaderVersion>> {
    let cache = ctx.paths.cache().join("loaders");
    match flavor {
        Flavor::Forge => {
            let base = &ctx.endpoints.forge_files;
            let index: HashMap<String, Vec<String>> = get_json_cached(
                &ctx.http,
                &format!("{base}/net/minecraftforge/forge/maven-metadata.json"),
                &cache.join("forge-index.json"),
                super::LIST_TTL,
            )
            .await?;
            // Promotions only mark versions; the list works without them.
            let promos = get_json_cached::<Promotions>(
                &ctx.http,
                &format!("{base}/net/minecraftforge/forge/promotions_slim.json"),
                &cache.join("forge-promotions.json"),
                super::LIST_TTL,
            )
            .await
            .map(|p| p.promos)
            .unwrap_or_default();
            Ok(forge_versions(&index, &promos, mc))
        }
        Flavor::NeoForge => {
            let base = &ctx.endpoints.neoforge_maven;
            // 1.20.1 NeoForge was published as `net.neoforged:forge`.
            let artifact = if mc == "1.20.1" { "forge" } else { "neoforge" };
            let all: NeoVersions = get_json_cached(
                &ctx.http,
                &format!("{base}/api/maven/versions/releases/net/neoforged/{artifact}"),
                &cache.join(format!("neoforge-{artifact}.json")),
                super::LIST_TTL,
            )
            .await?;
            Ok(neoforge_versions(&all.versions, mc))
        }
    }
}

/// `1.20.1-47.4.26` → `47.4.26`; `1.7.10-10.13.4.1614-1.7.10` → `10.13.4.1614`.
pub fn forge_label(mc: &str, full: &str) -> String {
    let s = full.strip_prefix(&format!("{mc}-")).unwrap_or(full);
    s.strip_suffix(&format!("-{mc}")).unwrap_or(s).to_owned()
}

fn forge_versions(
    index: &HashMap<String, Vec<String>>,
    promos: &HashMap<String, String>,
    mc: &str,
) -> Vec<LoaderVersion> {
    let Some(list) = index.get(mc) else {
        return vec![];
    };
    let recommended = promos
        .get(&format!("{mc}-recommended"))
        .or_else(|| promos.get(&format!("{mc}-latest")));
    let mut out: Vec<LoaderVersion> = list
        .iter()
        .rev()
        .map(|full| {
            let label = forge_label(mc, full);
            LoaderVersion {
                recommended: recommended == Some(&label),
                stable: true,
                id: full.clone(),
                label,
            }
        })
        .collect();
    out.sort_by(|a, b| super::compare_loader_versions(&b.label, &a.label));
    out
}

/// Minecraft version a NeoForge version belongs to:
/// `20.4.237` → `1.20.4`, `21.0.167` → `1.21`, `26.3.0.46-beta` → `26.3`,
/// `26.1.2.10` → `26.1.2`.
pub fn neoforge_mc(v: &str) -> Option<String> {
    let core = v.split(['-', '+']).next()?;
    let parts = core
        .split('.')
        .map(|p| p.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    match parts.as_slice() {
        [major, minor, _] if (20..26).contains(major) => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        [major, minor, patch, _] if *major >= 26 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        _ => None,
    }
}

fn neoforge_versions(all: &[String], mc: &str) -> Vec<LoaderVersion> {
    let mut out: Vec<LoaderVersion> = all
        .iter()
        .filter(|v| {
            if mc == "1.20.1" {
                v.starts_with("1.20.1-")
            } else {
                neoforge_mc(v).as_deref() == Some(mc)
            }
        })
        .map(|v| LoaderVersion {
            id: v.clone(),
            label: forge_label(mc, v),
            stable: !(v.contains("beta") || v.contains("alpha")),
            recommended: false,
        })
        .collect();
    out.sort_by(|a, b| super::compare_loader_versions(&b.label, &a.label));
    out
}

// ---------------------------------------------------------------- install

#[derive(Debug, Deserialize)]
struct InstallProfile {
    #[serde(default)]
    data: HashMap<String, SidedValue>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    json: Option<String>,
}

fn installer_url(ctx: &Ctx, flavor: Flavor, mc: &str, v: &str) -> String {
    match flavor {
        Flavor::Forge => format!(
            "{}/net/minecraftforge/forge/{v}/forge-{v}-installer.jar",
            ctx.endpoints.forge_maven
        ),
        Flavor::NeoForge if mc == "1.20.1" => format!(
            "{}/releases/net/neoforged/forge/{v}/forge-{v}-installer.jar",
            ctx.endpoints.neoforge_maven
        ),
        Flavor::NeoForge => format!(
            "{}/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
            ctx.endpoints.neoforge_maven
        ),
    }
}

/// Downloads (or reuses) the installer jar, verified against the Maven
/// `.sha1` side file when the repository has one.
async fn fetch_installer(
    ctx: &Ctx,
    flavor: Flavor,
    mc: &str,
    v: &str,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<std::path::PathBuf> {
    let url = installer_url(ctx, flavor, mc, v);
    let dir = match flavor {
        Flavor::Forge => "forge",
        Flavor::NeoForge => "neoforge",
    };
    let dest = ctx
        .paths
        .loaders()
        .join(dir)
        .join(url.rsplit('/').next().unwrap_or("installer.jar"));
    let sha1 = match ctx.http.get_text(&format!("{url}.sha1")).await {
        Ok(t) => {
            let t = t.split_whitespace().next().unwrap_or_default().to_owned();
            (t.len() == 40 && t.chars().all(|c| c.is_ascii_hexdigit())).then_some(t)
        }
        Err(e) if e.code() == "net.httpStatus" => None,
        // Offline with a cached installer: use it as is.
        Err(_) if dest.is_file() => None,
        Err(e) => return Err(e),
    };
    ctx.downloader()
        .run(
            vec![DownloadItem {
                url,
                dest: dest.clone(),
                checksum: sha1.map(Checksum::Sha1),
                size: None,
            }],
            Verify::Full,
            progress,
            cancel,
        )
        .await
        .map_err(|e| match e {
            CoreError::HttpStatus { status: 404, .. } => CoreError::LoaderUnavailable {
                loader: flavor.name().into(),
                mc: mc.to_owned(),
            },
            other => other,
        })?;
    Ok(dest)
}

fn read_json(installer: &Path, entry: &str) -> Result<serde_json::Value> {
    let bytes = archive::read_entry(installer, entry)?.ok_or_else(|| CoreError::LoaderInstall {
        loader: "Forge".into(),
        reason: format!("{entry} is missing from the installer"),
    })?;
    serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
        path: installer.join(entry),
        source,
    })
}

pub async fn install(
    ctx: &Ctx,
    flavor: Flavor,
    mc: &str,
    loader_version: &str,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<serde_json::Value> {
    let installer = fetch_installer(ctx, flavor, mc, loader_version, progress, cancel).await?;
    let profile_raw = read_json(&installer, "install_profile.json")?;
    if profile_raw.get("versionInfo").is_some() {
        return install_legacy(ctx, flavor, &installer, profile_raw, progress, cancel).await;
    }
    let profile: InstallProfile =
        serde_json::from_value(profile_raw).map_err(|source| CoreError::Json {
            path: installer.join("install_profile.json"),
            source,
        })?;
    let version_entry = profile
        .json
        .as_deref()
        .unwrap_or("/version.json")
        .trim_start_matches('/')
        .to_owned();
    let version = read_json(&installer, &version_entry)?;
    let version_libs: Vec<Library> = version
        .get("libraries")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|source| CoreError::Json {
            path: installer.join(&version_entry),
            source,
        })?
        .unwrap_or_default();

    // 1. libraries: bundled in the installer's maven/ folder, downloaded, or
    //    generated by a processor (empty URL).
    let libraries_dir = ctx.paths.libraries();
    let names: HashSet<String> = archive::entry_names(&installer)?.into_iter().collect();
    let mut seen = HashSet::new();
    let mut downloads = Vec::new();
    let mut generated = Vec::new();
    let env = RuleEnv::new(ctx.os.clone());
    for lib in profile.libraries.iter().chain(&version_libs) {
        if !crate::rules::allowed(&lib.rules, &env) {
            continue;
        }
        let Some((rel, art)) = library::plain_artifact(lib, &libraries_dir) else {
            continue;
        };
        if !seen.insert(art.path.clone()) {
            continue;
        }
        let bundled = format!("maven/{rel}");
        if names.contains(&bundled) {
            if !art.path.is_file() {
                archive::extract_entry(&installer, &bundled, &art.path)?;
            }
        } else if let Some(item) = art.download_item() {
            downloads.push(item);
        } else {
            generated.push(art.path);
        }
    }
    ctx.downloader()
        .run(downloads, Verify::Quick, progress, cancel)
        .await?;

    // 2. processors (patch the vanilla jar).
    let client_processors = profile.processors.iter().any(Processor::runs_on_client);
    if client_processors {
        let (vanilla, client_jar) = ensure_client(ctx, mc, progress, cancel).await?;
        let java = java::resolve(ctx, vanilla.json.java_major(), None, true, cancel).await?;
        let work = ctx.paths.loaders().join("work").join(format!(
            "{}-{loader_version}",
            flavor.name().to_ascii_lowercase()
        ));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| CoreError::io(&work, e))?;
        let path = |p: &Path| p.display().to_string();
        let builtins = [
            ("SIDE", "client".to_owned()),
            ("MINECRAFT_JAR", path(&client_jar)),
            ("MINECRAFT_VERSION", mc.to_owned()),
            ("ROOT", path(ctx.paths.content())),
            ("INSTALLER", path(&installer)),
            ("LIBRARY_DIR", path(&libraries_dir)),
        ];
        let data = processors::resolve_data(
            &profile.data,
            &builtins,
            flavor.name(),
            &libraries_dir,
            &installer,
            &work,
        )?;
        let penv = processors::Env {
            loader: flavor.name(),
            libraries: &libraries_dir,
            work_dir: &work,
            java: &java,
            data,
        };
        let result = processors::run_all(&profile.processors, &penv, progress, cancel).await;
        let _ = std::fs::remove_dir_all(&work);
        result?;
    }

    if let Some(missing) = generated.iter().find(|p| !p.is_file()) {
        return Err(CoreError::LoaderInstall {
            loader: flavor.name().into(),
            reason: format!("installer did not produce {}", missing.display()),
        });
    }
    Ok(version)
}

/// Old installers: `{ install: {path, filePath}, versionInfo: {…} }`.
async fn install_legacy(
    ctx: &Ctx,
    flavor: Flavor,
    installer: &Path,
    profile: serde_json::Value,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<serde_json::Value> {
    let bad = |reason: &str| CoreError::LoaderInstall {
        loader: flavor.name().into(),
        reason: reason.into(),
    };
    let install = profile
        .get("install")
        .ok_or_else(|| bad("install section missing"))?;
    let coord_str = install
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("install.path missing"))?;
    let file_path = install
        .get("filePath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("install.filePath missing"))?;
    let coord = Coordinate::parse(coord_str).ok_or_else(|| bad("install.path is invalid"))?;
    let rel = coord.path();
    let dest = processors::library_path(&ctx.paths.libraries(), coord_str)
        .ok_or_else(|| bad("install.path is invalid"))?;
    if !dest.is_file() && !archive::extract_entry(installer, file_path, &dest)? {
        return Err(bad("universal jar is missing from the installer"));
    }

    let mut version = profile
        .get("versionInfo")
        .cloned()
        .ok_or_else(|| bad("versionInfo missing"))?;
    let libs = version
        .get("libraries")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    version["libraries"] = legacy_libraries(libs, coord_str, &rel).into();

    let parsed: Vec<Library> =
        serde_json::from_value(version["libraries"].clone()).map_err(|source| CoreError::Json {
            path: installer.join("install_profile.json"),
            source,
        })?;
    let env = RuleEnv::new(ctx.os.clone());
    let items = library::resolve(&parsed, &env, &ctx.paths.libraries()).download_items();
    ctx.downloader()
        .run(items, Verify::Quick, progress, cancel)
        .await?;
    Ok(version)
}

/// Normalises `versionInfo.libraries`: drops server-only entries, points the
/// Forge jar at the extracted file and moves the old Forge Maven to HTTPS.
fn legacy_libraries(
    libs: Vec<serde_json::Value>,
    forge_coord: &str,
    forge_rel: &str,
) -> Vec<serde_json::Value> {
    libs.into_iter()
        .filter(|l| l.get("clientreq").and_then(|v| v.as_bool()) != Some(false))
        .map(|mut l| {
            if let Some(obj) = l.as_object_mut() {
                obj.remove("checksums");
                obj.remove("serverreq");
                obj.remove("clientreq");
                if obj.get("name").and_then(|n| n.as_str()) == Some(forge_coord) {
                    obj.remove("url");
                    obj.insert(
                        "downloads".into(),
                        serde_json::json!({"artifact": {"path": forge_rel, "url": ""}}),
                    );
                } else if let Some(url) = obj.get("url").and_then(|u| u.as_str()) {
                    let fixed = url
                        .replace(
                            "http://files.minecraftforge.net/maven",
                            "https://maven.minecraftforge.net",
                        )
                        .replace(
                            "https://files.minecraftforge.net/maven",
                            "https://maven.minecraftforge.net",
                        );
                    obj.insert("url".into(), fixed.into());
                }
            }
            l
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_mapping() {
        assert_eq!(neoforge_mc("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_mc("20.2.3-beta").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_mc("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_mc("21.1.253").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc("21.11.4-beta").as_deref(), Some("1.21.11"));
        assert_eq!(neoforge_mc("26.3.0.46-beta").as_deref(), Some("26.3"));
        assert_eq!(neoforge_mc("26.1.2.10").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_mc("0.25w14craftmine.3-beta"), None);
        assert_eq!(neoforge_mc("47.1.82"), None);
    }

    #[test]
    fn neoforge_filter_and_order() {
        let all: Vec<String> = [
            "21.1.252",
            "26.3.0.45-beta",
            "26.3.0.46-beta",
            "26.3.0.9-beta",
            "26.3.1.2",
            "1.20.1-47.1.105",
            "47.1.82",
        ]
        .map(String::from)
        .to_vec();
        let v = neoforge_versions(&all, "26.3");
        let ids: Vec<_> = v.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["26.3.0.46-beta", "26.3.0.45-beta", "26.3.0.9-beta"]);
        assert!(v.iter().all(|x| !x.stable));
        let legacy = neoforge_versions(&all, "1.20.1");
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].label, "47.1.105");
    }

    #[test]
    fn forge_list_uses_promotions() {
        let index: HashMap<String, Vec<String>> = serde_json::from_str(
            r#"{"1.20.1":["1.20.1-47.4.9","1.20.1-47.4.10","1.20.1-47.4.26"],
                "1.7.10":["1.7.10-10.13.4.1614-1.7.10"]}"#,
        )
        .unwrap();
        let promos: HashMap<String, String> =
            serde_json::from_str(r#"{"1.20.1-recommended":"47.4.10","1.20.1-latest":"47.4.26"}"#)
                .unwrap();
        let v = forge_versions(&index, &promos, "1.20.1");
        assert_eq!(v[0].id, "1.20.1-47.4.26");
        assert_eq!(v[0].label, "47.4.26");
        assert!(v.iter().find(|x| x.label == "47.4.10").unwrap().recommended);
        assert_eq!(v.iter().filter(|x| x.recommended).count(), 1);

        let old = forge_versions(&index, &HashMap::new(), "1.7.10");
        assert_eq!(old[0].label, "10.13.4.1614");
        assert!(forge_versions(&index, &promos, "26.3").is_empty());
    }

    #[test]
    fn legacy_library_normalisation() {
        let libs: Vec<serde_json::Value> = serde_json::from_str(
            r#"[{"name":"net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10","url":"https://maven.minecraftforge.net/"},
                {"name":"net.minecraft:launchwrapper:1.12","serverreq":true},
                {"name":"com.typesafe:config:1.2.1","url":"http://files.minecraftforge.net/maven/","checksums":["a"],"serverreq":true,"clientreq":true},
                {"name":"server:only:1","clientreq":false}]"#,
        )
        .unwrap();
        let out = legacy_libraries(
            libs,
            "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
            "net/minecraftforge/forge/x/forge-x.jar",
        );
        assert_eq!(out.len(), 3);
        assert_eq!(out[0]["downloads"]["artifact"]["url"], "");
        assert!(out[0].get("url").is_none());
        assert_eq!(out[2]["url"], "https://maven.minecraftforge.net/");
        assert!(out[2].get("checksums").is_none());
        let parsed: Vec<Library> = serde_json::from_value(out.into()).unwrap();
        assert_eq!(parsed.len(), 3);
    }
}
