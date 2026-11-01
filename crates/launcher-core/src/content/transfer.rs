//! Transfers all mods of a profile into another one (K99).
//!
//! When the target runs the same game with the same loader the files are
//! copied as they are. Otherwise each mod's version for the target is looked
//! up on Modrinth and downloaded with its required dependencies. A jar is
//! never copied across versions: it would stop the game from starting.
//!
//! The target only ever gains files. A mod it already has stays as it is, in
//! whichever version and state.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::install::{self, modrinth_loaders, pick, present_by_name};
use super::modrinth::{self, ProjectType, SearchQuery, SearchSort, Version};
use super::{installed, metadata};
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::instance::files::{DISABLED_SUFFIX, Folder, checked_name};
use crate::instance::{Instance, LoaderKind};
use crate::net::download::{DownloadItem, Verify, part_path};

/// Modrinth requests at once.
const LOOKUPS: usize = 8;
/// Safety net against runaway dependency trees.
const MAX_DEPENDENCIES: usize = 512;
/// Modrinth allows 300 requests a minute; a big folder can reach that.
const RATE_LIMIT_RETRIES: u32 = 3;
#[cfg(not(test))]
const RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
const RATE_LIMIT_WAIT: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferResult {
    /// Mods now in the target.
    pub transferred: Vec<String>,
    /// Required dependencies that came along.
    pub dependencies: Vec<String>,
    /// The target already has these; left untouched.
    pub already_present: Vec<String>,
    /// No version for the target was found; not transferred.
    pub not_found: Vec<String>,
    /// Required by a transferred mod but not available for the target.
    pub missing_dependencies: Vec<String>,
}

impl TransferResult {
    fn sort(&mut self) {
        for list in [
            &mut self.transferred,
            &mut self.dependencies,
            &mut self.already_present,
            &mut self.not_found,
            &mut self.missing_dependencies,
        ] {
            list.sort_by_key(|s| s.to_lowercase());
            list.dedup();
        }
    }
}

struct LocalMod {
    file: String,
    path: PathBuf,
    sha1: String,
    size: u64,
    id: Option<String>,
    name: Option<String>,
}

impl LocalMod {
    fn base(&self) -> &str {
        self.file
            .strip_suffix(DISABLED_SUFFIX)
            .unwrap_or(&self.file)
    }

    fn disabled(&self) -> bool {
        self.file.ends_with(DISABLED_SUFFIX)
    }

    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.base().to_owned())
    }
}

async fn read_mods(ctx: &Ctx, dir: &Path) -> Result<Vec<LocalMod>> {
    let (ctx, dir) = (ctx.clone(), dir.to_owned());
    tokio::task::spawn_blocking(move || {
        Ok(installed::hashes(&ctx, &dir)?
            .into_iter()
            .map(|(file, sha1, size)| {
                let path = dir.join(&file);
                let meta = metadata::read_light(&path).unwrap_or_default();
                LocalMod {
                    file,
                    path,
                    sha1,
                    size,
                    id: meta.id,
                    name: meta.name,
                }
            })
            .collect())
    })
    .await
    .expect("mod scan panicked")
}

/// What the target folder already holds.
#[derive(Default)]
struct Present {
    sha1s: HashSet<String>,
    ids: HashSet<String>,
    names: HashSet<String>,
    projects: HashSet<String>,
}

impl Present {
    fn of(mods: &[LocalMod]) -> Self {
        Self {
            sha1s: mods.iter().map(|m| m.sha1.clone()).collect(),
            ids: mods
                .iter()
                .filter_map(|m| m.id.as_deref())
                .map(str::to_ascii_lowercase)
                .collect(),
            names: mods.iter().map(|m| m.base().to_ascii_lowercase()).collect(),
            projects: HashSet::new(),
        }
    }

    fn has(&self, m: &LocalMod) -> bool {
        self.sha1s.contains(&m.sha1)
            || self.names.contains(&m.base().to_ascii_lowercase())
            || m.id
                .as_deref()
                .is_some_and(|id| self.ids.contains(&id.to_ascii_lowercase()))
    }
}

/// Whether the source's jars run on the target as they are.
fn runs_unchanged(source: &Instance, target: &Instance) -> bool {
    let (from, to) = (source.loader.kind, target.loader.kind);
    // Quilt runs Fabric mods.
    source.mc_version == target.mc_version
        && (from == to || (from == LoaderKind::Fabric && to == LoaderKind::Quilt))
}

