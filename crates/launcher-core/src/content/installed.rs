//! Installed content: identifies files in `mods/`, `resourcepacks/` and
//! `shaderpacks/` by SHA-1 on Modrinth, checks for updates and applies them.
//!
//! Hashes are cached in `cache/content-hashes.json` keyed by path, size and
//! modification time, so a large mods folder is hashed only once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::install::{self, modrinth_loaders};
use super::metadata;
use super::modrinth::{self, Version};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;
use crate::hash::sha1_file;
use crate::instance::Instance;
use crate::instance::files::{DISABLED_SUFFIX, Folder, checked_name};
use crate::net::download::Verify;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateInfo {
    pub version_id: String,
    pub version_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledItem {
    pub file_name: String,
    pub enabled: bool,
    #[ts(type = "number")]
    pub size: u64,
    /// Set when Modrinth knows the file.
    pub project_id: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
    /// A newer compatible version (only when updates were checked).
    pub update: Option<UpdateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    mtime: u64,
    sha1: String,
}

/// Serialises access to the hash cache file.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

fn cache_file(ctx: &Ctx) -> PathBuf {
    ctx.paths.cache().join("content-hashes.json")
}

/// Content files of a folder (jar/zip, enabled or disabled).
fn content_files(dir: &Path) -> Vec<(String, PathBuf, u64, u64)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<_> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let base = name.strip_suffix(DISABLED_SUFFIX).unwrap_or(&name);
            let lower = base.to_ascii_lowercase();
            if name.starts_with('.') || !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
                return None;
            }
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            Some((name, e.path(), meta.len(), mtime))
        })
        .collect();
    out.sort_by_key(|f| f.0.to_lowercase());
    out
}

/// SHA-1 of every content file in `dir` (file name → hash), using the cache.
pub fn hashes(ctx: &Ctx, dir: &Path) -> Result<Vec<(String, String, u64)>> {
    let files = content_files(dir);
    let _g = CACHE_LOCK.lock().expect("hash cache");
    let path = cache_file(ctx);
    let mut cache: HashMap<String, CacheEntry> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut dirty = false;
    let mut out = Vec::with_capacity(files.len());
    for (name, p, size, mtime) in files {
        let key = p.display().to_string();
        let sha1 = match cache.get(&key) {
            Some(c) if c.size == size && c.mtime == mtime => c.sha1.clone(),
            _ => {
                let h = sha1_file(&p)?;
                cache.insert(
                    key,
                    CacheEntry {
                        size,
                        mtime,
                        sha1: h.clone(),
                    },
                );
                dirty = true;
                h
            }
        };
        out.push((name, sha1, size));
    }
    if dirty {
        // Forget files that no longer exist.
        cache.retain(|k, _| Path::new(k).is_file());
        write_json_atomic(&path, &cache)?;
    }
    Ok(out)
}

async fn hashes_async(ctx: &Ctx, dir: &Path) -> Result<Vec<(String, String, u64)>> {
    let (c, d) = (ctx.clone(), dir.to_owned());
    tokio::task::spawn_blocking(move || hashes(&c, &d))
        .await
        .expect("hash task panicked")
}

/// Modrinth versions of the files in `dir` (file name → version).
pub async fn identify(ctx: &Ctx, dir: &Path) -> Result<HashMap<String, Version>> {
    let files = hashes_async(ctx, dir).await?;
    let list: Vec<String> = files.iter().map(|f| f.1.clone()).collect();
    let mut by_hash = modrinth::versions_by_sha1(ctx, &list).await?;
    Ok(files
        .into_iter()
        .filter_map(|(name, h, _)| by_hash.remove(&h).map(|v| (name, v)))
        .collect())
}

/// Version filter used for update checks of a folder.
fn update_loaders(folder: Folder, inst: &Instance) -> Vec<&'static str> {
    match folder {
        Folder::Mods => modrinth_loaders(inst.loader.kind).to_vec(),
        Folder::ResourcePacks => vec!["minecraft"],
        _ => vec![],
    }
}

