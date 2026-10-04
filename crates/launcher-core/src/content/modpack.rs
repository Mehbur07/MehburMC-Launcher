//! Modpack import: Modrinth `.mrpack` and CurseForge `.zip`.
//!
//! Both create a new instance, download the pack's files into it and copy
//! the `overrides/` folder. A failed import removes the half-built instance.
//!
//! - `.mrpack` (`modrinth.index.json`): files carry their download URLs and
//!   SHA-1/SHA-512; `overrides/` then `client-overrides/` are applied.
//! - CurseForge (`manifest.json`): files are `projectID`/`fileID` pairs
//!   resolved through the CurseForge API with the user's own API key.
//!   Authors can forbid third-party downloads; such files are reported with
//!   a link for manual download.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::archive;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::hash::Checksum;
use crate::instance::{Instance, InstanceStore, LoaderKind, LoaderSpec, NewInstance};
use crate::loader;
use crate::net::download::{DownloadItem, Verify};

const MRPACK_INDEX: &str = "modrinth.index.json";
const CF_MANIFEST: &str = "manifest.json";

/// A file the pack needs but that could not be downloaded automatically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BlockedFile {
    pub name: String,
    /// Page where the user can download it manually.
    pub url: String,
    /// Folder it belongs in (`mods`, `resourcepacks`, …).
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportResult {
    pub instance: Instance,
    pub blocked: Vec<BlockedFile>,
}

fn invalid(reason: impl Into<String>) -> CoreError {
    CoreError::ModpackInvalid(reason.into())
}

fn read_json<T: for<'de> Deserialize<'de>>(zip: &Path, entry: &str) -> Result<T> {
    let bytes =
        archive::read_entry(zip, entry)?.ok_or_else(|| invalid(format!("{entry} missing")))?;
    serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
        path: zip.join(entry),
        source,
    })
}

/// Which kind of pack `zip` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackKind {
    Modrinth,
    CurseForge,
}

pub fn detect(zip: &Path) -> Result<PackKind> {
    let names = archive::entry_names(zip).map_err(|_| invalid("not a zip archive"))?;
    if names.iter().any(|n| n == MRPACK_INDEX) {
        Ok(PackKind::Modrinth)
    } else if names.iter().any(|n| n == CF_MANIFEST) {
        Ok(PackKind::CurseForge)
    } else {
        Err(invalid(
            "neither modrinth.index.json nor manifest.json found",
        ))
    }
}

/// Turns a loader version as written in a pack (`47.2.0`) into our spec
/// value (`1.20.1-47.2.0` for Forge), using the real version list when the
/// network allows it.
async fn loader_spec(ctx: &Ctx, kind: LoaderKind, mc: &str, raw: &str) -> LoaderSpec {
    let version = match kind {
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let listed = loader::list_versions(ctx, kind, mc)
                .await
                .ok()
                .and_then(|l| {
                    l.into_iter()
                        .find(|v| v.id == raw || v.label == raw)
                        .map(|v| v.id)
                });
            listed.unwrap_or_else(|| match kind {
                LoaderKind::Forge => format!("{mc}-{raw}"),
                _ if mc == "1.20.1" && !raw.starts_with("1.20.1-") => format!("1.20.1-{raw}"),
                _ => raw.to_owned(),
            })
        }
        _ => raw.to_owned(),
    };
    LoaderSpec {
        kind,
        version: Some(version),
    }
}

fn safe_name(name: &str) -> String {
    let n: String = name.trim().chars().take(64).collect();
    if n.is_empty() { "Modpack".into() } else { n }
}

