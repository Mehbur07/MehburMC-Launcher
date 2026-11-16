//! Host platform in the vocabulary used by Mojang version files.

/// Platform description used for rule evaluation and native selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsInfo {
    /// `windows`, `osx` or `linux` (Mojang naming).
    pub name: &'static str,
    /// `x86_64`, `x86`, `arm64` or `arm32`.
    pub arch: &'static str,
    /// OS version string matched against `rules[].os.version` regexes.
    pub version: String,
    /// Apple Silicon Mac'te oyun Intel olarak, Rosetta 2 ile çalışıyor (K108):
    /// `arch` bu durumda `x86_64` olur, Java da Intel sürümüdür.
    pub rosetta: bool,
}

impl OsInfo {
    pub fn current() -> Self {
        let name = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "x86" => "x86",
            "aarch64" => "arm64",
            "arm" => "arm32",
            _ => "x86_64",
        };
        Self {
            name,
            arch,
            version: os_version(),
            rosetta: false,
        }
    }

    /// Bu kütüphanelerle oyun hangi platform gibi çalışmalı? Apple Silicon
    /// Mac'te 1.19 ve sonrası `natives-macos-arm64` kütüphaneleri getirir ve
    /// yerel çalışır. Daha eski sürümlerin (LWJGL 2 / 3.2) yalnız Intel yerel
    /// dosyaları vardır: oyun Intel Java ile Rosetta 2 üzerinden açılır (K108).
    /// Başka her sistemde platform aynen kalır.
    pub fn for_game(&self, libraries: &[crate::version::profile::Library]) -> Self {
        if self.name != "osx" || self.arch != "arm64" {
            return self.clone();
        }
        let native_arm = libraries.iter().any(|l| {
            l.name.ends_with(":natives-macos-arm64")
                || l.downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .is_some_and(|c| c.contains_key("natives-macos-arm64"))
        });
        if native_arm {
            return self.clone();
        }
        Self {
            arch: "x86_64",
            rosetta: true,
            ..self.clone()
        }
    }

    /// Value substituted for `${arch}` in legacy `natives` classifiers.
    pub fn bitness(&self) -> &'static str {
        match self.arch {
            "x86" | "arm32" => "32",
            _ => "64",
        }
    }

    /// Classpath separator for `${classpath_separator}`.
    pub fn classpath_separator(&self) -> &'static str {
        if self.name == "windows" { ";" } else { ":" }
    }

    /// Whether a modern `natives-<os>[-<arch>]` classifier targets this
    /// machine. `natives-windows` (no suffix) means x86_64.
    pub fn matches_native_classifier(&self, classifier: &str) -> bool {
        let Some(rest) = classifier.strip_prefix("natives-") else {
            return false;
        };
        let (os, arch) = match rest.split_once('-') {
            Some((os, arch)) => (os, Some(arch)),
            None => (rest, None),
        };
        let os_ok = match os {
            "windows" => self.name == "windows",
            "macos" | "osx" => self.name == "osx",
            "linux" => self.name == "linux",
            _ => false,
        };
        let arch_ok = match arch {
            None => matches!(self.arch, "x86_64" | "x86"),
            Some("x86") => self.arch == "x86",
            Some("x86_64") => self.arch == "x86_64",
            Some("arm64" | "aarch64") => self.arch == "arm64",
            Some("arm32") => self.arch == "arm32",
            Some(_) => false,
        };
        os_ok && arch_ok
    }

    #[cfg(test)]
    pub fn fake(name: &'static str, arch: &'static str) -> Self {
        Self {
            name,
            arch,
            version: "10.0".into(),
            rosetta: false,
        }
    }
}

fn os_version() -> String {
    // Only used by a handful of old rules (macOS 10.5 workarounds); an
    // approximate value is fine and avoids platform-specific syscalls.
    std::env::var("OS_VERSION_OVERRIDE").unwrap_or_else(|_| "10.0".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_classifier_matching() {
        let win64 = OsInfo::fake("windows", "x86_64");
        assert!(win64.matches_native_classifier("natives-windows"));
        assert!(!win64.matches_native_classifier("natives-windows-arm64"));
        assert!(!win64.matches_native_classifier("natives-linux"));
        assert!(!win64.matches_native_classifier("sources"));

        let winarm = OsInfo::fake("windows", "arm64");
        assert!(winarm.matches_native_classifier("natives-windows-arm64"));
        assert!(!winarm.matches_native_classifier("natives-windows"));

        let mac = OsInfo::fake("osx", "arm64");
        assert!(mac.matches_native_classifier("natives-macos-arm64"));
    }

    fn libs(json: &str) -> Vec<crate::version::profile::Library> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn old_versions_run_under_rosetta_on_apple_silicon() {
        // 1.19+: arm64 yerel kütüphanesi var → yerel.
        let modern = libs(
            r#"[{"name":"org.lwjgl:lwjgl:3.3.1"},
                {"name":"org.lwjgl:lwjgl:3.3.1:natives-macos-arm64","rules":[{"action":"allow","os":{"name":"osx"}}]}]"#,
        );
        // 1.12 (LWJGL 2, eski "natives" haritası) ve 1.18 (LWJGL 3.2, yalnız natives-macos).
        let legacy = libs(
            r#"[{"name":"org.lwjgl.lwjgl:lwjgl-platform:2.9.4","natives":{"osx":"natives-osx"},
                 "downloads":{"classifiers":{"natives-osx":{"path":"a.jar","url":"https://x/a.jar"}}}}]"#,
        );
        let lwjgl32 = libs(
            r#"[{"name":"org.lwjgl:lwjgl:3.2.2:natives-macos","rules":[{"action":"allow","os":{"name":"osx"}}]}]"#,
        );

        let m1 = OsInfo::fake("osx", "arm64");
        assert_eq!(m1.for_game(&modern), m1);
        for old in [&legacy, &lwjgl32] {
            let g = m1.for_game(old);
            assert!(g.rosetta);
            assert_eq!((g.name, g.arch), ("osx", "x86_64"));
        }

        // Intel Mac, Windows, Linux: hiçbir şey değişmez.
        for host in [
            OsInfo::fake("osx", "x86_64"),
            OsInfo::fake("windows", "arm64"),
            OsInfo::fake("linux", "arm64"),
        ] {
            assert_eq!(host.for_game(&legacy), host);
        }
    }
}
