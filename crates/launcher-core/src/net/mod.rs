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

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.get(url).await?;
        let bytes = resp.bytes().await.map_err(|source| network(url, source))?;
        Ok(bytes.to_vec())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
            path: url.into(),
            source,
        })
    }
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
