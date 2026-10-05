//! Networking: shared HTTP client, allowlist, downloader, metadata cache.

pub mod allowlist;
pub mod cache;
pub mod download;

use std::sync::Arc;
use std::time::Duration;

use reqwest::redirect;
use serde::de::DeserializeOwned;

pub use allowlist::Allowlist;

use crate::error::{CoreError, Result};

const USER_AGENT: &str = concat!(
    "MehburMC-Launcher/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Mehbur07/MehburMC-Launcher)"
);

/// Cheap-to-clone HTTP client that enforces the [`Allowlist`] on every
/// request and every redirect hop.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    allowlist: Arc<Allowlist>,
}

impl Http {
    pub fn new(allowlist: Allowlist) -> Result<Self> {
        let allowlist = Arc::new(allowlist);
        let policy_list = allowlist.clone();
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .redirect(redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else if policy_list.is_allowed(attempt.url()) {
                    attempt.follow()
                } else {
                    let url = attempt.url().to_string();
                    attempt.error(format!("redirect to {url} is not on the allowlist"))
                }
            }))
            .build()
            .map_err(|source| CoreError::Network {
                url: String::new(),
                source,
            })?;
        Ok(Self { client, allowlist })
    }

    pub fn allowlist(&self) -> &Allowlist {
        &self.allowlist
    }

    pub(crate) fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// GET returning the response if the status is 2xx.
    pub async fn get(&self, url: &str) -> Result<reqwest::Response> {
        let parsed = self.allowlist.check(url)?;
        let resp = self
            .client
            .get(parsed)
            .send()
            .await
            .map_err(|source| network(url, source))?;
        check_status(url, resp)
    }

    /// Whole body in memory (metadata, icons, skins); capped at
    /// [`MAX_BODY_BYTES`]. Large files go through the streaming downloader.
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.get(url).await?;
        read_limited(url, resp, MAX_BODY_BYTES).await
    }

    /// JSON request with extra headers; returns `(status, body)` without
    /// treating non-2xx as an error (APIs explain errors in the body).
    pub async fn request_json(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
        headers: &[(&str, &str)],
    ) -> Result<(u16, Vec<u8>)> {
        let parsed = self.allowlist.check(url)?;
        let mut req = self
            .client
            .request(method, parsed)
            .header(reqwest::header::ACCEPT, "application/json");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        if let Some(b) = body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(b.to_string());
        }
        let resp = req.send().await.map_err(|source| network(url, source))?;
        let status = resp.status().as_u16();
        let bytes = read_limited(url, resp, MAX_BODY_BYTES).await?;
        Ok((status, bytes))
    }

    /// Request with a raw body (file uploads); same contract as
    /// [`Self::request_json`]. Uploads may take longer than the read timeout
    /// of a normal response, so the whole request gets `timeout`.
    pub async fn request_bytes(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Vec<u8>,
        content_type: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<(u16, Vec<u8>)> {
        let parsed = self.allowlist.check(url)?;
        let mut req = self
            .client
            .request(method, parsed)
            .timeout(timeout)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let resp = req.send().await.map_err(|source| network(url, source))?;
        let status = resp.status().as_u16();
        let bytes = read_limited(url, resp, MAX_BODY_BYTES).await?;
        Ok((status, bytes))
    }

    pub async fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<(u16, Vec<u8>)> {
        self.request_json(reqwest::Method::POST, url, Some(body), &[])
            .await
    }

    pub async fn get_text(&self, url: &str) -> Result<String> {
        let bytes = self.get_bytes(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
            path: url.into(),
            source,
        })
    }
}

/// Appends percent-encoded path segments to `base`
/// (`https://meta.fabricmc.net/` + `["v2", "1.14 Pre-Release 1"]`).
pub fn join_url(base: &str, segments: &[&str]) -> Result<String> {
    let mut url = reqwest::Url::parse(base).map_err(|_| CoreError::UrlNotAllowed {
        url: base.to_owned(),
    })?;
    url.path_segments_mut()
        .map_err(|()| CoreError::UrlNotAllowed {
            url: base.to_owned(),
        })?
        .pop_if_empty()
        .extend(segments);
    Ok(url.into())
}

/// Upper bound for responses read into memory.
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Reads a body, refusing anything larger than `max` (checked against
/// `Content-Length` first, then while streaming).
pub(crate) async fn read_limited(
    url: &str,
    resp: reqwest::Response,
    max: usize,
) -> Result<Vec<u8>> {
    use futures_util::StreamExt;
    let too_large = || CoreError::ResponseTooLarge {
        url: url.to_owned(),
    };
    if resp.content_length().is_some_and(|n| n > max as u64) {
        return Err(too_large());
    }
    let mut out = Vec::with_capacity(resp.content_length().unwrap_or(0) as usize);
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|source| network(url, source))?;
        if out.len() + chunk.len() > max {
            return Err(too_large());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

pub(crate) fn network(url: &str, source: reqwest::Error) -> CoreError {
    CoreError::Network {
        url: url.to_owned(),
        source,
    }
}

pub(crate) fn check_status(url: &str, resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        Err(CoreError::HttpStatus {
            url: url.to_owned(),
            status: status.as_u16(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_oversized_bodies() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::builder().start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 2048]))
            .mount(&server)
            .await;
        let http = Http::new(Allowlist::with_loopback()).unwrap();
        let url = format!("{}/big", server.uri());
        let resp = http.get(&url).await.unwrap();
        let e = read_limited(&url, resp, 1024).await.unwrap_err();
        assert_eq!(e.code(), "net.tooLarge");
        let resp = http.get(&url).await.unwrap();
        assert_eq!(read_limited(&url, resp, 4096).await.unwrap().len(), 2048);
    }

    #[test]
    fn join_url_encodes_segments() {
        assert_eq!(
            join_url(
                "https://meta.fabricmc.net/",
                &["v2", "versions", "1.14 Pre-Release 1"]
            )
            .unwrap(),
            "https://meta.fabricmc.net/v2/versions/1.14%20Pre-Release%201"
        );
        assert_eq!(
            join_url("https://x.org/base", &["a+b"]).unwrap(),
            "https://x.org/base/a+b"
        );
    }
}
