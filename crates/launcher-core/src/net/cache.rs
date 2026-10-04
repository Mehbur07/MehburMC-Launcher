//! TTL cache for small metadata documents (manifests, loader version lists).
//!
//! Fresh entries are served from disk; expired entries are refetched; if the
//! network is unavailable a stale entry is used instead (offline mode).

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;

use super::Http;
use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

pub async fn get_json_cached<T: DeserializeOwned>(
    http: &Http,
    url: &str,
    file: &Path,
    ttl: Duration,
) -> Result<T> {
    if is_fresh(file, ttl)
        && let Some(v) = read_json(file)
    {
        return Ok(v);
    }
    match http.get_bytes(url).await {
        Ok(bytes) => {
            let value = serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
                path: url.into(),
                source,
            })?;
            write_atomic(file, &bytes)?;
            Ok(value)
        }
        Err(err) => match read_json(file) {
            Some(stale) => {
                tracing::warn!(%url, error = %err.detail(), "using stale cache (offline?)");
                Ok(stale)
            }
            None => Err(err),
        },
    }
}

fn is_fresh(file: &Path, ttl: Duration) -> bool {
    std::fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < ttl)
}

fn read_json<T: DeserializeOwned>(file: &Path) -> Option<T> {
    let bytes = std::fs::read(file).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::net::Allowlist;

    #[tokio::test]
    async fn fresh_hit_then_stale_fallback() {
        let server = MockServer::builder().start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"v":1}"#))
            .expect(1)
            .mount(&server)
            .await;
        let http = Http::new(Allowlist::with_loopback()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("c.json");
        let url = format!("{}/m.json", server.uri());

        let v: serde_json::Value = get_json_cached(&http, &url, &file, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(v["v"], 1);
        // Fresh: served from disk, no second request (expect(1) above).
        let _: serde_json::Value = get_json_cached(&http, &url, &file, Duration::from_secs(60))
            .await
            .unwrap();

        // Expired + server gone → stale copy is used.
        drop(server);
        let v: serde_json::Value = get_json_cached(&http, &url, &file, Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(v["v"], 1);
    }
}
