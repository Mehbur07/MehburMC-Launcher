//! Telefonda başlatma planı (K104).
//!
//! Oyun telefonda launcher'dan ayrı bir Android sürecinde, Amethyst'in oyun
//! motoruyla açılır. Rust tarafı dosyaları indirir ve bu planı hazırlar; Android
//! tarafı (`mcengine`) planı okuyup Java'yı başlatır. Masaüstünden farkları:
//!
//! - Mojang'ın LWJGL kütüphaneleri ve `natives` dosyaları telefonda çalışmaz;
//!   indirilmez. Yerlerine motorla gelen Android LWJGL'i kullanılır (sürümü plana
//!   yazılır: 3.4.1 ve sonrası `3.4.1`, öncesi `3.3.3`; LWJGL 2 için `lwjglx`).
//! - Motorla gelen JNA yerel kütüphanesi (libjnidispatch) 7.0.0 sürümünde; JNA
//!   5.14'ten eski jar'lar onu kabul etmez, bu yüzden 5.14.0 ile değiştirilir.
//!   Amethyst 5.13.0 kullanır ama o 6.x bekler (sürüm kontrolüne takılır).
//!   Ayrıca oshi 6.2 → 6.3, eski asm-all → 5.0.4 (Amethyst'teki gibi).
//! - Sınıf yolu, bellek, `java.library.path` ve log4j ayarı JVM argümanlarından
//!   ayrı alanlarda gelir; motor kendi değerleriyle birleştirir.

use std::path::Path;

use serde::Serialize;

use crate::library::{LibArtifact, ResolvedLibraries};
use crate::maven::Coordinate;

/// Grafik çevirmeni: masaüstü OpenGL çağrılarını telefonun GLES'ine çevirir.
pub const RENDERER_MOBILEGLUES: &str = "opengles_mobileglues";
pub const RENDERER_GL4ES: &str = "opengles2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogConfig {
    /// `client-1.12.xml` gibi; motor güvenlik yamalı karşılığını arar.
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidLaunch {
    pub version_id: String,
    pub java_home: String,
    pub java_major: u32,
    pub game_dir: String,
    pub natives_dir: String,
    /// Motorun LWJGL'i hariç (o önce eklenir).
    pub classpath: Vec<String>,
    /// Sürüm dosyasındaki ve profildeki JVM argümanları (sınıf yolu, bellek,
    /// `java.library.path` ve log4j hariç).
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
    pub memory_mb: u32,
    /// `3.3.3` veya `3.4.1`.
    pub lwjgl: String,
    /// LWJGL 2 sürümü: `lwjgl-lwjglx.jar` uyumluluk katmanı eklenir.
    pub lwjglx: bool,
    pub renderer: String,
    /// `arguments` bloklu (1.13+) sürüm: girdi kuyruğu yöntemi buna göre seçilir.
    pub modern_input: bool,
    pub log_config: Option<LogConfig>,
}

/// LWJGL sürümü, Amethyst'teki gibi rakamlar birleştirilerek (`3.3.3` → 333).
pub fn lwjgl_number(libraries: &[LibArtifact]) -> Option<u32> {
    libraries.iter().find_map(|a| {
        let version = a
            .name
            .strip_prefix("org.lwjgl:lwjgl:")
            .or_else(|| a.name.strip_prefix("org.lwjgl.lwjgl:lwjgl:"))?;
        let digits: String = version
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .filter(char::is_ascii_digit)
            .collect();
        digits.parse().ok().filter(|n| (200..=999).contains(n))
    })
}

pub fn lwjgl_component(number: u32) -> &'static str {
    if number >= 341 { "3.4.1" } else { "3.3.3" }
}

/// Telefonda kullanılmayan kütüphaneleri çıkarır, bazılarını değiştirir.
/// LWJGL sürümü bulunamazsa `None` (sürüm dosyası bozuk).
pub fn adapt_libraries(
    mut libs: ResolvedLibraries,
    libraries_dir: &Path,
) -> Option<(ResolvedLibraries, u32)> {
    let lwjgl = lwjgl_number(&libs.classpath)?;
    libs.natives.clear();
    libs.classpath.retain(|a| {
        let Some(c) = Coordinate::parse(&a.name) else {
            return true;
        };
        !c.group.starts_with("org.lwjgl")
            && !c
                .classifier
                .as_deref()
                .is_some_and(|x| x.starts_with("natives-"))
    });
    for a in &mut libs.classpath {
        replace(a, libraries_dir);
    }
    Some((libs, lwjgl))
}

struct Swap {
    name: &'static str,
    path: &'static str,
    sha1: &'static str,
    size: u64,
}

