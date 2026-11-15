//! Telefonda Java (K104).
//!
//! Masaüstü JRE'leri Android'de çalışmaz: telefon için derlenmiş OpenJDK'lar
//! gerekir. Amethyst projesinin (AngelAuraMC) yayınladığı arm64 paketleri
//! kullanılır; adresler ve SHA-256 parmak izleri sabittir, böylece indirilen
//! dosya değişirse kurulum reddedilir. Paketler ilk oyunda indirilir ve
//! `runtime/android-java<N>/` altına açılır (masaüstü Java listesine girmez).

use std::path::{Path, PathBuf};

use tokio_util::sync::CancellationToken;

use super::{JavaInstall, read_release};
use crate::archive::extract_tar_xz;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{Progress, Stage};
use crate::hash::Checksum;
use crate::net::download::{DownloadItem, Verify};

/// Sabitlenmiş bir Android JRE paketi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AndroidJre {
    pub major: u32,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

const BASE: &str = "https://github.com/AngelAuraMC/angelauramc-openjdk-build/releases/download";

/// arm64 paketleri (angelauramc-openjdk-build, 2026-04-04 sürümleri).
pub const JRES: [AndroidJre; 4] = [
    AndroidJre {
        major: 8,
        url: "download_jre8/jre8-android-arm64.tar.xz",
        sha256: "9a59124d9791957d55c68be664ab76831f336cf2e1e1cd4414220c6fdbf0e06d",
        size: 28_026_736,
    },
    AndroidJre {
        major: 17,
        url: "download_jre17/jre17-android-arm64.tar.xz",
        sha256: "e162c860fe05ee4a4e4af7606437419879f6c748386a7b09fa77d10db6a64091",
        size: 26_900_804,
    },
    AndroidJre {
        major: 21,
        url: "download_jre21/jre21-android-arm64.tar.xz",
        sha256: "8d41ec401ee59f7722df60ed991f81ad146e130452804bfdd8a05d3436f7bbfe",
        size: 28_675_516,
    },
    AndroidJre {
        major: 25,
        url: "download_jre25/jre25-android-arm64.tar.xz",
        sha256: "d3eb7afe2240c26728a1bb440502c5f18ac3883e932d202dd7f0c9bcbbce4c37",
        size: 38_031_580,
    },
];

/// Sürümün istediği Java'yı karşılayan en küçük paket (16 → 17, 22 → 25).
pub fn pick(required: u32) -> Option<AndroidJre> {
    JRES.iter().copied().find(|j| j.major >= required)
}

pub fn home(ctx: &Ctx, major: u32) -> PathBuf {
    ctx.paths.runtime().join(format!("android-java{major}"))
}

/// Hangi paketten kurulduğunu tutan dosya: sabit paket değişirse yeniden kurulur.
const STAMP: &str = ".mehbur-jre";

fn installed(home: &Path, jre: &AndroidJre) -> Option<JavaInstall> {
    let stamp = std::fs::read_to_string(home.join(STAMP)).ok()?;
    if stamp.trim() != jre.sha256 {
        return None;
    }
    read_release(home, true)
}

