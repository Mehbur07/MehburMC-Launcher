//! Fabric, Quilt and Legacy Fabric: their meta servers hand out a ready-made
//! launcher profile (`inheritsFrom` + Maven libraries with checksums), so
//! installing is "download the profile JSON, then download its libraries".

use serde::Deserialize;

use super::{LoaderVersion, compare_loader_versions};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::instance::LoaderKind;
use crate::net::cache::get_json_cached;
use crate::net::join_url;

#[derive(Debug, Deserialize)]
struct Entry {
    loader: Info,
}

#[derive(Debug, Deserialize)]
struct Info {
    version: String,
    /// Fabric and Legacy Fabric only; Quilt marks betas in the version.
    #[serde(default)]
    stable: Option<bool>,
}

/// `(meta base, api version)`.
fn meta(ctx: &Ctx, kind: LoaderKind) -> (&str, &'static str) {
    match kind {
        LoaderKind::Quilt => (&ctx.endpoints.quilt_meta, "v3"),
        LoaderKind::LegacyFabric => (&ctx.endpoints.legacy_fabric_meta, "v2"),
        _ => (&ctx.endpoints.fabric_meta, "v2"),
    }
}

fn cache_key(kind: LoaderKind, mc: &str) -> String {
    let safe: String = mc
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{kind:?}-{safe}.json").to_ascii_lowercase()
}

pub async fn list(ctx: &Ctx, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    let (base, api) = meta(ctx, kind);
    let url = join_url(base, &[api, "versions", "loader", mc])?;
    let file = ctx.paths.cache().join("loaders").join(cache_key(kind, mc));
    let entries: Vec<Entry> = match get_json_cached(&ctx.http, &url, &file, super::LIST_TTL).await {
        Ok(e) => e,
        // Unknown game versions are answered with 400/404.
        Err(CoreError::HttpStatus {
            status: 400 | 404, ..
        }) => vec![],
        Err(e) => return Err(e),
    };
    Ok(to_versions(entries, kind == LoaderKind::Quilt))
}

fn to_versions(entries: Vec<Entry>, sort: bool) -> Vec<LoaderVersion> {
    let mut out: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| {
            let stable = e
                .loader
                .stable
                .unwrap_or_else(|| !e.loader.version.contains('-'));
            LoaderVersion {
                label: e.loader.version.clone(),
                id: e.loader.version,
                stable,
                recommended: false,
            }
        })
        .collect();
    // Quilt's meta returns versions in no particular order.
    if sort {
        out.sort_by(|a, b| compare_loader_versions(&b.id, &a.id));
    }
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// Returns the launcher profile; libraries are fetched at launch like any
/// other version's.
pub async fn install(
    ctx: &Ctx,
    kind: LoaderKind,
    mc: &str,
    loader_version: &str,
    progress: &Progress,
) -> Result<serde_json::Value> {
    progress.set_totals(1, 0);
    let (base, api) = meta(ctx, kind);
    let url = join_url(
        base,
        &[
            api,
            "versions",
            "loader",
            mc,
            loader_version,
            "profile",
            "json",
        ],
    )?;
    let json: serde_json::Value = ctx.http.get_json(&url).await?;
    progress.item_done();
    Ok(json)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::NullSink;
    use crate::instance::LoaderSpec;
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    #[test]
    fn quilt_versions_are_sorted_and_betas_unstable() {
        let entries: Vec<Entry> = serde_json::from_str(
            r#"[{"loader":{"version":"0.20.0-beta.9"}},{"loader":{"version":"0.30.1"}},
                {"loader":{"version":"0.31.0-beta.4"}},{"loader":{"version":"0.29.2"}}]"#,
        )
        .unwrap();
        let v = to_versions(entries, true);
        let ids: Vec<_> = v.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["0.31.0-beta.4", "0.30.1", "0.29.2", "0.20.0-beta.9"]);
        assert!(!v[0].stable && v[1].stable);
    }

    #[test]
    fn fabric_keeps_server_order_and_flags() {
        let entries: Vec<Entry> = serde_json::from_str(
            r#"[{"loader":{"version":"0.19.5","stable":true}},{"loader":{"version":"0.19.4","stable":false}}]"#,
        )
        .unwrap();
        let v = to_versions(entries, false);
        assert_eq!(v[0].id, "0.19.5");
        assert!(v[0].stable && !v[1].stable);
    }

    #[tokio::test]
    async fn installs_profile_with_our_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/versions/loader/26.3"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{"loader":{"version":"0.19.5","stable":true}},{"loader":{"version":"0.19.4","stable":false}}]"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/versions/loader/26.3/0.19.5/profile/json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id":"fabric-loader-0.19.5-26.3","inheritsFrom":"26.3","type":"release",
                    "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                    "libraries":[{"name":"net.fabricmc:fabric-loader:0.19.5","url":"https://maven.fabricmc.net/"}]}"#,
            ))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.fabric_meta = server.uri();

        let spec = LoaderSpec {
            kind: LoaderKind::Fabric,
            version: None,
        };
        let c = tokio_util::sync::CancellationToken::new();
        let got = super::super::ensure_installed(&ctx, "26.3", &spec, false, "t", &c)
            .await
            .unwrap();
        assert_eq!(got.version_id, "fabric-loader-0.19.5-26.3");
        assert_eq!(got.loader_version.as_deref(), Some("0.19.5"));
        let file = ctx
            .paths
            .versions()
            .join(&got.version_id)
            .join(format!("{}.json", got.version_id));
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(json["inheritsFrom"], "26.3");
    }
}
