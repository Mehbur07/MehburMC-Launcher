//! Downloads Eclipse Temurin JREs from the Adoptium API into `runtime/java<N>`.
//!
//! `GET /v3/assets/latest/{major}/hotspot?os=&architecture=&image_type=jre`
//! returns `[{ binary: { package: { link, checksum (sha256), name, size } } }]`.

use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::{JavaInstall, managed_home, read_release};
use crate::archive::extract_zip;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{Progress, Stage};
use crate::hash::Checksum;
use crate::net::download::{DownloadItem, Verify};

#[derive(Debug, Deserialize)]
struct Asset {
    binary: Binary,
}

#[derive(Debug, Deserialize)]
struct Binary {
    package: Package,
}

#[derive(Debug, Deserialize)]
struct Package {
    link: String,
    #[serde(default)]
    checksum: Option<String>,
    name: String,
    #[serde(default)]
    size: Option<u64>,
}

fn adoptium_os(os: &str) -> &'static str {
    match os {
        "windows" => "windows",
        "osx" => "mac",
        _ => "linux",
    }
}

fn adoptium_arch(arch: &str) -> &'static str {
    match arch {
        "arm64" => "aarch64",
        "x86" => "x86-32",
        "arm32" => "arm",
        _ => "x64",
    }
}

async fn find_package(ctx: &Ctx, major: u32) -> Result<Package> {
    let os = adoptium_os(ctx.os.name);
    let native_arch = adoptium_arch(ctx.os.arch);
    // Prefer a native JRE, then a native JDK; on arm64 fall back to x64
    // (e.g. there is no Java 8 for Windows on ARM; it runs emulated).
    let mut candidates = vec![(native_arch, "jre"), (native_arch, "jdk")];
    if native_arch == "aarch64" {
        candidates.extend([("x64", "jre"), ("x64", "jdk")]);
    }
    for (arch, image) in candidates {
        let url = format!(
            "{}/v3/assets/latest/{major}/hotspot?os={os}&architecture={arch}&image_type={image}&vendor=eclipse",
            ctx.endpoints.adoptium
        );
        let assets: Vec<Asset> = ctx.http.get_json(&url).await?;
        if let Some(a) = assets.into_iter().next() {
            return Ok(a.binary.package);
        }
    }
    Err(CoreError::JavaUnavailable {
        major,
        os: os.into(),
        arch: native_arch.into(),
    })
}

pub async fn install(ctx: &Ctx, major: u32, cancel: &CancellationToken) -> Result<JavaInstall> {
    let pkg = find_package(ctx, major).await?;
    if !pkg.name.ends_with(".zip") {
        // tar.gz runtimes (Linux/macOS) are not handled yet.
        return Err(CoreError::JavaUnavailable {
            major,
            os: ctx.os.name.into(),
            arch: ctx.os.arch.into(),
        });
    }
    tracing::info!(major, package = %pkg.name, "installing Java runtime");

    let runtime = ctx.paths.runtime();
    let archive = runtime.join(".downloads").join(&pkg.name);
    let progress = Progress::new(ctx.events.clone(), &format!("java{major}"), Stage::Java);
    ctx.downloader()
        .run(
            vec![DownloadItem {
                url: pkg.link.clone(),
                dest: archive.clone(),
                checksum: pkg.checksum.clone().map(Checksum::Sha256),
                size: pkg.size,
            }],
            Verify::Full,
            &progress,
            cancel,
        )
        .await?;

    let final_dir = managed_home(ctx, major);
    let staging = runtime.join(format!(".java{major}.staging"));
    let _ = std::fs::remove_dir_all(&staging);
    let (archive2, staging2) = (archive.clone(), staging.clone());
    // Strip the archive's top-level folder (`jdk-25.0.4.1+1-jre/…`).
    tokio::task::spawn_blocking(move || {
        extract_zip(&archive2, &staging2, |name| {
            name.split_once('/')
                .map(|(_, rest)| rest.to_owned())
                .filter(|r| !r.is_empty())
        })
    })
    .await
    .expect("extract task panicked")?;

    if read_release(&staging, true).is_none() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(CoreError::JavaNotFound { major });
    }
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&staging, &final_dir).map_err(|e| CoreError::io(&final_dir, e))?;
    let _ = std::fs::remove_file(&archive);

    let install = read_release(&final_dir, true).ok_or(CoreError::JavaNotFound { major })?;
    tracing::info!(major, version = %install.version, "Java runtime ready");
    Ok(install)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_platform_names() {
        assert_eq!(adoptium_os("osx"), "mac");
        assert_eq!(adoptium_arch("x86_64"), "x64");
        assert_eq!(adoptium_arch("arm64"), "aarch64");
    }
}
