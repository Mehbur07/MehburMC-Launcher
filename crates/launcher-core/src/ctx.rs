//! Shared context passed to every long-running core operation.

use std::sync::Arc;

use crate::error::Result;
use crate::events::EventSink;
use crate::net::download::Downloader;
use crate::net::{Allowlist, Http};
use crate::os::OsInfo;
use crate::paths::Paths;

/// Remote endpoints; overridable so tests can point at a mock server.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub version_manifest: String,
    pub resources: String,
    pub adoptium: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            version_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
                .into(),
            resources: "https://resources.download.minecraft.net".into(),
            adoptium: "https://api.adoptium.net".into(),
        }
    }
}

#[derive(Clone)]
pub struct Ctx {
    pub paths: Paths,
    pub http: Http,
    pub events: Arc<dyn EventSink>,
    pub endpoints: Endpoints,
    pub os: OsInfo,
    pub concurrency: usize,
}

impl Ctx {
    pub fn new(paths: Paths, events: Arc<dyn EventSink>, concurrency: usize) -> Result<Self> {
        Ok(Self {
            paths,
            http: Http::new(Allowlist::default())?,
            events,
            endpoints: Endpoints::default(),
            os: OsInfo::current(),
            concurrency,
        })
    }

    pub fn downloader(&self) -> Downloader {
        Downloader::new(self.http.clone(), self.concurrency)
    }
}