/// Creates the instance and runs `fill`; removes the instance if it fails.
async fn with_new_instance<F, Fut>(
    store: &InstanceStore,
    req: NewInstance,
    fill: F,
) -> Result<(Instance, Vec<BlockedFile>)>
where
    F: FnOnce(Instance, PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<BlockedFile>>>,
{
    let inst = store.create(req)?;
    let dir = store.dir(&inst.id)?;
    match fill(inst.clone(), dir).await {
        Ok(blocked) => Ok((store.get(&inst.id)?, blocked)),
        Err(e) => {
            if let Err(del) = store.delete(&inst.id) {
                tracing::warn!(error = %del.detail(), "could not remove failed modpack instance");
            }
            Err(e)
        }
    }
}

fn apply_overrides(zip: &Path, game_dir: &Path, prefixes: &[&str]) -> Result<()> {
    for prefix in prefixes {
        let p = format!("{}/", prefix.trim_end_matches('/'));
        archive::extract_zip_with(zip, game_dir, true, |name| {
            name.strip_prefix(&p)
                .filter(|rest| !rest.is_empty())
                .map(str::to_owned)
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------- mrpack

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    format_version: u32,
    game: String,
    name: String,
    #[serde(default)]
    files: Vec<MrFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<MrEnv>,
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MrEnv {
    #[serde(default)]
    client: Option<String>,
}

/// `(mc, loader kind, loader version)` from `dependencies`.
fn mr_loader(deps: &HashMap<String, String>) -> Result<(String, LoaderKind, Option<String>)> {
    let mc = deps
        .get("minecraft")
        .cloned()
        .ok_or_else(|| invalid("dependencies.minecraft missing"))?;
    for (key, kind) in [
        ("fabric-loader", LoaderKind::Fabric),
        ("quilt-loader", LoaderKind::Quilt),
        ("forge", LoaderKind::Forge),
        ("neoforge", LoaderKind::NeoForge),
    ] {
        if let Some(v) = deps.get(key) {
            return Ok((mc, kind, Some(v.clone())));
        }
    }
    Ok((mc, LoaderKind::Vanilla, None))
}

pub async fn import_mrpack(
    ctx: &Ctx,
    store: &InstanceStore,
    pack: &Path,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<ImportResult> {
    let index: MrIndex = read_json(pack, MRPACK_INDEX)?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(invalid(format!(
            "unsupported format {} / game {}",
            index.format_version, index.game
        )));
    }
    let (mc, kind, raw) = mr_loader(&index.dependencies)?;
    let loader = match &raw {
        Some(v) => loader_spec(ctx, kind, &mc, v).await,
        None => LoaderSpec::default(),
    };
    let req = new_instance(&index.name, &mc, loader);

    let (instance, blocked) = with_new_instance(store, req, |_inst, dir| async move {
        let mut items = Vec::new();
        for f in &index.files {
            if f.env
                .as_ref()
                .and_then(|e| e.client.as_deref())
                .is_some_and(|c| c == "unsupported")
            {
                continue;
            }
            let dest = archive::safe_join(&dir, &f.path)
                .ok_or_else(|| invalid(format!("unsafe path {}", f.path)))?;
            // First mirror that passes the allowlist (spec: Modrinth, GitHub, GitLab).
            let url = f
                .downloads
                .iter()
                .find(|u| ctx.http.allowlist().check(u).is_ok())
                .ok_or_else(|| CoreError::UrlNotAllowed {
                    url: f.downloads.first().cloned().unwrap_or_default(),
                })?;
            let checksum = f
                .hashes
                .get("sha512")
                .map(|h| Checksum::Sha512(h.clone()))
                .or_else(|| f.hashes.get("sha1").map(|h| Checksum::Sha1(h.clone())))
                .ok_or_else(|| invalid(format!("{} has no hash", f.path)))?;
            items.push(DownloadItem {
                url: url.clone(),
                dest,
                checksum: Some(checksum),
                size: f.file_size,
            });
        }
        ctx.downloader()
            .run(items, Verify::Full, progress, cancel)
            .await?;
        let (p, d) = (pack.to_owned(), dir.clone());
        tokio::task::spawn_blocking(move || {
            apply_overrides(&p, &d, &["overrides", "client-overrides"])
        })
        .await
        .expect("overrides task panicked")?;
        Ok(vec![])
    })
    .await?;
    Ok(ImportResult { instance, blocked })
}

fn new_instance(name: &str, mc: &str, loader: LoaderSpec) -> NewInstance {
    NewInstance {
        name: safe_name(name),
        icon: Some("cube".into()),
        mc_version: mc.to_owned(),
        loader: Some(loader),
        java_path: None,
        memory_mb: None,
        jvm_args: None,
        resolution: None,
        fullscreen: None,
    }
}

// ---------------------------------------------------------------- CurseForge

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Debug, Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct CfList<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    mod_id: u64,
    file_name: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    file_length: Option<u64>,
    #[serde(default)]
    hashes: Vec<CfHash>,
}

#[derive(Debug, Deserialize)]
struct CfHash {
    value: String,
    algo: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    #[serde(default)]
    class_id: Option<u64>,
    #[serde(default)]
    links: Option<CfLinks>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    #[serde(default)]
    website_url: Option<String>,
}

/// `forge-47.2.0` → (Forge, `47.2.0`).
fn cf_loader(id: &str) -> Option<(LoaderKind, String)> {
    let (name, version) = id.split_once('-')?;
    let kind = match name {
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        _ => return None,
    };
    Some((kind, version.to_owned()))
}

/// CurseForge class id → instance folder.
fn cf_folder(class_id: Option<u64>) -> &'static str {
    match class_id {
        Some(12) => "resourcepacks",
        Some(6552) => "shaderpacks",
        _ => "mods",
    }
}

async fn cf_post<T: for<'de> Deserialize<'de>>(
    ctx: &Ctx,
    key: &str,
    path: &str,
    body: serde_json::Value,
) -> Result<Vec<T>> {
    let url = format!("{}{path}", ctx.endpoints.curseforge);
    let (status, bytes) = ctx
        .http
        .request_json(
            reqwest::Method::POST,
            &url,
            Some(&body),
            &[("x-api-key", key)],
        )
        .await?;
    match status {
        200 => {}
        401 | 403 => return Err(CoreError::CurseForgeKey),
        s => return Err(CoreError::HttpStatus { url, status: s }),
    }
    let list: CfList<T> = serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
        path: url.into(),
        source,
    })?;
    Ok(list.data)
}

pub async fn import_curseforge(
    ctx: &Ctx,
    store: &InstanceStore,
    pack: &Path,
    api_key: Option<&str>,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<ImportResult> {
    let manifest: CfManifest = read_json(pack, CF_MANIFEST)?;
    let mc = manifest.minecraft.version.clone();
    let loader_id = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .map(|l| l.id.clone());
    let loader = match loader_id.as_deref().and_then(cf_loader) {
        Some((kind, v)) => loader_spec(ctx, kind, &mc, &v).await,
        None => LoaderSpec::default(),
    };
    let files: Vec<&CfManifestFile> = manifest.files.iter().filter(|f| f.required).collect();
    let key = match api_key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => Some(k.to_owned()),
        None if files.is_empty() => None,
        None => return Err(CoreError::CurseForgeKey),
    };
    let overrides = manifest
        .overrides
        .clone()
        .unwrap_or_else(|| "overrides".into());
    let req = new_instance(&manifest.name, &mc, loader);

    let (instance, blocked) = with_new_instance(store, req, |_inst, dir| async move {
        let mut blocked = Vec::new();
        if let Some(key) = &key {
            let file_ids: Vec<u64> = files.iter().map(|f| f.file_id).collect();
            let mod_ids: Vec<u64> = files.iter().map(|f| f.project_id).collect();
            let cf_files: Vec<CfFile> =
                cf_post(ctx, key, "/v1/mods/files", json!({ "fileIds": file_ids })).await?;
            let mods: HashMap<u64, CfMod> =
                cf_post::<CfMod>(ctx, key, "/v1/mods", json!({ "modIds": mod_ids }))
                    .await?
                    .into_iter()
                    .map(|m| (m.id, m))
                    .collect();
            let mut items = Vec::new();
            for f in cf_files {
                let m = mods.get(&f.mod_id);
                let folder = cf_folder(m.and_then(|m| m.class_id));
                let name = crate::instance::files::checked_name(&f.file_name)?.to_owned();
                match f.download_url.filter(|u| !u.is_empty()) {
                    Some(url) => items.push(DownloadItem {
                        url,
                        dest: dir.join(folder).join(&name),
                        checksum: f
                            .hashes
                            .iter()
                            .find(|h| h.algo == 1)
                            .map(|h| Checksum::Sha1(h.value.clone())),
                        size: f.file_length,
                    }),
                    None => {
                        let page = m
                            .and_then(|m| m.links.as_ref())
                            .and_then(|l| l.website_url.clone())
                            .map(|u| format!("{}/files/{}", u.trim_end_matches('/'), f.id))
                            .unwrap_or_else(|| "https://www.curseforge.com/minecraft".into());
                        blocked.push(BlockedFile {
                            name,
                            url: page,
                            folder: folder.into(),
                        });
                    }
                }
            }
            ctx.downloader()
                .run(items, Verify::Full, progress, cancel)
                .await?;
        }
        let (p, d) = (pack.to_owned(), dir.clone());
        tokio::task::spawn_blocking(move || apply_overrides(&p, &d, &[overrides.as_str()]))
            .await
            .expect("overrides task panicked")?;
        Ok(blocked)
    })
    .await?;
    if !blocked.is_empty() {
        tracing::warn!(
            count = blocked.len(),
            "some CurseForge files need a manual download"
        );
    }
    Ok(ImportResult { instance, blocked })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::{NullSink, Stage};
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    fn sha1(b: &[u8]) -> String {
        hex::encode(<sha1::Sha1 as sha1::Digest>::digest(b))
    }

    fn zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, d) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap();
    }

    async fn setup(server: &MockServer) -> (tempfile::TempDir, Ctx, InstanceStore) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths.clone(), Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.curseforge = server.uri();
        // Loader lists are unreachable in tests: fall back to formatting.
        ctx.endpoints.forge_files = "http://127.0.0.1:1".into();
        (tmp, ctx, InstanceStore::new(paths))
    }

    fn progress() -> Progress {
        Progress::new(Arc::new(NullSink), "t", Stage::Content)
    }

    #[tokio::test]
    async fn imports_mrpack_with_overrides() {
        let server = MockServer::builder().start().await;
        Mock::given(method("GET"))
            .and(path("/data/x/sodium.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jar".to_vec()))
            .mount(&server)
            .await;
        let (tmp, ctx, store) = setup(&server).await;
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Test Pack",
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.16.0"},
            "files": [
                {"path": "mods/sodium.jar", "hashes": {"sha1": sha1(b"jar")},
                 "downloads": [format!("{}/data/x/sodium.jar", server.uri())], "fileSize": 3},
                {"path": "mods/server-only.jar", "hashes": {"sha1": "00"}, "env": {"client": "unsupported", "server": "required"},
                 "downloads": ["https://cdn.modrinth.com/x.jar"], "fileSize": 1}
            ]
        });
        let pack = tmp.path().join("p.mrpack");
        zip(
            &pack,
            &[
                (MRPACK_INDEX, index.to_string().as_bytes()),
                ("overrides/config/a.txt", b"base"),
                ("client-overrides/config/a.txt", b"client!"),
                ("overrides/options.txt", b"o"),
            ],
        );
        assert_eq!(detect(&pack).unwrap(), PackKind::Modrinth);
        let r = import_mrpack(&ctx, &store, &pack, &progress(), &CancellationToken::new())
            .await
            .unwrap();
        let i = &r.instance;
        assert_eq!(i.name, "Test Pack");
        assert_eq!(i.mc_version, "1.20.1");
        assert_eq!(i.loader.kind, LoaderKind::Fabric);
        assert_eq!(i.loader.version.as_deref(), Some("0.16.0"));
        let dir = store.dir(&i.id).unwrap();
        assert_eq!(std::fs::read(dir.join("mods/sodium.jar")).unwrap(), b"jar");
        assert!(!dir.join("mods/server-only.jar").exists());
        assert_eq!(std::fs::read(dir.join("config/a.txt")).unwrap(), b"client!");
        assert!(dir.join("options.txt").is_file());
    }

    #[tokio::test]
    async fn failed_mrpack_leaves_no_instance() {
        let server = MockServer::builder().start().await;
        let (tmp, ctx, store) = setup(&server).await;
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Evil",
            "dependencies": {"minecraft": "1.20.1"},
            "files": [{"path": "../../escape.jar", "hashes": {"sha1": "00"},
                       "downloads": ["https://cdn.modrinth.com/x.jar"]}]
        });
        let pack = tmp.path().join("evil.mrpack");
        zip(&pack, &[(MRPACK_INDEX, index.to_string().as_bytes())]);
        let e = import_mrpack(&ctx, &store, &pack, &progress(), &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(e.code(), "content.modpackInvalid");
        assert!(store.list().is_empty());
    }

    #[tokio::test]
    async fn imports_curseforge_and_reports_blocked_files() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/v1/mods/files"))
            .and(header("x-api-key", "k"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                {"id": 11, "modId": 1, "fileName": "jei.jar", "fileLength": 3,
                 "downloadUrl": format!("{}/files/jei.jar", server.uri()),
                 "hashes": [{"value": sha1(b"jei"), "algo": 1}, {"value": "md5", "algo": 2}]},
                {"id": 22, "modId": 2, "fileName": "Faithful.zip", "downloadUrl": null, "hashes": []}
            ]})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/mods"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                {"id": 1, "classId": 6, "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei"}},
                {"id": 2, "classId": 12, "links": {"websiteUrl": "https://www.curseforge.com/minecraft/texture-packs/faithful/"}}
            ]})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/jei.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jei".to_vec()))
            .mount(&server)
            .await;
        let (tmp, ctx, store) = setup(&server).await;
        let manifest = json!({
            "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]},
            "manifestType": "minecraftModpack", "name": "CF Pack", "overrides": "overrides",
            "files": [{"projectID": 1, "fileID": 11, "required": true},
                      {"projectID": 2, "fileID": 22, "required": true},
                      {"projectID": 3, "fileID": 33, "required": false}]
        });
        let pack = tmp.path().join("cf.zip");
        zip(
            &pack,
            &[
                (CF_MANIFEST, manifest.to_string().as_bytes()),
                ("overrides/config/jei.toml", b"x"),
            ],
        );
        assert_eq!(detect(&pack).unwrap(), PackKind::CurseForge);

        let no_key = import_curseforge(
            &ctx,
            &store,
            &pack,
            None,
            &progress(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(no_key.code(), "content.curseforgeKey");

        let r = import_curseforge(
            &ctx,
            &store,
            &pack,
            Some("k"),
            &progress(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(r.instance.loader.kind, LoaderKind::Forge);
        assert_eq!(r.instance.loader.version.as_deref(), Some("1.20.1-47.2.0"));
        let dir = store.dir(&r.instance.id).unwrap();
        assert_eq!(std::fs::read(dir.join("mods/jei.jar")).unwrap(), b"jei");
        assert!(dir.join("config/jei.toml").is_file());
        assert_eq!(
            r.blocked,
            [BlockedFile {
                name: "Faithful.zip".into(),
                url: "https://www.curseforge.com/minecraft/texture-packs/faithful/files/22".into(),
                folder: "resourcepacks".into()
            }]
        );
    }

    #[test]
    fn loader_ids() {
        assert_eq!(
            cf_loader("forge-47.2.0"),
            Some((LoaderKind::Forge, "47.2.0".into()))
        );
        assert_eq!(
            cf_loader("neoforge-21.1.1"),
            Some((LoaderKind::NeoForge, "21.1.1".into()))
        );
        assert_eq!(cf_loader("liteloader-1"), None);
        let mut d = HashMap::new();
        d.insert("minecraft".to_owned(), "26.3".to_owned());
        d.insert("neoforge".to_owned(), "26.3.0.46-beta".to_owned());
        let (mc, k, v) = mr_loader(&d).unwrap();
        assert_eq!(
            (mc.as_str(), k, v.as_deref()),
            ("26.3", LoaderKind::NeoForge, Some("26.3.0.46-beta"))
        );
    }
}