const JNA: Swap = Swap {
    name: "net.java.dev.jna:jna:5.14.0",
    path: "net/java/dev/jna/jna/5.14.0/jna-5.14.0.jar",
    sha1: "67bf3eaea4f0718cb376a181a629e5f88fa1c9dd",
    size: 1_878_533,
};
const OSHI: Swap = Swap {
    name: "com.github.oshi:oshi-core:6.3.0",
    path: "com/github/oshi/oshi-core/6.3.0/oshi-core-6.3.0.jar",
    sha1: "9e98cf55be371cafdb9c70c35d04ec2a8c2b42ac",
    size: 957_945,
};
const ASM_ALL: Swap = Swap {
    name: "org.ow2.asm:asm-all:5.0.4",
    path: "org/ow2/asm/asm-all/5.0.4/asm-all-5.0.4.jar",
    sha1: "e6244859997b3d4237a552669279780876228909",
    size: 241_810,
};

fn version_parts(v: &str) -> (u32, u32) {
    let mut it = v.split(|c: char| !c.is_ascii_digit());
    let major = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let minor = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (major, minor)
}

fn replace(a: &mut LibArtifact, root: &Path) {
    let Some(c) = Coordinate::parse(&a.name) else {
        return;
    };
    let (major, minor) = version_parts(&c.version);
    let swap = match (c.group.as_str(), c.artifact.as_str()) {
        ("net.java.dev.jna", "jna") if (major, minor) < (5, 14) => &JNA,
        ("com.github.oshi", "oshi-core") if (major, minor) == (6, 2) => &OSHI,
        ("org.ow2.asm", "asm-all") if major < 5 => &ASM_ALL,
        _ => return,
    };
    tracing::info!(from = %a.name, to = swap.name, "library replaced for Android");
    *a = LibArtifact {
        name: swap.name.into(),
        path: root.join(swap.path),
        url: Some(format!("https://repo1.maven.org/maven2/{}", swap.path)),
        sha1: Some(swap.sha1.into()),
        size: Some(swap.size),
    };
}

/// 1.17'den itibaren (sayısal ya da 1.17-1.19 varlık dizini) MobileGlues,
/// öncesi gl4es; Amethyst'in otomatik seçimiyle aynı.
pub fn renderer_for(assets_id: &str) -> &'static str {
    let modern = (!assets_id.is_empty() && assets_id.chars().all(|c| c.is_ascii_digit()))
        || matches!(assets_id, "1.17" | "1.18" | "1.19");
    if modern {
        RENDERER_MOBILEGLUES
    } else {
        RENDERER_GL4ES
    }
}

/// Motorun kendisi verdiği JVM argümanlarını çıkarır: sınıf yolu, bellek ve
/// `java.library.path` motorda kurulur.
pub fn filter_jvm_args(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut skip_next = false;
    for a in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a == "-cp" || a == "-classpath" || a == "--class-path" {
            skip_next = true;
            continue;
        }
        if a.starts_with("-Djava.library.path=")
            || a.starts_with("-Xms")
            || a.starts_with("-Xmx")
            || a.starts_with("-Dlog4j.configurationFile=")
            || a == "-XstartOnFirstThread"
        {
            continue;
        }
        out.push(a);
    }
    out
}

