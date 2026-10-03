//! Minimal Modrinth v2 client (versions of a project, single version, project).

use serde::Deserialize;

use crate::ctx::Ctx;
use crate::error::Result;
use crate::net::join_url;

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    /// `release`, `beta` or `alpha`.
    pub version_type: String,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: Hashes,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hashes {
    #[serde(default)]
    pub sha512: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    /// `required`, `optional`, `incompatible` or `embedded`.
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
}

/// Versions of a project (slug or id) for the given loaders and game
/// version, newest first.
pub async fn project_versions(
    ctx: &Ctx,
    project: &str,
    loaders: &[&str],
    game_version: &str,
) -> Result<Vec<Version>> {
    let base = join_url(
        &ctx.endpoints.modrinth,
        &["v2", "project", project, "version"],
    )?;
    let mut url = reqwest::Url::parse(&base).expect("join_url returns a valid URL");
    let loaders = serde_json::to_string(loaders).expect("strings serialize");
    let games = serde_json::to_string(&[game_version]).expect("strings serialize");
    url.query_pairs_mut()
        .append_pair("loaders", &loaders)
        .append_pair("game_versions", &games);
    ctx.http.get_json(url.as_str()).await
}

pub async fn version(ctx: &Ctx, id: &str) -> Result<Version> {
    let url = join_url(&ctx.endpoints.modrinth, &["v2", "version", id])?;
    ctx.http.get_json(&url).await
}

pub async fn project(ctx: &Ctx, id_or_slug: &str) -> Result<Project> {
    let url = join_url(&ctx.endpoints.modrinth, &["v2", "project", id_or_slug])?;
    ctx.http.get_json(&url).await
}
