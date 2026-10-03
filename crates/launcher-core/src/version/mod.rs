//! Version manifest, version JSON loading and `inheritsFrom` resolution.

pub mod compare;
pub mod merge;
pub mod profile;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;
use crate::net::cache::get_json_cached;
pub use profile::VersionJson;

const MANIFEST_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_INHERITANCE_DEPTH: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestEntry {
    pub id: String,
    /// `release`, `snapshot`, `old_beta` or `old_alpha`.
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

pub async fn manifest(ctx: &Ctx) -> Result<VersionManifest> {
    let file = ctx.paths.cache().join("version_manifest_v2.json");
    get_json_cached(
        &ctx.http,
        &ctx.endpoints.version_manifest,
        &file,
        MANIFEST_TTL,
    )
    .await
}

/// A fully merged, launch-ready version.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub json: VersionJson,
    /// Version whose `client.jar` goes on the classpath.
    pub jar_id: String,
    /// `[child, …, root]`.
    pub chain: Vec<String>,
}

/// Loads `id` (downloading it if needed) and merges its `inheritsFrom` chain.
pub async fn resolve(ctx: &Ctx, id: &str) -> Result<ResolvedVersion> {
    let manifest = match manifest(ctx).await {
        Ok(m) => Some(m),
        Err(e) => {
            tracing::warn!(error = %e.detail(), "version manifest unavailable, using local files");
            None
        }
    };

    let mut chain = Vec::new();
    let mut jsons = Vec::new();
    let mut next = Some(id.to_owned());
    while let Some(cur) = next {
        if chain.len() >= MAX_INHERITANCE_DEPTH || chain.contains(&cur) {
            return Err(CoreError::InvalidVersion {
                id: id.to_owned(),
                reason: "inheritsFrom chain is too deep or cyclic".into(),
            });
        }
        let json = load(ctx, &cur, manifest.as_ref()).await?;
        next = json.inherits_from.clone();
        chain.push(cur);
        jsons.push(json);
    }

    // jar: explicit `jar` field, else the deepest version that ships a client.
    let explicit_jar = jsons.iter().find_map(|j| j.jar.clone());
    let jar_id = explicit_jar
        .or_else(|| {
            jsons
                .iter()
                .find(|j| j.downloads.as_ref().is_some_and(|d| d.client.is_some()))
                .map(|j| j.id.clone())
        })
        .unwrap_or_else(|| chain.last().cloned().unwrap_or_default());

    let mut merged = jsons.pop().expect("chain is non-empty");
    while let Some(child) = jsons.pop() {
        merged = merge::merge(child, merged);
    }
    if merged.main_class.is_none() {
        return Err(CoreError::InvalidVersion {
            id: id.to_owned(),
            reason: "mainClass is missing".into(),
        });
    }
    Ok(ResolvedVersion {
        json: merged,
        jar_id,
        chain,
    })
}

/// Returns the local version file if it is current, otherwise downloads it
/// from the manifest (verifying its SHA-1).
async fn load(ctx: &Ctx, id: &str, manifest: Option<&VersionManifest>) -> Result<VersionJson> {
    let path = ctx.paths.versions().join(id).join(format!("{id}.json"));
    let entry = manifest.and_then(|m| m.versions.iter().find(|v| v.id == id));
    let local = std::fs::read(&path).ok();

    if let Some(bytes) = &local {
        let current = match entry.and_then(|e| e.sha1.as_deref()) {
            // Not a Mojang version (loader profile) or offline: trust the file.
            None => true,
            Some(expected) => sha1_hex(bytes).eq_ignore_ascii_case(expected),
        };
        if current {
            return parse(&path, bytes);
        }
    }

    let Some(entry) = entry else {
        return Err(CoreError::VersionNotFound(id.to_owned()));
    };
    let bytes = ctx.http.get_bytes(&entry.url).await?;
    if let Some(expected) = &entry.sha1 {
        let actual = sha1_hex(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(CoreError::HashMismatch {
                path,
                expected: expected.clone(),
                actual,
            });
        }
    }
    write_atomic(&path, &bytes)?;
    parse(&path, &bytes)
}

fn parse(path: &std::path::Path, bytes: &[u8]) -> Result<VersionJson> {
    serde_json::from_slice(bytes).map_err(|source| CoreError::Json {
        path: path.to_owned(),
        source,
    })
}

fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_version_files() {
        for (file, major, has_new_args) in [
            ("26.3.json", 25, true),
            ("1.12.2.json", 8, false),
            ("1.5.2.json", 8, false),
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/versions")
                .join(file);
            let v: VersionJson = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(v.java_major(), major, "{file}");
            assert_eq!(v.arguments.is_some(), has_new_args, "{file}");
            assert!(!v.libraries.is_empty());
        }
    }
}