/// Gerekirse indirir ve kurar; `task` ilerlemenin gösterileceği görevdir.
pub async fn ensure(
    ctx: &Ctx,
    required: u32,
    task: &str,
    cancel: &CancellationToken,
) -> Result<JavaInstall> {
    let jre = pick(required).ok_or(CoreError::JavaUnavailable {
        major: required,
        os: "android".into(),
        arch: ctx.os.arch.into(),
    })?;
    let final_dir = home(ctx, jre.major);
    if let Some(found) = installed(&final_dir, &jre) {
        return Ok(found);
    }
    tracing::info!(
        required,
        major = jre.major,
        "installing the Android Java runtime"
    );

    let runtime = ctx.paths.runtime();
    let archive = runtime
        .join(".downloads")
        .join(format!("android-jre{}.tar.xz", jre.major));
    let progress = Progress::new(ctx.events.clone(), task, Stage::Java);
    ctx.downloader()
        .run(
            vec![DownloadItem {
                url: format!("{BASE}/{}", jre.url),
                dest: archive.clone(),
                checksum: Some(Checksum::Sha256(jre.sha256.into())),
                size: Some(jre.size),
            }],
            Verify::Full,
            &progress,
            cancel,
        )
        .await?;

    let staging = runtime.join(format!(".android-java{}.staging", jre.major));
    let _ = std::fs::remove_dir_all(&staging);
    let (archive2, staging2) = (archive.clone(), staging.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Paket içi adlar `./bin/java` biçiminde.
        extract_tar_xz(&archive2, &staging2, |n| {
            let n = n.strip_prefix("./").unwrap_or(n);
            (!n.is_empty()).then(|| n.to_owned())
        })?;
        fix_freetype(&staging2)
    })
    .await
    .expect("extract task panicked")?;

    if read_release(&staging, true).is_none() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(CoreError::JavaNotFound { major: jre.major });
    }
    std::fs::write(staging.join(STAMP), jre.sha256).map_err(|e| CoreError::io(&staging, e))?;
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&staging, &final_dir).map_err(|e| CoreError::io(&final_dir, e))?;
    let _ = std::fs::remove_file(&archive);

    let install =
        read_release(&final_dir, true).ok_or(CoreError::JavaNotFound { major: jre.major })?;
    tracing::info!(major = jre.major, version = %install.version, "Android Java runtime ready");
    Ok(install)
}

/// Bazı paketlerde yazı tipi kütüphanesi `libfreetype.so.6` adıyla gelir; Java
/// onu `libfreetype.so` olarak arar (Amethyst'in `postPrepare` adımı).
fn fix_freetype(home: &Path) -> Result<()> {
    for dir in [home.join("lib"), home.join("lib").join("aarch64")] {
        let from = dir.join("libfreetype.so.6");
        let to = dir.join("libfreetype.so");
        if from.is_file() && !to.exists() {
            std::fs::rename(&from, &to).map_err(|e| CoreError::io(&to, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_smallest_runtime_that_fits() {
        assert_eq!(pick(8).unwrap().major, 8);
        assert_eq!(pick(16).unwrap().major, 17);
        assert_eq!(pick(17).unwrap().major, 17);
        assert_eq!(pick(21).unwrap().major, 21);
        assert_eq!(pick(22).unwrap().major, 25);
        assert_eq!(pick(25).unwrap().major, 25);
        assert!(pick(26).is_none());
    }

    #[test]
    fn pinned_packages_look_sane() {
        for j in JRES {
            assert_eq!(j.sha256.len(), 64);
            assert!(j.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(
                j.url
                    .ends_with(&format!("jre{}-android-arm64.tar.xz", j.major))
            );
        }
    }

    #[test]
    fn renames_freetype() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("libfreetype.so.6"), b"ft").unwrap();
        fix_freetype(dir.path()).unwrap();
        assert!(lib.join("libfreetype.so").is_file());
        assert!(!lib.join("libfreetype.so.6").exists());
    }

    #[test]
    fn stamp_guards_a_replaced_package() {
        let dir = tempfile::tempdir().unwrap();
        let h = dir.path();
        std::fs::create_dir_all(h.join("bin")).unwrap();
        std::fs::write(
            h.join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" }),
            b"x",
        )
        .unwrap();
        std::fs::write(h.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
        let jre = pick(21).unwrap();
        assert!(installed(h, &jre).is_none(), "no stamp yet");
        std::fs::write(h.join(STAMP), "0".repeat(64)).unwrap();
        assert!(installed(h, &jre).is_none(), "other package");
        std::fs::write(h.join(STAMP), jre.sha256).unwrap();
        assert_eq!(installed(h, &jre).unwrap().major, 21);
    }
}