fn check_folder(folder: Folder) -> Result<()> {
    if matches!(
        folder,
        Folder::Mods | Folder::ResourcePacks | Folder::ShaderPacks
    ) {
        Ok(())
    } else {
        Err(CoreError::InvalidInstance(format!(
            "{folder:?} has no managed content"
        )))
    }
}

/// Lists a content folder with Modrinth metadata. Mods Modrinth does not
/// know (or all of them offline) get name, version and icon from the jar.
pub async fn scan(
    ctx: &Ctx,
    inst: &Instance,
    game_dir: &Path,
    folder: Folder,
    check_updates: bool,
) -> Result<Vec<InstalledItem>> {
    let mut items = scan_remote(ctx, inst, game_dir, folder, check_updates).await?;
    if folder == Folder::Mods {
        let dir = game_dir.join(folder.dir_name());
        let unknown: Vec<(usize, PathBuf)> = items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.project_id.is_none())
            .map(|(n, i)| (n, dir.join(&i.file_name)))
            .collect();
        let metas = tokio::task::spawn_blocking(move || {
            unknown
                .into_iter()
                .filter_map(|(n, p)| metadata::read(&p).map(|m| (n, m)))
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        for (n, m) in metas {
            let item = &mut items[n];
            item.title = m.name;
            item.version_number = m.version;
            item.icon_url = m.icon;
        }
    }
    Ok(items)
}

async fn scan_remote(
    ctx: &Ctx,
    inst: &Instance,
    game_dir: &Path,
    folder: Folder,
    check_updates: bool,
) -> Result<Vec<InstalledItem>> {
    check_folder(folder)?;
    let dir = game_dir.join(folder.dir_name());
    let files = hashes_async(ctx, &dir).await?;
    let mut items: Vec<InstalledItem> = files
        .iter()
        .map(|(name, _, size)| InstalledItem {
            file_name: name.clone(),
            enabled: !name.ends_with(DISABLED_SUFFIX),
            size: *size,
            project_id: None,
            slug: None,
            title: None,
            version_number: None,
            icon_url: None,
            update: None,
        })
        .collect();
    if files.is_empty() {
        return Ok(items);
    }

    let hashes: Vec<String> = files.iter().map(|f| f.1.clone()).collect();
    let known = match modrinth::versions_by_sha1(ctx, &hashes).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(error = %e.detail(), "content metadata unavailable");
            return Ok(items);
        }
    };
    let ids: Vec<String> = {
        let mut v: Vec<String> = known.values().map(|v| v.project_id.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let projects: HashMap<String, modrinth::Project> = modrinth::projects(ctx, &ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let latest = if check_updates {
        modrinth::latest_by_sha1(
            ctx,
            &hashes,
            &update_loaders(folder, inst),
            &inst.mc_version,
        )
        .await?
    } else {
        HashMap::new()
    };

    for (item, h) in items.iter_mut().zip(&hashes) {
        let Some(v) = known.get(h) else { continue };
        let p = projects.get(&v.project_id);
        item.project_id = Some(v.project_id.clone());
        item.slug = p.map(|p| p.slug.clone());
        item.title = p.map(|p| p.title.clone());
        item.icon_url = p.and_then(|p| p.icon_url.clone());
        item.version_number = Some(v.version_number.clone());
        if let Some(new) = latest.get(h).filter(|n| n.id != v.id) {
            item.update = Some(UpdateInfo {
                version_id: new.id.clone(),
                version_number: new.version_number.clone(),
            });
        }
    }
    Ok(items)
}

/// Replaces the given files with their newest compatible versions. A
/// disabled file stays disabled. Returns the new file names.
pub async fn update(
    ctx: &Ctx,
    inst: &Instance,
    game_dir: &Path,
    folder: Folder,
    file_names: &[String],
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    check_folder(folder)?;
    let dir = game_dir.join(folder.dir_name());
    let files = hashes_async(ctx, &dir).await?;
    let wanted: Vec<&(String, String, u64)> =
        files.iter().filter(|f| file_names.contains(&f.0)).collect();
    let hashes: Vec<String> = wanted.iter().map(|f| f.1.clone()).collect();
    let latest = modrinth::latest_by_sha1(
        ctx,
        &hashes,
        &update_loaders(folder, inst),
        &inst.mc_version,
    )
    .await?;

    let mut replaced = Vec::new();
    let mut items = Vec::new();
    for (old_name, h, _) in wanted {
        let Some(new) = latest.get(h) else { continue };
        let Some(file) = new.primary_file() else {
            continue;
        };
        let disabled = old_name.ends_with(DISABLED_SUFFIX);
        let mut new_name = checked_name(&file.filename)?.to_owned();
        if disabled {
            new_name.push_str(DISABLED_SUFFIX);
        }
        if &new_name == old_name {
            continue;
        }
        items.push(install::download_item(&dir.join(&new_name), file));
        replaced.push((old_name.clone(), new_name));
    }
    ctx.downloader()
        .run(items, Verify::Full, &install::quiet_progress(), cancel)
        .await?;
    let mut out = Vec::new();
    for (old, new) in replaced {
        let p = dir.join(&old);
        std::fs::remove_file(&p).map_err(|e| CoreError::io(&p, e))?;
        tracing::info!(%old, %new, "content updated");
        out.push(new);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::NullSink;
    use crate::instance::{LoaderKind, LoaderSpec};
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    fn sha1(b: &[u8]) -> String {
        hex::encode(<sha1::Sha1 as sha1::Digest>::digest(b))
    }

    fn sha512(b: &[u8]) -> String {
        hex::encode(<sha2::Sha512 as sha2::Digest>::digest(b))
    }

    fn inst() -> Instance {
        serde_json::from_value(json!({
            "name": "t", "mcVersion": "1.20.1",
            "loader": LoaderSpec { kind: LoaderKind::Fabric, version: None }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn scan_identifies_and_updates() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.modrinth = server.uri();

        let game = tmp.path().join("game");
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("sodium-0.5.jar.disabled"), b"old sodium").unwrap();
        std::fs::write(mods.join("unknown.jar"), b"mystery").unwrap();
        std::fs::write(mods.join("notes.txt"), b"ignored").unwrap();
        let old = sha1(b"old sodium");
        let new_body = b"new sodium";

        let version = |id: &str, number: &str, file: &str, body: &[u8]| {
            json!({"id": id, "project_id": "AANobbMI", "version_number": number,
                   "version_type": "release", "files": [{
                       "url": format!("{}/dl/{file}", server.uri()), "filename": file, "primary": true,
                       "hashes": {"sha1": sha1(body), "sha512": sha512(body)}, "size": body.len()}]})
        };
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ old.clone(): version("v1", "0.5", "sodium-0.5.jar", b"old sodium") }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v2/version_files/update"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ old.clone(): version("v2", "0.6", "sodium-0.6.jar", new_body) }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "AANobbMI", "slug": "sodium", "title": "Sodium", "icon_url": "https://cdn.modrinth.com/i.png"}
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/dl/sodium-0.6.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(new_body.to_vec()))
            .mount(&server)
            .await;

        let items = scan(&ctx, &inst(), &game, Folder::Mods, true)
            .await
            .unwrap();
        assert_eq!(items.len(), 2, "txt files are not content");
        let s = items
            .iter()
            .find(|i| i.file_name.starts_with("sodium"))
            .unwrap();
        assert_eq!(s.title.as_deref(), Some("Sodium"));
        assert!(!s.enabled);
        assert_eq!(s.update.as_ref().unwrap().version_number, "0.6");
        let u = items.iter().find(|i| i.file_name == "unknown.jar").unwrap();
        assert!(u.project_id.is_none() && u.update.is_none());
        // Hash cache written.
        assert!(cache_file(&ctx).is_file());

        let new = update(
            &ctx,
            &inst(),
            &game,
            Folder::Mods,
            &["sodium-0.5.jar.disabled".into()],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(new, ["sodium-0.6.jar.disabled"]);
        assert!(!mods.join("sodium-0.5.jar.disabled").exists());
        assert_eq!(
            std::fs::read(mods.join("sodium-0.6.jar.disabled")).unwrap(),
            new_body
        );

        assert!(
            scan(&ctx, &inst(), &game, Folder::Saves, false)
                .await
                .is_err()
        );
    }
}
