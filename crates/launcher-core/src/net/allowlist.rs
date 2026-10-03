//! HTTPS + host (+ optional path prefix) allowlist for every outgoing request,
//! including each redirect hop. See ARCHITECTURE.md §6.

use reqwest::Url;

/// `(host, optional path prefix)`.
const DEFAULT_RULES: &[(&str, Option<&str>)] = &[
    // Mojang
    ("piston-meta.mojang.com", None),
    ("piston-data.mojang.com", None),
    ("launchermeta.mojang.com", None),
    ("launcher.mojang.com", None),
    ("libraries.minecraft.net", None),
    ("resources.download.minecraft.net", None),
    ("launchercontent.mojang.com", None),
    ("textures.minecraft.net", None),
    ("sessionserver.mojang.com", None),
    ("api.minecraftservices.com", None),
    // Microsoft / Xbox auth
    ("login.microsoftonline.com", None),
    ("user.auth.xboxlive.com", None),
    ("xsts.auth.xboxlive.com", None),
    // Fabric / Quilt / Legacy Fabric
    ("meta.fabricmc.net", None),
    ("maven.fabricmc.net", None),
    ("meta.quiltmc.org", None),
    ("maven.quiltmc.org", None),
    ("meta.legacyfabric.net", None),
    ("maven.legacyfabric.net", None),
    // Forge / NeoForge
    ("files.minecraftforge.net", None),
    ("maven.minecraftforge.net", None),
    ("maven.neoforged.net", None),
    ("repo1.maven.org", None),
    // Modrinth
    ("api.modrinth.com", None),
    ("cdn.modrinth.com", None),
    // CurseForge
    ("api.curseforge.com", None),
    ("edge.forgecdn.net", None),
    ("mediafilez.forgecdn.net", None),
    // Adoptium (binaries are GitHub release assets)
    ("api.adoptium.net", None),
    ("github.com", Some("/adoptium/")),
    ("objects.githubusercontent.com", None),
    ("release-assets.githubusercontent.com", None),
];

#[derive(Debug, Clone)]
pub struct Allowlist {
    rules: Vec<(String, Option<String>)>,
    /// Test-only escape hatch for a local mock server (`http://127.0.0.1`).
    allow_loopback_http: bool,
}

impl Default for Allowlist {
    fn default() -> Self {
        Self {
            rules: DEFAULT_RULES
                .iter()
                .map(|(h, p)| ((*h).to_owned(), p.map(str::to_owned)))
                .collect(),
            allow_loopback_http: false,
        }
    }
}

impl Allowlist {
    /// Allowlist that additionally permits plain HTTP to 127.0.0.1/localhost.
    pub fn with_loopback() -> Self {
        Self {
            allow_loopback_http: true,
            ..Self::default()
        }
    }

    pub fn is_allowed(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        if self.allow_loopback_http && matches!(host.as_str(), "127.0.0.1" | "localhost") {
            return matches!(url.scheme(), "http" | "https");
        }
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        self.rules.iter().any(|(h, prefix)| {
            *h == host && prefix.as_deref().is_none_or(|p| url.path().starts_with(p))
        })
    }

    pub fn check(&self, url: &str) -> crate::Result<Url> {
        let parsed = Url::parse(url).map_err(|_| crate::CoreError::UrlNotAllowed {
            url: url.to_owned(),
        })?;
        if self.is_allowed(&parsed) {
            Ok(parsed)
        } else {
            Err(crate::CoreError::UrlNotAllowed {
                url: url.to_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(u: &str) -> bool {
        Allowlist::default().is_allowed(&Url::parse(u).unwrap())
    }

    #[test]
    fn allows_official_https_hosts() {
        assert!(ok(
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
        ));
        assert!(ok("https://LIBRARIES.minecraft.net/x.jar"));
        assert!(ok(
            "https://github.com/adoptium/temurin25-binaries/releases/download/x/y.zip"
        ));
    }

    #[test]
    fn rejects_everything_else() {
        assert!(!ok("http://piston-meta.mojang.com/x")); // plain http
        assert!(!ok("https://evil.example.com/x"));
        assert!(!ok("https://piston-meta.mojang.com.evil.com/x")); // suffix trick
        assert!(!ok("https://github.com/someone/malware/releases/x.zip")); // path prefix
        assert!(!ok("https://user:pw@libraries.minecraft.net/x"));
        assert!(!ok("http://127.0.0.1:8080/x")); // loopback only in tests
        assert!(
            Allowlist::with_loopback().is_allowed(&Url::parse("http://127.0.0.1:8080/x").unwrap())
        );
    }
}
