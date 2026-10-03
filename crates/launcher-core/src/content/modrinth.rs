//! Modrinth v2 client: search, projects, versions, hash lookups and updates.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::net::join_url;

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    pub version_number: String,
    /// `release`, `beta` or `alpha`.
    pub version_type: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: String,
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

/// Version summary for the UI.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionSummary {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
}

impl From<&Version> for VersionSummary {
    fn from(v: &Version) -> Self {
        Self {
            id: v.id.clone(),
            name: v.name.clone(),
            version_number: v.version_number.clone(),
            version_type: v.version_type.clone(),
            game_versions: v.game_versions.clone(),
            loaders: v.loaders.clone(),
            date_published: v.date_published.clone(),
        }
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

#[derive(Debug, Clone, Default, Deserialize)]
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
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

/// One search result, as shown in the browser.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchHit {
    pub project_id: String,
    pub project_type: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    #[ts(type = "number")]
    pub downloads: u64,
    pub date_modified: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    #[ts(type = "number")]
    pub total_hits: u64,
    #[ts(type = "number")]
    pub offset: u64,
}

/// Modrinth's snake_case wire format.
#[derive(Debug, Deserialize)]
struct RawPage {
    hits: Vec<RawHit>,
    total_hits: u64,
    offset: u64,
}

#[derive(Debug, Deserialize)]
struct RawHit {
    project_id: String,
    project_type: String,
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    date_modified: String,
    #[serde(default)]
    categories: Vec<String>,
}

impl From<RawPage> for SearchPage {
    fn from(r: RawPage) -> Self {
        Self {
            total_hits: r.total_hits,
            offset: r.offset,
            hits: r
                .hits
                .into_iter()
                .map(|h| SearchHit {
                    project_id: h.project_id,
                    project_type: h.project_type,
                    slug: h.slug,
                    title: h.title,
                    description: h.description,
                    author: h.author,
                    icon_url: h.icon_url.filter(|u| !u.is_empty()),
                    downloads: h.downloads,
                    date_modified: h.date_modified,
                    categories: h.categories,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProjectType {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
}

impl ProjectType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchSort {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchQuery {
    pub query: String,
    pub project_type: ProjectType,
    /// Restrict to this Minecraft version.
    #[ts(optional)]
    pub game_version: Option<String>,
    /// Restrict to this Modrinth loader (`fabric`, `forge`, …).
    #[ts(optional)]
    pub loader: Option<String>,
    #[serde(default)]
    pub sort: SearchSort,
    #[serde(default)]
    #[ts(type = "number")]
    pub offset: u64,
}

const PAGE_SIZE: u64 = 20;

/// Search facets: AND across groups, OR within a group.
pub fn facets(q: &SearchQuery) -> String {
    let mut groups = vec![vec![format!("project_type:{}", q.project_type.as_str())]];
    if let Some(v) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        groups.push(vec![format!("versions:{v}")]);
    }
    if let Some(l) = q.loader.as_deref().filter(|l| !l.is_empty()) {
        // Quilt runs Fabric mods.
        let mut g = vec![format!("categories:{l}")];
        if l == "quilt" {
            g.push("categories:fabric".into());
        }
        groups.push(g);
    }
    serde_json::to_string(&groups).expect("strings serialize")
}

pub async fn search(ctx: &Ctx, q: &SearchQuery) -> Result<SearchPage> {
    let mut url = parse(&join_url(&ctx.endpoints.modrinth, &["v2", "search"])?)?;
    let index = match q.sort {
        SearchSort::Relevance => "relevance",
        SearchSort::Downloads => "downloads",
        SearchSort::Updated => "updated",
        SearchSort::Newest => "newest",
    };
    url.query_pairs_mut()
        .append_pair("query", q.query.trim())
        .append_pair("facets", &facets(q))
        .append_pair("index", index)
        .append_pair("offset", &q.offset.to_string())
        .append_pair("limit", &PAGE_SIZE.to_string());
    let raw: RawPage = ctx.http.get_json(url.as_str()).await?;
    Ok(raw.into())
}

fn parse(u: &str) -> Result<reqwest::Url> {
    reqwest::Url::parse(u).map_err(|_| CoreError::UrlNotAllowed { url: u.to_owned() })
}

/// Versions of a project (slug or id), newest first. Empty filters are
/// left out.
pub async fn project_versions(
    ctx: &Ctx,
    project: &str,
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<Vec<Version>> {
    let base = join_url(
        &ctx.endpoints.modrinth,
        &["v2", "project", project, "version"],
    )?;
    let mut url = parse(&base)?;
    {
        let mut q = url.query_pairs_mut();
        if !loaders.is_empty() {
            q.append_pair(
                "loaders",
                &serde_json::to_string(loaders).expect("strings serialize"),
            );
        }
        if let Some(g) = game_version {
            q.append_pair(
                "game_versions",
                &serde_json::to_string(&[g]).expect("strings serialize"),
            );
        }
    }
    match ctx.http.get_json(url.as_str()).await {
        // Unknown project: "no compatible versions" reads better than HTTP 404.
        Err(CoreError::HttpStatus { status: 404, .. }) => Ok(vec![]),
        other => other,
    }
}

pub async fn version(ctx: &Ctx, id: &str) -> Result<Version> {
    let url = join_url(&ctx.endpoints.modrinth, &["v2", "version", id])?;
    ctx.http.get_json(&url).await
}

pub async fn project(ctx: &Ctx, id_or_slug: &str) -> Result<Project> {
    let url = join_url(&ctx.endpoints.modrinth, &["v2", "project", id_or_slug])?;
    ctx.http.get_json(&url).await
}

/// Several projects at once.
pub async fn projects(ctx: &Ctx, ids: &[String]) -> Result<Vec<Project>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut url = parse(&join_url(&ctx.endpoints.modrinth, &["v2", "projects"])?)?;
    url.query_pairs_mut().append_pair(
        "ids",
        &serde_json::to_string(ids).expect("strings serialize"),
    );
    ctx.http.get_json(url.as_str()).await
}

async fn post_hashes<T: for<'de> Deserialize<'de>>(
    ctx: &Ctx,
    path: &[&str],
    body: serde_json::Value,
) -> Result<HashMap<String, T>> {
    let url = join_url(&ctx.endpoints.modrinth, path)?;
    let (status, bytes) = ctx.http.post_json(&url, &body).await?;
    if status != 200 {
        return Err(CoreError::HttpStatus { url, status });
    }
    serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
        path: url.into(),
        source,
    })
}

/// Identifies files by SHA-1: hash → version. Unknown files are absent.
pub async fn versions_by_sha1(ctx: &Ctx, hashes: &[String]) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    post_hashes(
        ctx,
        &["v2", "version_files"],
        json!({ "hashes": hashes, "algorithm": "sha1" }),
    )
    .await
}

/// Newest compatible version for each file hash.
pub async fn latest_by_sha1(
    ctx: &Ctx,
    hashes: &[String],
    loaders: &[&str],
    game_version: &str,
) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let mut body = json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "game_versions": [game_version],
    });
    if !loaders.is_empty() {
        body["loaders"] = json!(loaders);
    }
    post_hashes(ctx, &["v2", "version_files", "update"], body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facet_building() {
        let q = SearchQuery {
            query: "sodium".into(),
            project_type: ProjectType::Mod,
            game_version: Some("1.20.1".into()),
            loader: Some("quilt".into()),
            sort: SearchSort::Downloads,
            offset: 0,
        };
        assert_eq!(
            facets(&q),
            r#"[["project_type:mod"],["versions:1.20.1"],["categories:quilt","categories:fabric"]]"#
        );
        let q = SearchQuery {
            game_version: None,
            loader: Some(String::new()),
            project_type: ProjectType::Modpack,
            ..q
        };
        assert_eq!(facets(&q), r#"[["project_type:modpack"]]"#);
    }

    #[test]
    fn parses_search_hits() {
        let raw: RawPage = serde_json::from_str(
            r#"{"hits":[{"project_id":"AANobbMI","project_type":"mod","slug":"sodium","title":"Sodium",
                "description":"fast","author":"jelly","icon_url":"https://cdn.modrinth.com/x.png",
                "downloads":5,"date_modified":"2026-01-01","categories":["fabric"],"extra":1}],
                "offset":0,"limit":20,"total_hits":1}"#,
        )
        .unwrap();
        let page = SearchPage::from(raw);
        assert_eq!(page.hits[0].project_id, "AANobbMI");
        assert_eq!(page.total_hits, 1);
        // Serialised for the UI in camelCase.
        let ui = serde_json::to_value(&page).unwrap();
        assert_eq!(ui["hits"][0]["iconUrl"], "https://cdn.modrinth.com/x.png");
    }
}