/// Transfers every mod of `source` into `target` (see the module docs).
pub async fn transfer(
    ctx: &Ctx,
    source: &Instance,
    source_dir: &Path,
    target: &Instance,
    target_dir: &Path,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<TransferResult> {
    let loaders = modrinth_loaders(target.loader.kind);
    if loaders.is_empty() {
        return Err(CoreError::AddonUnavailable {
            project: "mods".into(),
            loader: crate::loader::display_name(target.loader.kind).into(),
            mc: target.mc_version.clone(),
        });
    }
    let dir = target_dir.join(Folder::Mods.dir_name());
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| CoreError::io(&dir, e))?;
    let mut mods = read_mods(ctx, &source_dir.join(Folder::Mods.dir_name())).await?;
    // Enabled mods first: their dependencies should come enabled.
    mods.sort_by_key(LocalMod::disabled);
    let existing = read_mods(ctx, &dir).await?;
    let present = Present::of(&existing);

    let mut result = if runs_unchanged(source, target) {
        copy_mods(&mods, &dir, &present, progress, cancel).await?
    } else {
        let search = Search {
            ctx,
            mc: &target.mc_version,
            loaders,
            dir: &dir,
            progress,
            cancel,
            present,
            done: HashSet::new(),
            result: TransferResult::default(),
            missing: Vec::new(),
        };
        search.run(&mods, &existing).await?
    };
    result.sort();
    tracing::info!(
        source = %source.id,
        target = %target.id,
        transferred = result.transferred.len(),
        dependencies = result.dependencies.len(),
        not_found = result.not_found.len(),
        "mods transferred"
    );
    Ok(result)
}

async fn copy_mods(
    mods: &[LocalMod],
    dir: &Path,
    present: &Present,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<TransferResult> {
    let mut result = TransferResult::default();
    let (copy, have): (Vec<&LocalMod>, Vec<&LocalMod>) = mods.iter().partition(|m| !present.has(m));
    result.already_present = have.iter().map(|m| m.label()).collect();
    progress.set_totals(copy.len() as u64, copy.iter().map(|m| m.size).sum());
    for m in copy {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        // Through a temporary name: a cut copy must not look like a mod.
        let dest = dir.join(&m.file);
        let part = part_path(&dest);
        tokio::fs::copy(&m.path, &part)
            .await
            .map_err(|e| CoreError::io(&m.path, e))?;
        tokio::fs::rename(&part, &dest)
            .await
            .map_err(|e| CoreError::io(&dest, e))?;
        progress.add_bytes(m.size);
        progress.item_done();
        result.transferred.push(m.label());
    }
    progress.finish();
    Ok(result)
}

/// Waits and retries while Modrinth reports its rate limit.
async fn patient<T, F, Fut>(cancel: &CancellationToken, mut call: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    for _ in 0..RATE_LIMIT_RETRIES {
        match call().await {
            Err(CoreError::HttpStatus { status: 429, .. }) => {
                tracing::warn!(wait = ?RATE_LIMIT_WAIT, "Modrinth rate limit reached");
                tokio::select! {
                    () = cancel.cancelled() => return Err(CoreError::Cancelled),
                    () = tokio::time::sleep(RATE_LIMIT_WAIT) => {}
                }
            }
            other => return other,
        }
    }
    call().await
}

/// Lower-case letters and digits only: `Fabric API` and `fabric-api` match.
fn normalized(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A project to find a version of.
struct Wanted {
    project: Option<String>,
    /// The version a dependency names.
    pinned: Option<String>,
    /// Name of the source mod; `None` for a dependency.
    root: Option<String>,
    disabled: bool,
}

/// Required dependencies of a chosen version.
fn required(version: &Version, disabled: bool) -> impl Iterator<Item = Wanted> + '_ {
    version
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
        .filter(|d| d.project_id.is_some() || d.version_id.is_some())
        .map(move |d| Wanted {
            project: d.project_id.clone(),
            pinned: d.version_id.clone(),
            root: None,
            disabled,
        })
}

enum Planned {
    /// Goes to this path.
    Queued(PathBuf, DownloadItem),
    /// A file with this name is already in the folder.
    NameTaken,
    /// The version offers nothing to install.
    Unusable,
}

struct Search<'a> {
    ctx: &'a Ctx,
    mc: &'a str,
    loaders: &'a [&'a str],
    dir: &'a Path,
    progress: &'a Progress,
    cancel: &'a CancellationToken,
    present: Present,
    /// Projects handled so far.
    done: HashSet<String>,
    result: TransferResult,
    /// Project or version ids of dependencies that could not be found.
    missing: Vec<String>,
}

