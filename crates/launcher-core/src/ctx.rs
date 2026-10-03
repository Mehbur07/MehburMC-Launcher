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
    pub fabric_meta: String,
    pub quilt_meta: String,
    pub legacy_fabric_meta: String,
    /// files.minecraftforge.net (version index + promotions).
    pub forge_files: String,
    pub forge_maven: String,
    pub neoforge_maven: String,
    pub modrinth: String,
    pub ms_login: String,
    pub xbox_user: String,
    pub xbox_xsts: String,
    pub mc_services: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            version_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
                .into(),
            resources: "https://resources.download.minecraft.net".into(),
            adoptium: "https://api.adoptium.net".into(),
            fabric_meta: "https://meta.fabricmc.net".into(),
            quilt_meta: "https://meta.quiltmc.org".into(),
            legacy_fabric_meta: "https://meta.legacyfabric.net".into(),
            forge_files: "https://files.minecraftforge.net".into(),
            forge_maven: "https://maven.minecraftforge.net".into(),
            neoforge_maven: "https://maven.neoforged.net".into(),
            modrinth: "https://api.modrinth.com".into(),
            ms_login: "https://login.microsoftonline.com".into(),
            xbox_user: "https://user.auth.xboxlive.com".into(),
            xbox_xsts: "https://xsts.auth.xboxlive.com".into(),
            mc_services: "https://api.minecraftservices.com".into(),
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
