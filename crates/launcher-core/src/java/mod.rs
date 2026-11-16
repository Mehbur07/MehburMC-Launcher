//! Java runtime discovery and selection.
//!
//! Managed runtimes live in `runtime/java<major>/` and are downloaded from
//! Adoptium on demand. System installations are discovered from `JAVA_HOME`,
//! `PATH` and the usual vendor folders by reading each JDK's `release` file
//! (no process spawn needed).

pub mod adoptium;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JavaInstall {
    /// The `java` executable.
    pub executable: String,
    pub home: String,
    pub major: u32,
    pub version: String,
    pub vendor: Option<String>,
    pub arch: Option<String>,
    /// Downloaded and owned by the launcher.
    pub managed: bool,
}

impl JavaInstall {
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(&self.executable)
    }
}

/// `1.8.0_412` → 8, `17.0.11` → 17, `25` → 25, `21-ea` → 21.
pub fn parse_major(version: &str) -> Option<u32> {
    let v = version.trim().trim_matches('"');
    let mut parts = v.split(|c: char| !c.is_ascii_digit());
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

pub fn executable_in(home: &Path) -> PathBuf {
    let name = if cfg!(windows) { "java.exe" } else { "java" };
    home.join("bin").join(name)
}

/// Mac'teki Java'lar bir uygulama paketi gibi durur: asıl Java klasörü
/// `<paket>/Contents/Home` içindedir (Adoptium arşivi ve
/// `/Library/Java/JavaVirtualMachines/*.jdk` böyle). Öyleyse onu döndürür,
/// değilse klasörün kendisini (K108).
pub fn bundle_home(dir: &Path) -> PathBuf {
    let inner = dir.join("Contents").join("Home");
    if inner.join("bin").is_dir() {
        inner
    } else {
        dir.to_path_buf()
    }
}

/// `OS_ARCH` değerini Mojang adlarına çevirir (`amd64` → `x86_64`, `aarch64` → `arm64`).
fn normalize_arch(arch: &str) -> &str {
    match arch {
        "amd64" | "x86_64" | "x64" => "x86_64",
        "aarch64" | "arm64" => "arm64",
        "i386" | "i586" | "i686" | "x86" => "x86",
        other => other,
    }
}

/// Bu Java bu oyun için uygun mimaride mi? Mac'te Intel Java ile arm64 yerel
/// dosyaları (ya da tersi) birlikte çalışmaz; Rosetta kararı (K108) mimariyi
/// belirler. Mimarisini yazmayan Java'lar ve başka sistemler kabul edilir.
fn arch_fits(java: &JavaInstall, os: &crate::os::OsInfo) -> bool {
    if os.name != "osx" {
        return true;
    }
    java.arch
        .as_deref()
        .is_none_or(|a| normalize_arch(a) == os.arch)
}

/// Reads `<home>/release` (`JAVA_VERSION="21.0.4"`, `IMPLEMENTOR=…`, `OS_ARCH=…`).
/// A macOS bundle (`<dir>/Contents/Home`) is accepted too.
pub fn read_release(home: &Path, managed: bool) -> Option<JavaInstall> {
    let home = &bundle_home(home);
    let exe = executable_in(home);
    if !exe.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_owned())
    };
    let version = field("JAVA_VERSION")?;
    Some(JavaInstall {
        executable: exe.display().to_string(),
        home: home.display().to_string(),
        major: parse_major(&version)?,
        version,
        vendor: field("IMPLEMENTOR"),
        arch: field("OS_ARCH"),
        managed,
    })
}

/// `runtime/java<major>`; Rosetta ile çalışan oyunların Intel Java'sı ayrı
/// durur: `runtime/java<major>-x64` (K108).
pub fn managed_home(ctx: &Ctx, major: u32) -> PathBuf {
    let suffix = if ctx.os.rosetta { "-x64" } else { "" };
    ctx.paths.runtime().join(format!("java{major}{suffix}"))
}

/// Runtimes under `runtime/`.
pub fn managed(ctx: &Ctx) -> Vec<JavaInstall> {
    let Ok(rd) = std::fs::read_dir(ctx.paths.runtime()) else {
        return vec![];
    };
    let mut out: Vec<_> = rd
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("java"))
        .filter_map(|e| read_release(&e.path(), true))
        .collect();
    out.sort_by_key(|j| j.major);
    out
}

/// Java installations found on the system (not managed by the launcher).
pub fn scan_system() -> Vec<JavaInstall> {
    let mut homes: Vec<PathBuf> = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        homes.push(PathBuf::from(h));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if executable_in(dir.parent().unwrap_or(&dir)).is_file()
                && dir.file_name().is_some_and(|n| n == "bin")
                && let Some(home) = dir.parent()
            {
                homes.push(home.to_path_buf());
            }
        }
    }
    for root in vendor_roots() {
        if let Ok(rd) = std::fs::read_dir(&root) {
            homes.extend(rd.flatten().map(|e| e.path()));
        }
    }

    let mut seen = HashSet::new();
    let mut out: Vec<_> = homes
        .into_iter()
        .filter_map(|h| std::fs::canonicalize(&h).ok().or(Some(h)))
        .filter(|h| seen.insert(h.clone()))
        .filter_map(|h| read_release(&h, false))
        .collect();
    out.sort_by(|a, b| b.major.cmp(&a.major).then(a.home.cmp(&b.home)));
    out
}