impl Search<'_> {
    async fn run(mut self, mods: &[LocalMod], existing: &[LocalMod]) -> Result<TransferResult> {
        let cancel = self.cancel;
        let ctx = self.ctx;
        let hashes: Vec<String> = existing.iter().map(|m| m.sha1.clone()).collect();
        self.present.projects = patient(cancel, || modrinth::versions_by_sha1(ctx, &hashes))
            .await?
            .into_values()
            .map(|v| v.project_id)
            .collect();
        let hashes: Vec<String> = mods.iter().map(|m| m.sha1.clone()).collect();
        let known = patient(cancel, || modrinth::versions_by_sha1(ctx, &hashes)).await?;

        let mut round = Vec::new();
        let mut unknown = Vec::new();
        for m in mods {
            let project = known.get(&m.sha1).map(|v| &v.project_id);
            if self.present.has(m) || project.is_some_and(|p| self.present.projects.contains(p)) {
                self.result.already_present.push(m.label());
            } else if let Some(p) = project {
                round.push(Wanted {
                    project: Some(p.clone()),
                    pinned: None,
                    root: Some(m.label()),
                    disabled: m.disabled(),
                });
            } else if m.id.is_some() {
                unknown.push(m);
            } else {
                // Without a mod id the file found could not be verified.
                self.result.not_found.push(m.label());
            }
        }
        round.extend(self.transfer_by_name(unknown).await?);

        let mut items = Vec::new();
        while !round.is_empty() {
            round = self.resolve_round(round, &mut items).await?;
        }
        self.ctx
            .downloader()
            .run(items, Verify::Full, self.progress, self.cancel)
            .await?;
        self.name_missing().await;
        Ok(self.result)
    }

    /// Mods Modrinth does not know by hash (another release of the jar, a
    /// CurseForge build). Their project is found by name, so the download is
    /// kept only if it really is the same mod. Returns their dependencies.
    async fn transfer_by_name(&mut self, unknown: Vec<&LocalMod>) -> Result<Vec<Wanted>> {
        let this = &*self;
        // Futures listed first: a closure inside the stream makes the whole
        // transfer future unusable as a Tauri command (not general enough).
        let lookups: Vec<_> = unknown
            .into_iter()
            .map(|m| async move { Ok::<_, CoreError>((m, this.find_by_name(m).await?)) })
            .collect();
        let found: Vec<(&LocalMod, Option<Version>)> = stream::iter(lookups)
            .buffered(LOOKUPS)
            .try_collect()
            .await?;

        let mut queued = Vec::new();
        let mut items = Vec::new();
        for (m, version) in found {
            let Some(version) = version else {
                self.result.not_found.push(m.label());
                continue;
            };
            if self.present.projects.contains(&version.project_id) {
                self.result.already_present.push(m.label());
                continue;
            }
            if !self.done.insert(version.project_id.clone()) {
                continue;
            }
            match self.plan_file(&version, m.disabled()) {
                Planned::Queued(dest, item) => {
                    items.push(item);
                    queued.push((m, version, dest));
                }
                Planned::NameTaken => self.result.already_present.push(m.label()),
                Planned::Unusable => self.result.not_found.push(m.label()),
            }
        }
        self.ctx
            .downloader()
            .run(items, Verify::Full, self.progress, self.cancel)
            .await?;

        let mut deps = Vec::new();
        for (m, version, dest) in queued {
            let id = {
                let dest = dest.clone();
                tokio::task::spawn_blocking(move || metadata::read_light(&dest))
                    .await
                    .expect("mod read panicked")
                    .and_then(|meta| meta.id)
            };
            if id.is_some_and(|id| m.id.as_deref().is_some_and(|i| i.eq_ignore_ascii_case(&id))) {
                self.result.transferred.push(m.label());
                deps.extend(required(&version, m.disabled()));
            } else {
                tracing::info!(file = %m.file, found = %version.project_id, "same name, another mod");
                tokio::fs::remove_file(&dest)
                    .await
                    .map_err(|e| CoreError::io(&dest, e))?;
                self.done.remove(&version.project_id);
                self.result.not_found.push(m.label());
            }
        }
        Ok(deps)
    }

    /// The target's version of the project whose slug or title matches `m`.
    async fn find_by_name(&self, m: &LocalMod) -> Result<Option<Version>> {
        let Some(id) = m.id.as_deref() else {
            return Ok(None);
        };
        let name = m.name.as_deref().unwrap_or(id);
        let query = SearchQuery {
            query: name.to_owned(),
            project_type: ProjectType::Mod,
            game_version: Some(self.mc.to_owned()),
            loader: self.loaders.first().map(|l| (*l).to_owned()),
            sort: SearchSort::Relevance,
            offset: 0,
        };
        let page = patient(self.cancel, || modrinth::search(self.ctx, &query)).await?;
        let (id, name) = (normalized(id), normalized(name));
        let Some(hit) = page
            .hits
            .iter()
            .find(|h| normalized(&h.slug) == id || normalized(&h.title) == name)
        else {
            return Ok(None);
        };
        self.versions_of(&hit.project_id).await
    }

    async fn versions_of(&self, project: &str) -> Result<Option<Version>> {
        let versions = patient(self.cancel, || {
            modrinth::project_versions(self.ctx, project, self.loaders, Some(self.mc))
        })
        .await?;
        Ok(pick(versions))
    }

    /// The version to install, or `None` if the target has no such version.
    async fn lookup(&self, wanted: &Wanted) -> Result<Option<Version>> {
        if let Some(id) = &wanted.pinned {
            match patient(self.cancel, || modrinth::version(self.ctx, id)).await {
                // A dependency may name a version of another game release.
                Ok(v) if self.compatible(&v) => return Ok(Some(v)),
                Ok(_) | Err(CoreError::HttpStatus { status: 404, .. }) => {}
                Err(e) => return Err(e),
            }
        }
        match &wanted.project {
            Some(project) => self.versions_of(project).await,
            None => Ok(None),
        }
    }

    fn compatible(&self, v: &Version) -> bool {
        v.game_versions.iter().any(|g| g == self.mc)
            && v.loaders.iter().any(|l| self.loaders.contains(&l.as_str()))
    }

    /// Picks a version for each wanted project; returns the next round.
    async fn resolve_round(
        &mut self,
        round: Vec<Wanted>,
        items: &mut Vec<DownloadItem>,
    ) -> Result<Vec<Wanted>> {
        let this = &*self;
        let lookups: Vec<_> = round
            .into_iter()
            .map(|w| async move {
                let version = this.lookup(&w).await?;
                Ok::<_, CoreError>((w, version))
            })
            .collect();
        let found: Vec<(Wanted, Option<Version>)> = stream::iter(lookups)
            .buffered(LOOKUPS)
            .try_collect()
            .await?;
        let ids: Vec<String> = found
            .iter()
            .filter_map(|(_, v)| v.as_ref().map(|v| v.project_id.clone()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let projects: HashMap<String, modrinth::Project> =
            patient(self.cancel, || modrinth::projects(self.ctx, &ids))
                .await?
                .into_iter()
                .map(|p| (p.id.clone(), p))
                .collect();

        let mut next = Vec::new();
        for (wanted, version) in found {
            let Some(version) = version else {
                self.report_missing(&wanted);
                continue;
            };
            if !self.done.insert(version.project_id.clone()) {
                continue;
            }
            let project = projects.get(&version.project_id);
            let in_folder = self.present.projects.contains(&version.project_id)
                || project.is_some_and(|p| present_by_name(self.dir, &p.slug));
            if in_folder {
                if let Some(root) = wanted.root {
                    self.result.already_present.push(root);
                }
                continue;
            }
            if wanted.root.is_none() && self.result.dependencies.len() >= MAX_DEPENDENCIES {
                self.report_missing(&wanted);
                continue;
            }
            match self.plan_file(&version, wanted.disabled) {
                Planned::Queued(_, item) => items.push(item),
                Planned::NameTaken => {
                    if let Some(root) = wanted.root {
                        self.result.already_present.push(root);
                    }
                    continue;
                }
                Planned::Unusable => {
                    self.report_missing(&wanted);
                    continue;
                }
            }
            // The project's title: a jar's own name can carry its old version.
            let title = project.map(|p| p.title.clone());
            match wanted.root {
                Some(root) => self.result.transferred.push(title.unwrap_or(root)),
                None => self
                    .result
                    .dependencies
                    .push(title.unwrap_or_else(|| version.name.clone())),
            }
            next.extend(required(&version, wanted.disabled));
        }
        // Already handled: no request needed.
        next.retain(|w: &Wanted| {
            w.project
                .as_ref()
                .is_none_or(|p| !self.done.contains(p) && !self.present.projects.contains(p))
        });
        Ok(next)
    }

    fn report_missing(&mut self, wanted: &Wanted) {
        match &wanted.root {
            Some(root) => self.result.not_found.push(root.clone()),
            None => self
                .missing
                .extend(wanted.project.as_ref().or(wanted.pinned.as_ref()).cloned()),
        }
    }

    fn plan_file(&mut self, version: &Version, disabled: bool) -> Planned {
        let Some(file) = version.primary_file() else {
            return Planned::Unusable;
        };
        let Ok(name) = checked_name(&file.filename) else {
            return Planned::Unusable;
        };
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
            return Planned::Unusable;
        }
        if !self.present.names.insert(lower) {
            return Planned::NameTaken;
        }
        let dest = if disabled {
            self.dir.join(format!("{name}{DISABLED_SUFFIX}"))
        } else {
            self.dir.join(name)
        };
        let item = install::download_item(&dest, file);
        Planned::Queued(dest, item)
    }

    /// Project ids of missing dependencies become their titles if possible.
    async fn name_missing(&mut self) {
        if self.missing.is_empty() {
            return;
        }
        let titles: HashMap<String, String> = modrinth::projects(self.ctx, &self.missing)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id, p.title))
            .collect();
        self.result.missing_dependencies = self
            .missing
            .iter()
            .map(|id| titles.get(id).unwrap_or(id).clone())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;

    use serde_json::{Value, json};
    use sha1::Digest;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::events::{NullSink, Stage};
    use crate::instance::LoaderSpec;
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    struct Fixture {
        _tmp: tempfile::TempDir,
        ctx: Ctx,
        from: PathBuf,
        to: PathBuf,
    }

    fn fixture(modrinth: Option<&MockServer>) -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        // Same-version transfers must not use the network at all.
        ctx.endpoints.modrinth = modrinth.map_or("http://127.0.0.1:9".to_owned(), MockServer::uri);
        let (from, to) = (tmp.path().join("from"), tmp.path().join("to"));
        std::fs::create_dir_all(from.join("mods")).unwrap();
        std::fs::create_dir_all(to.join("mods")).unwrap();
        Fixture {
            _tmp: tmp,
            ctx,
            from,
            to,
        }
    }

    fn inst(mc: &str, kind: LoaderKind) -> Instance {
        serde_json::from_value(json!({
            "id": format!("{mc}-{kind:?}"), "name": "t", "mcVersion": mc,
            "loader": LoaderSpec { kind, version: None }
        }))
        .unwrap()
    }

    async fn run(f: &Fixture, source: &Instance, target: &Instance) -> Result<TransferResult> {
        let progress = Progress::new(Arc::new(NullSink), "t", Stage::Content);
        transfer(
            &f.ctx,
            source,
            &f.from,
            target,
            &f.to,
            &progress,
            &CancellationToken::new(),
        )
        .await
    }

    /// A Fabric mod jar with an id and a name.
    fn jar(id: &str, name: &str, salt: &str) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        z.start_file("fabric.mod.json", SimpleFileOptions::default())
            .unwrap();
        let text = json!({"id": id, "name": name, "version": salt}).to_string();
        z.write_all(text.as_bytes()).unwrap();
        z.finish().unwrap().into_inner()
    }

    fn sha1(b: &[u8]) -> String {
        hex::encode(sha1::Sha1::digest(b))
    }

    fn version(
        server: &MockServer,
        (id, project): (&str, &str),
        file: &str,
        body: &[u8],
        deps: Value,
    ) -> Value {
        json!({"id": id, "project_id": project, "name": format!("{project} {id}"),
               "version_number": id, "version_type": "release",
               "game_versions": ["1.21.4"], "loaders": ["fabric"], "dependencies": deps,
               "files": [{"url": format!("{}/dl/{file}", server.uri()), "filename": file,
                          "primary": true, "size": body.len(),
                          "hashes": {"sha1": sha1(body)}}]})
    }

    async fn serve_file(server: &MockServer, file: &str, body: &[u8]) {
        Mock::given(method("GET"))
            .and(path(format!("/dl/{file}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
            .mount(server)
            .await;
    }

    async fn serve_versions(server: &MockServer, project: &str, versions: Value) {
        Mock::given(method("GET"))
            .and(path(format!("/v2/project/{project}/version")))
            .respond_with(ResponseTemplate::new(200).set_body_json(versions))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn same_version_copies_the_files_without_the_network() {
        let f = fixture(None);
        let mods = f.from.join("mods");
        let target = f.to.join("mods");
        std::fs::write(mods.join("a.jar"), jar("a", "A", "1")).unwrap();
        std::fs::write(mods.join("off.jar.disabled"), jar("off", "Off", "1")).unwrap();
        std::fs::write(mods.join("plain.jar"), b"no metadata").unwrap();
        // The target's copy of `b` is another version: it stays.
        std::fs::write(mods.join("b-2.jar"), jar("b", "B", "2")).unwrap();
        std::fs::write(target.join("b-1.jar"), jar("b", "B", "1")).unwrap();
        // Same file name, other content: never overwritten.
        std::fs::write(mods.join("clash.jar"), b"theirs").unwrap();
        std::fs::write(target.join("clash.jar"), b"mine").unwrap();
        std::fs::write(mods.join("notes.txt"), b"not a mod").unwrap();

        let same = inst("1.21.4", LoaderKind::Fabric);
        let r = run(&f, &same, &inst("1.21.4", LoaderKind::Fabric))
            .await
            .unwrap();

        assert_eq!(r.transferred, ["A", "Off", "plain.jar"]);
        assert_eq!(r.already_present, ["B", "clash.jar"]);
        assert!(r.not_found.is_empty());
        assert!(target.join("a.jar").exists());
        assert!(target.join("off.jar.disabled").exists());
        assert!(!target.join("off.jar").exists());
        assert!(!target.join("b-2.jar").exists() && target.join("b-1.jar").exists());
        assert_eq!(std::fs::read(target.join("clash.jar")).unwrap(), b"mine");
        assert!(!target.join("notes.txt").exists());
        // The source is left as it was.
        assert!(mods.join("a.jar").exists());
    }

    #[tokio::test]
    async fn quilt_takes_fabric_jars_but_not_the_other_way_round() {
        let (a, b) = (
            inst("1.21.4", LoaderKind::Fabric),
            inst("1.21.4", LoaderKind::Quilt),
        );
        assert!(runs_unchanged(&a, &b));
        assert!(!runs_unchanged(&b, &a));
        assert!(!runs_unchanged(&a, &inst("1.20.1", LoaderKind::Fabric)));
        assert!(!runs_unchanged(&a, &inst("1.21.4", LoaderKind::Forge)));
    }

    #[tokio::test]
    async fn a_target_without_a_loader_is_refused() {
        let f = fixture(None);
        std::fs::write(f.from.join("mods/a.jar"), b"x").unwrap();
        let e = run(
            &f,
            &inst("1.21.4", LoaderKind::Fabric),
            &inst("1.21.4", LoaderKind::Vanilla),
        )
        .await
        .unwrap_err();
        assert_eq!(e.code(), "loader.addonUnavailable");
        assert!(!f.to.join("mods/a.jar").exists());
    }

    #[tokio::test]
    async fn another_version_downloads_the_right_files_with_dependencies() {
        let server = MockServer::builder().start().await;
        let f = fixture(Some(&server));
        let mods = f.from.join("mods");
        let (sodium_old, iris_old, gone_old, have_old) = (
            jar("sodium", "Sodium", "old"),
            jar("iris", "Iris", "old"),
            jar("gone", "Gone Mod", "old"),
            jar("have", "Have", "old"),
        );
        std::fs::write(mods.join("sodium-old.jar"), &sodium_old).unwrap();
        std::fs::write(mods.join("iris-old.jar.disabled"), &iris_old).unwrap();
        std::fs::write(mods.join("gone-old.jar"), &gone_old).unwrap();
        std::fs::write(mods.join("have-old.jar"), &have_old).unwrap();
        std::fs::write(f.to.join("mods/have-new.jar"), jar("have", "Have", "new")).unwrap();

        let old = |id: &str, project: &str, body: &[u8]| {
            version(&server, (id, project), "x.jar", body, json!([]))
        };
        let mut by_hash = serde_json::Map::new();
        for (body, id, project) in [
            (&sodium_old, "s1", "sodium-id"),
            (&iris_old, "i1", "iris-id"),
            (&gone_old, "g1", "gone-id"),
            (&have_old, "h1", "have-id"),
        ] {
            by_hash.insert(sha1(body), old(id, project, body));
        }
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(move |req: &wiremock::Request| {
                // Only what was asked for: the target's file is not on Modrinth.
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                let asked: serde_json::Map<String, Value> = body["hashes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|h| h.as_str())
                    .filter_map(|h| by_hash.get(h).map(|v| (h.to_owned(), v.clone())))
                    .collect();
                ResponseTemplate::new(200).set_body_json(Value::Object(asked))
            })
            .mount(&server)
            .await;

        let (sodium_new, iris_new, lib, off_lib) = (
            jar("sodium", "Sodium", "new"),
            jar("iris", "Iris", "new"),
            jar("lib", "Lib", "new"),
            jar("offlib", "Off Lib", "new"),
        );
        let sodium_deps = json!([
            {"project_id": "lib-id", "dependency_type": "required"},
            {"project_id": "nowhere-id", "dependency_type": "required"},
            {"project_id": "opt-id", "dependency_type": "optional"},
            {"project_id": "iris-id", "dependency_type": "required"}
        ]);
        let iris_deps = json!([{"project_id": "offlib-id", "dependency_type": "required"}]);
        serve_versions(
            &server,
            "sodium-id",
            json!([
                version(
                    &server,
                    ("s9", "sodium-id"),
                    "sodium-beta.jar",
                    b"beta",
                    json!([])
                )
                .as_object()
                .map(|o| {
                    let mut o = o.clone();
                    o.insert("version_type".into(), json!("beta"));
                    Value::Object(o)
                })
                .unwrap(),
                version(
                    &server,
                    ("s2", "sodium-id"),
                    "sodium-new.jar",
                    &sodium_new,
                    sodium_deps
                )
            ]),
        )
        .await;
        serve_versions(
            &server,
            "iris-id",
            json!([version(
                &server,
                ("i2", "iris-id"),
                "iris-new.jar",
                &iris_new,
                iris_deps
            )]),
        )
        .await;
        serve_versions(
            &server,
            "lib-id",
            json!([version(
                &server,
                ("l1", "lib-id"),
                "lib.jar",
                &lib,
                json!([])
            )]),
        )
        .await;
        serve_versions(
            &server,
            "offlib-id",
            json!([version(
                &server,
                ("o1", "offlib-id"),
                "offlib.jar",
                &off_lib,
                json!([])
            )]),
        )
        .await;
        serve_versions(&server, "gone-id", json!([])).await;
        serve_versions(&server, "nowhere-id", json!([])).await;
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "sodium-id", "slug": "sodium", "title": "Sodium"},
                {"id": "iris-id", "slug": "iris", "title": "Iris"},
                {"id": "lib-id", "slug": "lib", "title": "Library"},
                {"id": "offlib-id", "slug": "offlib", "title": "Off Library"},
                {"id": "nowhere-id", "slug": "nowhere", "title": "Nowhere Lib"}
            ])))
            .mount(&server)
            .await;
        for (file, body) in [
            ("sodium-new.jar", &sodium_new),
            ("iris-new.jar", &iris_new),
            ("lib.jar", &lib),
            ("offlib.jar", &off_lib),
        ] {
            serve_file(&server, file, body).await;
        }

        let r = run(
            &f,
            &inst("1.20.1", LoaderKind::Fabric),
            &inst("1.21.4", LoaderKind::Fabric),
        )
        .await
        .unwrap();

        assert_eq!(r.transferred, ["Iris", "Sodium"]);
        assert_eq!(r.dependencies, ["Library", "Off Library"]);
        assert_eq!(r.already_present, ["Have"]);
        assert_eq!(r.not_found, ["Gone Mod"]);
        assert_eq!(r.missing_dependencies, ["Nowhere Lib"]);

        let target = f.to.join("mods");
        // Releases are preferred over a newer beta; the bytes were verified.
        assert_eq!(
            std::fs::read(target.join("sodium-new.jar")).unwrap(),
            sodium_new
        );
        // A disabled mod stays disabled, and what only it needs too.
        assert!(target.join("iris-new.jar.disabled").exists());
        assert!(target.join("offlib.jar.disabled").exists());
        assert!(target.join("lib.jar").exists());
        // Old-version jars are never copied; the target's own mod stays.
        for old in ["sodium-old.jar", "gone-old.jar", "have-old.jar"] {
            assert!(!target.join(old).exists(), "{old}");
        }
        assert!(target.join("have-new.jar").exists());
    }

    #[tokio::test]
    async fn unknown_jars_are_searched_by_name_and_kept_only_if_they_match() {
        let server = MockServer::builder().start().await;
        let f = fixture(Some(&server));
        let mods = f.from.join("mods");
        std::fs::write(mods.join("real.jar"), jar("real", "Real Mod", "cf")).unwrap();
        std::fs::write(mods.join("fake.jar"), jar("fake", "Fake Mod", "cf")).unwrap();
        std::fs::write(mods.join("none.jar"), jar("none", "No Match", "cf")).unwrap();
        std::fs::write(mods.join("bare.jar"), b"no metadata").unwrap();

        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let hit = |id: &str, slug: &str, title: &str| json!({"project_id": id, "project_type": "mod", "slug": slug, "title": title});
        let page = |hits: Value| json!({"hits": hits, "offset": 0, "total_hits": 1});
        for (query, hits) in [
            ("Real Mod", json!([hit("real-id", "real-mod", "Real Mod")])),
            // Same title, but the jar is another mod.
            ("Fake Mod", json!([hit("other-id", "other", "Fake Mod")])),
            // Similar is not the same.
            (
                "No Match",
                json!([hit("near-id", "no-match-2", "No Match 2")]),
            ),
        ] {
            Mock::given(method("GET"))
                .and(path("/v2/search"))
                .and(query_param("query", query))
                .respond_with(ResponseTemplate::new(200).set_body_json(page(hits)))
                .mount(&server)
                .await;
        }
        let (real, other, dep) = (
            jar("real", "Real Mod", "new"),
            jar("different", "Fake Mod", "new"),
            jar("dep", "Dep", "new"),
        );
        let deps = json!([{"project_id": "dep-id", "dependency_type": "required"}]);
        serve_versions(
            &server,
            "real-id",
            json!([version(
                &server,
                ("r1", "real-id"),
                "real-new.jar",
                &real,
                json!([])
            )]),
        )
        .await;
        serve_versions(
            &server,
            "other-id",
            json!([version(
                &server,
                ("o1", "other-id"),
                "other.jar",
                &other,
                deps
            )]),
        )
        .await;
        serve_file(&server, "real-new.jar", &real).await;
        serve_file(&server, "other.jar", &other).await;
        serve_file(&server, "dep.jar", &dep).await;

        let r = run(
            &f,
            &inst("1.20.1", LoaderKind::Fabric),
            &inst("1.21.4", LoaderKind::Fabric),
        )
        .await
        .unwrap();

        assert_eq!(r.transferred, ["Real Mod"]);
        assert_eq!(r.not_found, ["bare.jar", "Fake Mod", "No Match"]);
        let target = f.to.join("mods");
        assert_eq!(std::fs::read(target.join("real-new.jar")).unwrap(), real);
        // The wrong mod is gone again, and its dependency never came.
        assert!(!target.join("other.jar").exists());
        assert!(!target.join("dep.jar").exists());
    }

    #[tokio::test]
    async fn waits_out_the_rate_limit() {
        let server = MockServer::builder().start().await;
        let f = fixture(Some(&server));
        let body = jar("a", "A", "old");
        std::fs::write(f.from.join("mods/a.jar"), &body).unwrap();
        let new = jar("a", "A", "new");

        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ sha1(&body): version(&server, ("a1", "a-id"), "a.jar", &body, json!([])) }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/project/a-id/version"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        serve_versions(
            &server,
            "a-id",
            json!([version(
                &server,
                ("a2", "a-id"),
                "a-new.jar",
                &new,
                json!([])
            )]),
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([{"id": "a-id", "slug": "a-mod", "title": "A"}])),
            )
            .mount(&server)
            .await;
        serve_file(&server, "a-new.jar", &new).await;

        let r = run(
            &f,
            &inst("1.20.1", LoaderKind::Fabric),
            &inst("1.21.4", LoaderKind::Fabric),
        )
        .await
        .unwrap();
        assert_eq!(r.transferred, ["A"]);
    }

    #[tokio::test]
    async fn a_server_failure_is_an_error_not_a_missing_mod() {
        let server = MockServer::builder().start().await;
        let f = fixture(Some(&server));
        let body = jar("a", "A", "old");
        std::fs::write(f.from.join("mods/a.jar"), &body).unwrap();
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({ sha1(&body): version(&server, ("a1", "a-id"), "a.jar", &body, json!([])) }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/project/a-id/version"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let e = run(
            &f,
            &inst("1.20.1", LoaderKind::Fabric),
            &inst("1.21.4", LoaderKind::Fabric),
        )
        .await
        .unwrap_err();
        assert!(matches!(e, CoreError::HttpStatus { status: 500, .. }));
    }
}
