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
}