fn vendor_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Eclipse Foundation",
                    "Microsoft",
                    "Zulu",
                    "BellSoft",
                    "Amazon Corretto",
                    "Semeru",
                ] {
                    roots.push(base.join(vendor));
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
    }
    roots
}

/// Picks the runtime for `major`: explicit override → managed → system with
/// the exact major → download from Adoptium (if allowed).
pub async fn resolve(
    ctx: &Ctx,
    major: u32,
    override_exe: Option<&Path>,
    auto_download: bool,
    cancel: &CancellationToken,
) -> Result<JavaInstall> {
    if let Some(exe) = override_exe {
        // Defence in depth: instance.json may have been edited by hand.
        crate::instance::validate_java_path(&exe.display().to_string())?;
        let home = exe.parent().and_then(Path::parent);
        if let Some(found) = home.and_then(|h| read_release(h, false)) {
            if found.major != major {
                tracing::warn!(
                    want = major,
                    have = found.major,
                    "Java override has a different major version"
                );
            }
            return Ok(found);
        }
        if exe.is_file() {
            // No release file: trust the user's choice.
            return Ok(JavaInstall {
                executable: exe.display().to_string(),
                home: home.map(|h| h.display().to_string()).unwrap_or_default(),
                major,
                version: "unknown".into(),
                vendor: None,
                arch: None,
                managed: false,
            });
        }
        return Err(CoreError::JavaNotFound { major });
    }

    if let Some(j) = read_release(&managed_home(ctx, major), true)
        && arch_fits(&j, &ctx.os)
    {
        return Ok(j);
    }
    if let Some(j) = scan_system()
        .into_iter()
        .find(|j| j.major == major && arch_fits(j, &ctx.os))
    {
        return Ok(j);
    }
    if !auto_download {
        return Err(CoreError::JavaNotFound { major });
    }
    adoptium::install(ctx, major, cancel).await
}

/// Bir sürümü kuran yardımcı programlar (Forge işlemcileri, OptiFine
/// yamacısı) için Java: oyunun kendisiyle aynı seçim, Apple Silicon'daki
/// Rosetta kararı dahil (K108). Böylece aynı Java bir kez indirilir.
pub async fn resolve_for_version(
    ctx: &Ctx,
    version: &crate::version::VersionJson,
    cancel: &CancellationToken,
) -> Result<JavaInstall> {
    let game = Ctx {
        os: ctx.os.for_game(&version.libraries),
        ..ctx.clone()
    };
    resolve(&game, version.java_major(), None, true, cancel).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_parsing() {
        assert_eq!(parse_major("1.8.0_412"), Some(8));
        assert_eq!(parse_major("\"17.0.11\""), Some(17));
        assert_eq!(parse_major("25"), Some(25));
        assert_eq!(parse_major("21-ea"), Some(21));
        assert_eq!(parse_major("garbage"), None);
    }

    #[test]
    fn reads_release_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(executable_in(dir.path()), b"").unwrap();
        std::fs::write(
            dir.path().join("release"),
            "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"21.0.4\"\nOS_ARCH=\"amd64\"\n",
        )
        .unwrap();
        let j = read_release(dir.path(), true).unwrap();
        assert_eq!(j.major, 21);
        assert_eq!(j.vendor.as_deref(), Some("Eclipse Adoptium"));
        assert_eq!(j.arch.as_deref(), Some("amd64"));
        assert!(j.managed);
    }

    fn fake_java(home: &Path, version: &str, arch: &str) {
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(executable_in(home), b"").unwrap();
        std::fs::write(
            home.join("release"),
            format!("JAVA_VERSION=\"{version}\"\nOS_ARCH=\"{arch}\"\n"),
        )
        .unwrap();
    }

    #[test]
    fn reads_a_mac_bundle() {
        // Adoptium'un Mac arşivi (üst klasör atıldıktan sonra) ve *.jdk paketleri.
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("Contents").join("Home");
        fake_java(&home, "17.0.12", "aarch64");
        let j = read_release(dir.path(), true).unwrap();
        assert_eq!(j.major, 17);
        assert_eq!(PathBuf::from(&j.home), home);
        assert_eq!(j.executable_path(), executable_in(&home));
        // Düz klasör olduğu gibi kalır.
        assert_eq!(bundle_home(&home), home);
    }

    #[test]
    fn mac_java_must_match_the_game_arch() {
        let java = |arch: Option<&str>| JavaInstall {
            executable: String::new(),
            home: String::new(),
            major: 17,
            version: "17".into(),
            vendor: None,
            arch: arch.map(Into::into),
            managed: false,
        };
        let m1 = crate::os::OsInfo::fake("osx", "arm64");
        let rosetta = crate::os::OsInfo {
            arch: "x86_64",
            rosetta: true,
            ..m1.clone()
        };
        assert!(arch_fits(&java(Some("aarch64")), &m1));
        assert!(!arch_fits(&java(Some("x86_64")), &m1));
        assert!(arch_fits(&java(Some("x86_64")), &rosetta));
        assert!(!arch_fits(&java(Some("aarch64")), &rosetta));
        assert!(arch_fits(&java(None), &m1));
        // Windows'ta değişiklik yok (ARM'de Intel Java da çalışır).
        let win_arm = crate::os::OsInfo::fake("windows", "arm64");
        assert!(arch_fits(&java(Some("amd64")), &win_arm));
    }
}