/// `--fullscreen`, `--width` ve `--height` telefonda anlamsız: pencereyi motor açar.
pub fn filter_game_args(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut skip_next = false;
    for a in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a == "--width" || a == "--height" {
            skip_next = true;
            continue;
        }
        if a == "--fullscreen" {
            continue;
        }
        out.push(a);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::library::NativeJar;

    fn lib(name: &str) -> LibArtifact {
        let c = Coordinate::parse(name).unwrap();
        let mut p = PathBuf::from("/data/libraries");
        for part in c.group.split('.') {
            p.push(part);
        }
        p.push(&c.artifact);
        p.push(&c.version);
        p.push(format!("{}-{}.jar", c.artifact, c.version));
        LibArtifact {
            name: name.into(),
            path: p,
            url: Some("https://libraries.minecraft.net/x.jar".into()),
            sha1: None,
            size: None,
        }
    }

    #[test]
    fn drops_lwjgl_and_natives() {
        let libs = ResolvedLibraries {
            classpath: vec![
                lib("com.mojang:brigadier:1.0.18"),
                lib("org.lwjgl:lwjgl:3.3.3"),
                lib("org.lwjgl:lwjgl:3.3.3:natives-linux"),
                lib("org.lwjgl:lwjgl-opengl:3.3.3"),
                lib("com.mojang:jtracy:1.0.29:natives-linux"),
            ],
            natives: vec![NativeJar {
                artifact: lib("com.mojang:text2speech:1.17.9:natives-linux"),
                exclude: vec![],
                flatten: false,
            }],
        };
        let (out, n) = adapt_libraries(libs, Path::new("/data/libraries")).unwrap();
        assert_eq!(n, 333);
        assert_eq!(lwjgl_component(n), "3.3.3");
        let names: Vec<_> = out.classpath.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["com.mojang:brigadier:1.0.18"]);
        assert!(out.natives.is_empty());
    }

    #[test]
    fn detects_lwjgl_versions() {
        let n = |v: &str| lwjgl_number(&[lib(v)]);
        assert_eq!(n("org.lwjgl:lwjgl:3.4.1"), Some(341));
        assert_eq!(lwjgl_component(341), "3.4.1");
        assert_eq!(n("org.lwjgl:lwjgl:3.2.2"), Some(322));
        assert_eq!(n("org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"), Some(294));
        assert_eq!(n("com.mojang:brigadier:1.0.18"), None);
        assert!(adapt_libraries(ResolvedLibraries::default(), Path::new("/l")).is_none());
    }

    #[test]
    fn replaces_old_jna_for_the_engine_native() {
        let libs = ResolvedLibraries {
            classpath: vec![
                lib("org.lwjgl:lwjgl:3.3.1"),
                lib("net.java.dev.jna:jna:5.12.1"),
                lib("com.github.oshi:oshi-core:6.2.2"),
                lib("org.ow2.asm:asm-all:4.1"),
                lib("net.java.dev.jna:jna-platform:5.12.1"),
            ],
            natives: vec![],
        };
        let (out, _) = adapt_libraries(libs, Path::new("/data/libraries")).unwrap();
        let names: Vec<_> = out.classpath.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "net.java.dev.jna:jna:5.14.0",
                "com.github.oshi:oshi-core:6.3.0",
                "org.ow2.asm:asm-all:5.0.4",
                "net.java.dev.jna:jna-platform:5.12.1",
            ]
        );
        let jna = &out.classpath[0];
        assert_eq!(
            jna.path,
            PathBuf::from("/data/libraries/net/java/dev/jna/jna/5.14.0/jna-5.14.0.jar")
        );
        assert!(
            jna.url
                .as_deref()
                .unwrap()
                .starts_with("https://repo1.maven.org/")
        );

        let keep = ResolvedLibraries {
            classpath: vec![
                lib("org.lwjgl:lwjgl:3.3.3"),
                lib("net.java.dev.jna:jna:5.15.0"),
            ],
            natives: vec![],
        };
        let (out, _) = adapt_libraries(keep, Path::new("/data/libraries")).unwrap();
        assert_eq!(out.classpath[0].name, "net.java.dev.jna:jna:5.15.0");
    }

    #[test]
    fn picks_renderer_by_assets_index() {
        assert_eq!(renderer_for("26"), RENDERER_MOBILEGLUES);
        assert_eq!(renderer_for("1.19"), RENDERER_MOBILEGLUES);
        assert_eq!(renderer_for("1.17"), RENDERER_MOBILEGLUES);
        assert_eq!(renderer_for("1.16"), RENDERER_GL4ES);
        assert_eq!(renderer_for("1.12"), RENDERER_GL4ES);
        assert_eq!(renderer_for("legacy"), RENDERER_GL4ES);
        assert_eq!(renderer_for("pre-1.6"), RENDERER_GL4ES);
    }

    /// Kotlin tarafı (`GamePlan.kt`) bu adlarla okur; değişirse ikisi birlikte değişmeli.
    #[test]
    fn plan_json_matches_the_engine_reader() {
        let plan = AndroidLaunch {
            version_id: "1.21.1".into(),
            java_home: "/j".into(),
            java_major: 21,
            game_dir: "/g".into(),
            natives_dir: "/n".into(),
            classpath: vec!["/a.jar".into()],
            jvm_args: vec![],
            main_class: "M".into(),
            game_args: vec!["--demo".into()],
            memory_mb: 2048,
            lwjgl: "3.3.3".into(),
            lwjglx: false,
            renderer: RENDERER_MOBILEGLUES.into(),
            modern_input: true,
            log_config: Some(LogConfig {
                id: "client-1.12.xml".into(),
                path: "/l.xml".into(),
            }),
        };
        let v = serde_json::to_value(&plan).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "classpath",
                "gameArgs",
                "gameDir",
                "javaHome",
                "javaMajor",
                "jvmArgs",
                "logConfig",
                "lwjgl",
                "lwjglx",
                "mainClass",
                "memoryMb",
                "modernInput",
                "nativesDir",
                "renderer",
                "versionId",
            ]
        );
        assert_eq!(v["logConfig"]["id"], "client-1.12.xml");
        assert_eq!(v["logConfig"]["path"], "/l.xml");
    }

    #[test]
    fn filters_engine_owned_arguments() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let jvm = filter_jvm_args(s(&[
            "-Xmx4G",
            "-Djava.library.path=/n",
            "-Dminecraft.launcher.brand=MehburMC",
            "-cp",
            "/a.jar:/b.jar",
            "-Dlog4j.configurationFile=/x.xml",
            "-DignoreList=a,b",
        ]));
        assert_eq!(
            jvm,
            s(&["-Dminecraft.launcher.brand=MehburMC", "-DignoreList=a,b"])
        );
        let game = filter_game_args(s(&[
            "--username",
            "Steve",
            "--width",
            "854",
            "--height",
            "480",
            "--fullscreen",
            "--demo",
        ]));
        assert_eq!(game, s(&["--username", "Steve", "--demo"]));
    }
}
