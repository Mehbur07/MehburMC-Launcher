//! MehburMC Library (ARCHITECTURE K72): community mods, published only
//! after an admin approved them. The uploader's launcher checks the jar
//! (`content::scan`), uploads it to the private `library` bucket as
//! `<uid>/<sha1>.jar` and records it with `submit_library_mod`
//! (`supabase/phase19.sql`). Installing downloads the file, checks it
//! against the listed SHA-1 and scans it again — the uploader's report is
//! never trusted on its own.

use std::path::Path;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use super::FriendsClient;
use super::avatar::sha1_hex;
pub use crate::content::scan::LibraryKind;
use crate::content::scan::{self, Finding, ModLoader, ScanReport, Severity, Verdict};
use crate::error::{CoreError, Result};
use crate::instance::LoaderKind;
use crate::instance::files::{Folder, checked_name};

const BUCKET: &str = "library";
const MAX_NAME_CHARS: usize = 64;
const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_NOTE_CHARS: usize = 500;
const MAX_FILE_NAME_CHARS: usize = 120;

/// Codes the server (or this module) raises; each has an
/// `errors.library.*` translation.
pub const LIBRARY_ERRORS: &[&str] = &[
    "library.blocked",
    "library.fileMissing",
    "library.quota",
    "library.notFound",
    "library.changed",
    "library.conflict",
    "library.wrongLoader",
    "library.badKind",
    "library.serverNotReady",
    "library.storageFull",
    "library.wrongKind",
];

/// Finding codes the scanner produces; anything else in a report that came
/// from the server is dropped.
const FINDING_CODES: &[&str] = &[
    "webhook",
    "credentialPaths",
    "hiddenPayload",
    "executable",
    "zipBomb",
    "tooManyFiles",
    "tooLarge",
    "noDescriptor",
    "notAJar",
    "network",
    "processExec",
    "nativeLoad",
    "nativeBinary",
    "defineClass",
    "crypto",
    "suspiciousUrl",
    "encodedBlob",
    "obfuscated",
    "badClass",
    "deepNesting",
    "codeInPack",
    "badDownload",
    "unsafePath",
    "externalDownload",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryStatus {
    /// Waiting for an admin.
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryReportReason {
    Malware,
    Broken,
    Stolen,
    Other,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryMod {
    #[ts(type = "number")]
    pub id: i64,
    /// Mod, modpack, resource pack or shader (K88).
    pub kind: LibraryKind,
    /// Account name of the uploader.
    pub author: String,
    pub name: String,
    pub description: String,
    pub mod_id: String,
    pub version: String,
    pub loaders: Vec<ModLoader>,
    pub game_versions: String,
    pub sha1: String,
    #[ts(type = "number")]
    pub size: u64,
    pub file_name: String,
    pub status: LibraryStatus,
    /// The uploader's automatic check (codes filtered to known ones).
    pub findings: Vec<Finding>,
    /// Admin's note on a rejection; only for own uploads.
    pub review_note: Option<String>,
    /// RFC 3339.
    pub created_at: String,
    /// Uploaded by this installation.
    pub mine: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Row {
    pub(crate) id: i64,
    pub(crate) owner: String,
    author: String,
    name: String,
    #[serde(default)]
    description: String,
    mod_id: String,
    version: String,
    loaders: Vec<String>,
    #[serde(default)]
    game_versions: String,
    sha1: String,
    size: u64,
    file_name: String,
    status: LibraryStatus,
    #[serde(default)]
    scan: Value,
    review_note: Option<String>,
    created_at: String,
    #[serde(default)]
    mine: bool,
    /// Missing before K88 (mods only); an unknown kind drops the row.
    #[serde(default)]
    kind: Option<String>,
}

/// PostgREST's PGRST202: no function with these parameter names, i.e. the
/// server predates the SQL file that added them.
fn missing_function(e: &CoreError) -> bool {
    matches!(e, CoreError::FriendsServer { status: 404, reason }
        if reason.contains("Could not find the function"))
}

/// Storage refused the file type or size: a bucket still set up for mods
/// only (library_packs.sql not run).
fn bucket_refused(e: &CoreError) -> bool {
    matches!(e, CoreError::FriendsServer { status, reason }
        if *status == 413 || reason.contains("mime type") || reason.contains("exceeded the maximum"))
}

fn parse_kind(s: Option<&str>) -> Option<LibraryKind> {
    match s {
        None => Some(LibraryKind::Mod),
        Some(s) => LibraryKind::ALL.into_iter().find(|k| k.as_str() == s),
    }
}

/// Content type of an upload in the `library` bucket.
fn mime(kind: LibraryKind) -> &'static str {
    match kind {
        LibraryKind::Mod => "application/java-archive",
        LibraryKind::Modpack => "application/x-modrinth-modpack+zip",
        LibraryKind::ResourcePack | LibraryKind::Shader => "application/zip",
    }
}

/// Instance folder an item of this kind goes into (`None`: a modpack
/// becomes an instance of its own).
pub fn install_folder(kind: LibraryKind) -> Option<Folder> {
    match kind {
        LibraryKind::Mod => Some(Folder::Mods),
        LibraryKind::ResourcePack => Some(Folder::ResourcePacks),
        LibraryKind::Shader => Some(Folder::ShaderPacks),
        LibraryKind::Modpack => None,
    }
}

/// An id the server accepts (`[a-z0-9_.-]{1,64}`) for packs, from their name.
fn slug(name: &str, kind: LibraryKind) -> String {
    let mut s = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s: String = s.trim_matches(['-', '.']).chars().take(64).collect();
    if s.is_empty() {
        kind.as_str().to_owned()
    } else {
        s
    }
}

/// The library file after its SHA-1 check, or the warnings this
/// launcher's own scan found (when they were not accepted yet).
enum Checked {
    Ready(Vec<u8>),
    Ask(Vec<Finding>),
}

/// Result of an install attempt.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum LibraryInstall {
    Installed {
        file_name: String,
    },
    /// The same file (by SHA-1) is already in `mods/`.
    AlreadyPresent {
        file_name: String,
    },
    /// This launcher's own scan found warnings; nothing was installed.
    /// Call again with `accept_warnings` after the user agreed.
    NeedsConfirm {
        findings: Vec<Finding>,
    },
    /// A modpack became a new instance.
    Imported {
        instance_id: String,
        name: String,
    },
}

fn loader_str(l: ModLoader) -> &'static str {
    match l {
        ModLoader::Fabric => "fabric",
        ModLoader::Quilt => "quilt",
        ModLoader::Forge => "forge",
        ModLoader::NeoForge => "neoforge",
    }
}

fn parse_loader(s: &str) -> Option<ModLoader> {
    Some(match s {
        "fabric" => ModLoader::Fabric,
        "quilt" => ModLoader::Quilt,
        "forge" => ModLoader::Forge,
        "neoforge" => ModLoader::NeoForge,
        _ => return None,
    })
}

/// Whether a mod for `loaders` runs on an instance with `kind`.
pub fn fits(loaders: &[ModLoader], kind: LoaderKind) -> bool {
    let want = match kind {
        LoaderKind::Fabric | LoaderKind::LegacyFabric => ModLoader::Fabric,
        LoaderKind::Quilt => ModLoader::Quilt,
        LoaderKind::Forge => ModLoader::Forge,
        LoaderKind::NeoForge => ModLoader::NeoForge,
        LoaderKind::Vanilla | LoaderKind::Optifine => return false,
    };
    loaders.contains(&want)
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Trimmed, without control characters (newlines kept if `multiline`).
fn clean_text(s: &str, max: usize, multiline: bool) -> String {
    s.trim()
        .chars()
        .filter(|c| (multiline && *c == '\n') || !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A file name the server accepts (`[A-Za-z0-9._+-]`, ends with the
/// kind's extension).
pub fn library_file_name(name: &str, kind: LibraryKind) -> String {
    let ext = kind.ext();
    let stem = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim_end_matches(".disabled");
    let stem = match stem.rsplit_once('.') {
        Some((s, e)) if e.eq_ignore_ascii_case(ext) => s,
        _ => stem,
    };
    let mut s: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(MAX_FILE_NAME_CHARS - 1 - ext.len())
        .collect();
    s = s.trim_matches('.').to_owned();
    if s.is_empty() {
        s = kind.as_str().into();
    }
    format!("{s}.{ext}")
}

/// Findings from a server row, limited to known codes and short examples.
fn findings_from(scan: &Value) -> Vec<Finding> {
    let Some(list) = scan["findings"].as_array() else {
        return Vec::new();
    };
    list.iter()
        .take(FINDING_CODES.len())
        .filter_map(|f| {
            let code = f["code"].as_str()?;
            if !FINDING_CODES.contains(&code) {
                return None;
            }
            let severity = match f["severity"].as_str()? {
                "block" => Severity::Block,
                "warn" => Severity::Warn,
                _ => return None,
            };
            let examples = f["examples"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .take(5)
                        .map(|s| clean_text(s, 160, false))
                        .collect()
                })
                .unwrap_or_default();
            Some(Finding {
                severity,
                code: code.to_owned(),
                examples,
            })
        })
        .collect()
}

impl Row {
    pub(crate) fn into_mod(self) -> Option<LibraryMod> {
        if !is_uuid(&self.owner) || !is_sha1(&self.sha1) {
            return None;
        }
        let kind = parse_kind(self.kind.as_deref())?;
        let loaders: Vec<ModLoader> = self
            .loaders
            .iter()
            .filter_map(|l| parse_loader(l))
            .collect();
        Some(LibraryMod {
            id: self.id,
            kind,
            author: clean_text(&self.author, 16, false),
            name: clean_text(&self.name, MAX_NAME_CHARS, false),
            description: clean_text(&self.description, MAX_DESCRIPTION_CHARS, true),
            mod_id: clean_text(&self.mod_id, 64, false),
            version: clean_text(&self.version, 64, false),
            loaders,
            game_versions: clean_text(&self.game_versions, 100, false),
            sha1: self.sha1.to_ascii_lowercase(),
            size: self.size,
            file_name: library_file_name(&self.file_name, kind),
            status: self.status,
            findings: findings_from(&self.scan),
            review_note: self
                .review_note
                .map(|n| clean_text(&n, MAX_NOTE_CHARS, true)),
            created_at: self.created_at,
            mine: self.mine,
        })
    }
}

impl FriendsClient {
    /// Checks `bytes` as `kind` and submits it for review; returns the
    /// library id. Blocked files never leave the computer.
    pub async fn submit_library_mod(
        &self,
        bytes: &[u8],
        kind: LibraryKind,
        file_name: &str,
        name: &str,
        description: &str,
        author: &str,
    ) -> Result<(i64, ScanReport)> {
        let report = {
            let bytes = bytes.to_vec();
            tokio::task::spawn_blocking(move || scan::scan_as(&bytes, kind))
                .await
                .map_err(|e| CoreError::FriendsServer {
                    status: 0,
                    reason: e.to_string(),
                })?
        };
        let Some(d) = report
            .descriptor
            .clone()
            .filter(|_| report.verdict != Verdict::Block)
        else {
            return Err(CoreError::Friends("library.blocked"));
        };
        let file_name = library_file_name(file_name, kind);
        // Packs: the given name, else the pack's own, else the file name.
        let stem = file_name.rsplit_once('.').map_or("", |(s, _)| s);
        let name = [clean_text(name, MAX_NAME_CHARS, false), d.name.clone()]
            .into_iter()
            .map(|n| n.chars().take(MAX_NAME_CHARS).collect::<String>())
            .find(|n| !n.trim().is_empty())
            .unwrap_or_else(|| stem.chars().take(MAX_NAME_CHARS).collect());
        let mod_id = match kind {
            LibraryKind::Mod => d.mod_id.clone(),
            _ => slug(&name, kind),
        };
        let version = match d.version.chars().take(64).collect::<String>() {
            v if !v.trim().is_empty() => v,
            _ if kind == LibraryKind::Mod => "unknown".to_owned(),
            _ => "1".to_owned(),
        };
        self.ensure_identity().await?;
        let me = self.user_id().await?;
        let upload = self
            .send_raw(
                Method::POST,
                &format!(
                    "/storage/v1/object/{BUCKET}/{me}/{}.{}",
                    report.sha1,
                    kind.ext()
                ),
                bytes.to_vec(),
                mime(kind),
                &[("x-upsert", "true")],
            )
            .await;
        match upload {
            // The bucket still has the mods-only settings (type, 25 MiB).
            Err(e) if kind != LibraryKind::Mod && bucket_refused(&e) => {
                return Err(CoreError::Friends("library.serverNotReady"));
            }
            r => r?,
        };
        let loaders: Vec<&str> = d.loaders.iter().map(|l| loader_str(*l)).collect();
        let mut args = json!({
            "p_name": name,
            "p_description": clean_text(description, MAX_DESCRIPTION_CHARS, true),
            "p_author": author.trim(),
            "p_mod_id": mod_id,
            "p_version": version,
            "p_loaders": loaders,
            "p_game_versions": d.game_versions,
            "p_sha1": report.sha1,
            "p_size": report.size,
            "p_file_name": file_name,
            "p_scan": report,
            "p_kind": kind.as_str(),
        });
        let id = match self.rpc("submit_library_mod", args.clone()).await {
            // Server without library_packs.sql: no `p_kind` yet; mods still work.
            Err(e) if missing_function(&e) && kind == LibraryKind::Mod => {
                args.as_object_mut().map(|o| o.remove("p_kind"));
                self.rpc("submit_library_mod", args).await?
            }
            Err(e) if missing_function(&e) => {
                return Err(CoreError::Friends("library.serverNotReady"));
            }
            r => r?,
        };
        Ok((id, report))
    }

    /// Withdraws one of the caller's uploads and deletes its file.
    pub async fn withdraw_library_mod(&self, id: i64) -> Result<()> {
        let me = self.user_id().await?;
        let sha1: String = self
            .rpc("withdraw_library_mod", json!({ "p_id": id }))
            .await?;
        if !is_sha1(&sha1) {
            return Ok(());
        }
        // The kind is gone with the row: remove whichever file it was.
        let sha1 = sha1.to_ascii_lowercase();
        let prefixes: Vec<String> = ["jar", "mrpack", "zip"]
            .iter()
            .map(|ext| format!("{me}/{sha1}.{ext}"))
            .collect();
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": prefixes })),
            &[],
        )
        .await
        .map(drop)
    }

    /// Approved items of every kind and the caller's own uploads (own first).
    pub async fn library_mods(&self) -> Result<Vec<LibraryMod>> {
        Ok(self
            .library_rows()
            .await?
            .into_iter()
            .filter_map(Row::into_mod)
            .collect())
    }

    async fn library_rows(&self) -> Result<Vec<Row>> {
        if !self.has_identity().await {
            self.ensure_identity().await?;
        }
        match self
            .rpc("library_list", json!({ "p_max": 300, "p_all": true }))
            .await
        {
            // Server without library_packs.sql: the mods-only list.
            Err(e) if missing_function(&e) => {
                self.rpc("library_list", json!({ "p_max": 300 })).await
            }
            r => r,
        }
    }

    pub async fn report_library_mod(
        &self,
        id: i64,
        reason: LibraryReportReason,
        note: &str,
    ) -> Result<()> {
        self.rpc_void(
            "report_library_mod",
            json!({
                "p_id": id,
                "p_reason": reason,
                "p_note": clean_text(note, MAX_NOTE_CHARS, true),
            }),
        )
        .await
    }

    /// An approved item (or one of the caller's own) with its uploader.
    async fn installable(&self, id: i64) -> Result<(String, LibraryMod)> {
        let row = self
            .library_rows()
            .await?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or(CoreError::Friends("library.notFound"))?;
        let owner = row.owner.clone();
        let m = row
            .into_mod()
            .ok_or(CoreError::Friends("library.notFound"))?;
        if m.status != LibraryStatus::Approved && !m.mine {
            return Err(CoreError::Friends("library.notFound"));
        }
        Ok((owner, m))
    }

    /// Downloads the item (SHA-1 checked) and scans it here; blocked files
    /// are refused, warnings come back unless `accept_warnings`.
    async fn checked(&self, owner: &str, m: &LibraryMod, accept_warnings: bool) -> Result<Checked> {
        let bytes = self.library_file(owner, &m.sha1, m.kind).await?;
        let kind = m.kind;
        let (report, bytes) =
            tokio::task::spawn_blocking(move || (scan::scan_as(&bytes, kind), bytes))
                .await
                .map_err(|e| CoreError::FriendsServer {
                    status: 0,
                    reason: e.to_string(),
                })?;
        match report.verdict {
            Verdict::Block => Err(CoreError::Friends("library.blocked")),
            Verdict::Warn if !accept_warnings => Ok(Checked::Ask(report.findings)),
            _ => Ok(Checked::Ready(bytes)),
        }
    }

    /// Installs library item `id` (a mod, resource pack or shader) into the
    /// matching folder of the instance at `instance_dir`, after verifying
    /// its SHA-1 and scanning it here. Refuses blocked files; asks back
    /// ([`LibraryInstall::NeedsConfirm`]) when the scan has warnings and
    /// `accept_warnings` is false. Modpacks: [`Self::download_library_modpack`].
    pub async fn install_library_mod(
        &self,
        id: i64,
        instance_dir: &Path,
        loader: LoaderKind,
        accept_warnings: bool,
    ) -> Result<LibraryInstall> {
        let (owner, m) = self.installable(id).await?;
        let folder = install_folder(m.kind).ok_or(CoreError::Friends("library.wrongKind"))?;
        if m.kind == LibraryKind::Mod && !fits(&m.loaders, loader) {
            return Err(CoreError::Friends("library.wrongLoader"));
        }
        let dir = instance_dir.join(folder.dir_name());
        let file_name = checked_name(&m.file_name)?.to_owned();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| CoreError::io(&dir, e))?;
        let present = {
            let (ctx, d) = (self.ctx().clone(), dir.clone());
            tokio::task::spawn_blocking(move || crate::content::installed::hashes(&ctx, &d))
                .await
                .map_err(|e| CoreError::FriendsServer {
                    status: 0,
                    reason: e.to_string(),
                })??
        };
        if let Some((name, _, _)) = present.iter().find(|(_, sha, _)| *sha == m.sha1) {
            return Ok(LibraryInstall::AlreadyPresent {
                file_name: name.clone(),
            });
        }
        if present
            .iter()
            .any(|(n, _, _)| *n == file_name || *n == format!("{file_name}.disabled"))
        {
            return Err(CoreError::Friends("library.conflict"));
        }
        let bytes = match self.checked(&owner, &m, accept_warnings).await? {
            Checked::Ready(b) => b,
            Checked::Ask(findings) => return Ok(LibraryInstall::NeedsConfirm { findings }),
        };
        crate::fsutil::write_atomic(&dir.join(&file_name), &bytes)?;
        Ok(LibraryInstall::Installed { file_name })
    }

    /// A library modpack, checked like any item, saved as a `.mrpack` for
    /// the importer. `Err(findings)` when the scan warned and
    /// `accept_warnings` is false.
    pub async fn download_library_modpack(
        &self,
        id: i64,
        accept_warnings: bool,
    ) -> Result<std::result::Result<(LibraryMod, std::path::PathBuf), Vec<Finding>>> {
        let (owner, m) = self.installable(id).await?;
        if m.kind != LibraryKind::Modpack {
            return Err(CoreError::Friends("library.wrongKind"));
        }
        let bytes = match self.checked(&owner, &m, accept_warnings).await? {
            Checked::Ready(b) => b,
            Checked::Ask(findings) => return Ok(Err(findings)),
        };
        let dir = self.ctx().paths.cache().join("library").join("import");
        let path = dir.join(&m.file_name);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| CoreError::io(&dir, e))?;
        crate::fsutil::write_atomic(&path, &bytes)?;
        Ok(Ok((m, path)))
    }

    /// The file from the cache or the server, verified against `sha1`.
    pub(crate) async fn library_file(
        &self,
        owner: &str,
        sha1: &str,
        kind: LibraryKind,
    ) -> Result<Vec<u8>> {
        let ext = kind.ext();
        let cached = self
            .ctx()
            .paths
            .cache()
            .join("library")
            .join(format!("{sha1}.{ext}"));
        if let Ok(bytes) = std::fs::read(&cached)
            && sha1_hex(&bytes) == sha1
        {
            return Ok(bytes);
        }
        let bytes = self
            .send(
                Method::GET,
                &format!("/storage/v1/object/authenticated/{BUCKET}/{owner}/{sha1}.{ext}"),
                None,
                &[],
            )
            .await?;
        if sha1_hex(&bytes) != sha1 {
            return Err(CoreError::HashMismatch {
                path: cached,
                expected: sha1.to_owned(),
                actual: sha1_hex(&bytes),
            });
        }
        if let Err(e) = crate::fsutil::write_atomic(&cached, &bytes) {
            tracing::debug!(error = %e.detail(), "could not cache a library file");
        }
        Ok(bytes)
    }

    /// Deletes every library file of the caller (before `delete_me`). A
    /// server without the library (phase 19 SQL not run) has nothing.
    pub(crate) async fn delete_all_library_files(&self) -> Result<()> {
        match self.delete_own_objects(BUCKET).await {
            Err(CoreError::FriendsServer {
                status: 400 | 404, ..
            }) => Ok(()),
            r => r,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, ME, signed_in};
    use super::*;
    use crate::content::scan::tests::{class, fabric_json, jar};

    fn good_jar() -> Vec<u8> {
        jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "com/cool/Mod.class",
                class(&[("java/util/List", "add")], &[]),
            ),
        ])
    }

    fn warn_jar() -> Vec<u8> {
        jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "com/cool/Net.class",
                class(&[("java/net/URL", "openConnection")], &[]),
            ),
        ])
    }

    fn row(id: i64, bytes: &[u8], status: &str, mine: bool) -> Value {
        json!({
            "id": id, "owner": FRIEND, "author": "Alex", "name": format!("Mod {id}"),
            "description": "line1\nline2\u{7}", "mod_id": "coolmod", "version": "1.2.0",
            "loaders": ["fabric", "quilt", "bogus"], "game_versions": ">=1.21",
            "sha1": sha1_hex(bytes), "size": bytes.len(), "file_name": "cool mod.jar",
            "status": status, "review_note": null, "created_at": "2026-10-06T10:00:00Z",
            "mine": mine,
            "scan": { "verdict": "warn", "findings": [
                { "severity": "warn", "code": "network", "examples": ["a/B.class"] },
                { "severity": "block", "code": "<script>", "examples": [] }
            ]}
        })
    }

    #[test]
    fn file_names_fit_the_server_rule() {
        assert_eq!(
            library_file_name("Cool Mod (1.2).jar", LibraryKind::Mod),
            "Cool_Mod__1.2_.jar"
        );
        assert_eq!(
            library_file_name("C:\\x\\sodium+mc.jar.disabled", LibraryKind::Mod),
            "sodium+mc.jar"
        );
        assert_eq!(library_file_name("..jar", LibraryKind::Mod), "mod.jar");
        assert_eq!(
            library_file_name("Faithful 32x.zip", LibraryKind::ResourcePack),
            "Faithful_32x.zip"
        );
        assert_eq!(
            library_file_name("pack.mrpack", LibraryKind::Modpack),
            "pack.mrpack"
        );
        // A wrong extension stays part of the name.
        assert_eq!(
            library_file_name("bsl.jar", LibraryKind::Shader),
            "bsl.jar.zip"
        );
        assert_eq!(library_file_name(".zip", LibraryKind::Shader), "shader.zip");
        assert!(
            library_file_name(&"a".repeat(300), LibraryKind::Modpack).len() <= MAX_FILE_NAME_CHARS
        );
    }

    #[test]
    fn loader_fit() {
        let fq = [ModLoader::Fabric, ModLoader::Quilt];
        assert!(fits(&fq, LoaderKind::Fabric) && fits(&fq, LoaderKind::Quilt));
        assert!(!fits(&fq, LoaderKind::Forge) && !fits(&fq, LoaderKind::Vanilla));
        assert!(fits(&[ModLoader::NeoForge], LoaderKind::NeoForge));
    }

    #[tokio::test]
    async fn submits_by_uploading_then_recording() {
        let server = MockServer::builder().start().await;
        let bytes = good_jar();
        let sha = sha1_hex(&bytes);
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/library/{ME}/{sha}.jar")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/submit_library_mod"))
            .and(body_partial_json(json!({
                "p_name": "Cool Mod", "p_author": "Steve", "p_mod_id": "coolmod",
                "p_version": "1.2.0", "p_loaders": ["fabric", "quilt"],
                "p_game_versions": ">=1.21", "p_sha1": sha, "p_file_name": "cool_mod.jar",
                "p_scan": { "verdict": "pass", "kind": "mod" }, "p_kind": "mod"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(4)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let (id, report) = c
            .submit_library_mod(
                &bytes,
                LibraryKind::Mod,
                "cool mod.jar",
                "  ",
                "desc",
                "Steve",
            )
            .await
            .unwrap();
        assert_eq!((id, report.verdict), (4, Verdict::Pass));

        // A blocked jar is refused before anything is sent (mocks expect 1).
        let evil = jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "a/B.class",
                class(&[], &["https://discord.com/api/webhooks/1/x"]),
            ),
        ]);
        let e = c
            .submit_library_mod(&evil, LibraryKind::Mod, "e.jar", "Evil", "", "Steve")
            .await
            .unwrap_err();
        assert_eq!(e.code(), "library.blocked");
    }

    #[tokio::test]
    async fn list_sanitizes_rows() {
        let server = MockServer::builder().start().await;
        let bytes = good_jar();
        let mut bad = row(2, &bytes, "approved", false);
        bad["sha1"] = json!("../../x");
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/library_list"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([row(1, &bytes, "pending", true), bad])),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let list = c.library_mods().await.unwrap();
        assert_eq!(list.len(), 1);
        let m = &list[0];
        assert_eq!(m.status, LibraryStatus::Pending);
        assert_eq!(m.loaders, [ModLoader::Fabric, ModLoader::Quilt]);
        assert_eq!(m.description, "line1\nline2");
        assert_eq!(m.file_name, "cool_mod.jar");
        assert_eq!(m.findings.len(), 1);
        assert_eq!(m.findings[0].code, "network");
    }

    #[tokio::test]
    async fn falls_back_to_the_mods_only_list_on_an_old_server() {
        let server = MockServer::builder().start().await;
        let bytes = good_jar();
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/library_list"))
            .and(body_partial_json(json!({ "p_all": true })))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "code": "PGRST202",
                "message": "Could not find the function public.library_list(p_all, p_max) in the schema cache"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/library_list"))
            .and(body_json(json!({ "p_max": 300 })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([row(1, &bytes, "approved", false)])),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let list = c.library_mods().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, LibraryKind::Mod);
    }

    #[tokio::test]
    async fn install_verifies_rescans_and_asks_on_warnings() {
        let server = MockServer::builder().start().await;
        let good = good_jar();
        let warn = warn_jar();
        let mut lying = row(3, &good, "approved", false);
        lying["sha1"] = json!("1".repeat(40));
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/library_list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                row(1, &good, "approved", false),
                row(2, &warn, "approved", false),
                lying,
                row(4, &good, "pending", false),
            ])))
            .mount(&server)
            .await;
        for (bytes, sha) in [
            (&good, sha1_hex(&good)),
            (&warn, sha1_hex(&warn)),
            (&good, "1".repeat(40)),
        ] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/storage/v1/object/authenticated/library/{FRIEND}/{sha}.jar"
                )))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
                .mount(&server)
                .await;
        }
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let inst = tmp.path().join("inst");
        let mods = inst.join("mods");

        let r = c
            .install_library_mod(1, &inst, LoaderKind::Fabric, false)
            .await
            .unwrap();
        assert!(
            matches!(r, LibraryInstall::Installed { ref file_name } if file_name == "cool_mod.jar")
        );
        assert_eq!(std::fs::read(mods.join("cool_mod.jar")).unwrap(), good);
        // Same bytes again → already there.
        assert!(matches!(
            c.install_library_mod(1, &inst, LoaderKind::Fabric, false)
                .await
                .unwrap(),
            LibraryInstall::AlreadyPresent { .. }
        ));
        // Same name as an installed file → conflict (checked before downloading).
        assert_eq!(
            c.install_library_mod(2, &inst, LoaderKind::Fabric, true)
                .await
                .unwrap_err()
                .code(),
            "library.conflict"
        );
        std::fs::remove_file(mods.join("cool_mod.jar")).unwrap();
        // Warnings found here → ask first, install only after consent.
        assert!(matches!(
            c.install_library_mod(2, &inst, LoaderKind::Fabric, false)
                .await
                .unwrap(),
            LibraryInstall::NeedsConfirm { .. }
        ));
        assert!(!mods.join("cool_mod.jar").exists());
        assert!(matches!(
            c.install_library_mod(2, &inst, LoaderKind::Fabric, true)
                .await
                .unwrap(),
            LibraryInstall::Installed { .. }
        ));
        assert_eq!(std::fs::read(mods.join("cool_mod.jar")).unwrap(), warn);
        std::fs::remove_file(mods.join("cool_mod.jar")).unwrap();
        // Bytes that do not match the listed SHA-1 are never installed.
        assert_eq!(
            c.install_library_mod(3, &inst, LoaderKind::Fabric, true)
                .await
                .unwrap_err()
                .code(),
            "download.hashMismatch"
        );
        // Someone else's pending upload and the wrong loader are refused.
        for (id, loader, code) in [
            (4, LoaderKind::Fabric, "library.notFound"),
            (1, LoaderKind::Forge, "library.wrongLoader"),
        ] {
            assert_eq!(
                c.install_library_mod(id, &inst, loader, true)
                    .await
                    .unwrap_err()
                    .code(),
                code
            );
        }
        assert!(!mods.join("cool_mod.jar").exists());
    }

    fn pack(kind: LibraryKind) -> Vec<u8> {
        match kind {
            LibraryKind::ResourcePack => jar(&[(
                "pack.mcmeta",
                br#"{"pack":{"pack_format":46,"description":"Crisp 16x"}}"#.to_vec(),
            )]),
            LibraryKind::Shader => jar(&[("shaders/final.fsh", b"void main(){}".to_vec())]),
            LibraryKind::Modpack => jar(&[(
                "modrinth.index.json",
                br#"{"formatVersion":1,"game":"minecraft","versionId":"2.0","name":"Big Pack",
                     "dependencies":{"minecraft":"1.21.4","fabric-loader":"0.16"},"files":[]}"#
                    .to_vec(),
            )]),
            LibraryKind::Mod => good_jar(),
        }
    }

    fn pack_row(id: i64, kind: LibraryKind, bytes: &[u8]) -> Value {
        let mut r = row(id, bytes, "approved", false);
        r["kind"] = json!(kind.as_str());
        r["loaders"] = json!([]);
        r["file_name"] = json!(format!("item{id}.{}", kind.ext()));
        r
    }

    #[tokio::test]
    async fn submits_packs_as_their_own_file_type() {
        let server = MockServer::builder().start().await;
        let bytes = pack(LibraryKind::ResourcePack);
        let sha = sha1_hex(&bytes);
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/library/{ME}/{sha}.zip")))
            .and(wiremock::matchers::header(
                "content-type",
                "application/zip",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/submit_library_mod"))
            .and(body_partial_json(json!({
                "p_kind": "resourcepack", "p_name": "Crisp 16x", "p_mod_id": "crisp-16x",
                "p_version": "pack_format 46", "p_loaders": [], "p_file_name": "Crisp_16x.zip",
                "p_scan": { "kind": "resourcepack", "verdict": "pass" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(9)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let (id, _) = c
            .submit_library_mod(
                &bytes,
                LibraryKind::ResourcePack,
                "Crisp 16x.zip",
                "",
                "",
                "Steve",
            )
            .await
            .unwrap();
        assert_eq!(id, 9);
        // A jar sent as a resource pack is refused here, before any upload.
        let e = c
            .submit_library_mod(
                &good_jar(),
                LibraryKind::ResourcePack,
                "x.zip",
                "",
                "",
                "Steve",
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "library.blocked");
    }

    #[tokio::test]
    async fn a_bucket_set_up_for_mods_only_says_the_server_is_not_ready() {
        let server = MockServer::builder().start().await;
        let bytes = pack(LibraryKind::Modpack);
        let sha = sha1_hex(&bytes);
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/library/{ME}/{sha}.mrpack"
            )))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "statusCode": "415", "error": "invalid_mime_type",
                "message": "mime type application/x-modrinth-modpack+zip is not supported"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let e = c
            .submit_library_mod(&bytes, LibraryKind::Modpack, "p.mrpack", "P", "", "Steve")
            .await
            .unwrap_err();
        assert_eq!(e.code(), "library.serverNotReady");
    }

    #[tokio::test]
    async fn packs_install_into_their_folders_and_modpacks_download() {
        let server = MockServer::builder().start().await;
        let rp = pack(LibraryKind::ResourcePack);
        let sh = pack(LibraryKind::Shader);
        let mp = pack(LibraryKind::Modpack);
        let mut unknown = pack_row(4, LibraryKind::Shader, &sh);
        unknown["kind"] = json!("datapack");
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/library_list"))
            .and(body_partial_json(json!({ "p_all": true })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                pack_row(1, LibraryKind::ResourcePack, &rp),
                pack_row(2, LibraryKind::Shader, &sh),
                pack_row(3, LibraryKind::Modpack, &mp),
                unknown,
            ])))
            .mount(&server)
            .await;
        for (bytes, ext) in [(&rp, "zip"), (&sh, "zip"), (&mp, "mrpack")] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/storage/v1/object/authenticated/library/{FRIEND}/{}.{ext}",
                    sha1_hex(bytes)
                )))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
                .mount(&server)
                .await;
        }
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let list = c.library_mods().await.unwrap();
        assert_eq!(
            list.iter().map(|m| m.kind).collect::<Vec<_>>(),
            [
                LibraryKind::ResourcePack,
                LibraryKind::Shader,
                LibraryKind::Modpack
            ]
        );
        let inst = tmp.path().join("inst");
        // Packs ignore the loader (a vanilla instance takes a resource pack).
        c.install_library_mod(1, &inst, LoaderKind::Vanilla, false)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(inst.join("resourcepacks/item1.zip")).unwrap(),
            rp
        );
        c.install_library_mod(2, &inst, LoaderKind::Fabric, false)
            .await
            .unwrap();
        assert!(inst.join("shaderpacks/item2.zip").is_file());
        assert_eq!(
            c.install_library_mod(3, &inst, LoaderKind::Fabric, true)
                .await
                .unwrap_err()
                .code(),
            "library.wrongKind"
        );
        let (m, file) = c.download_library_modpack(3, false).await.unwrap().unwrap();
        assert_eq!(m.name, "Mod 3");
        assert_eq!(std::fs::read(&file).unwrap(), mp);
        assert_eq!(
            c.download_library_modpack(1, true)
                .await
                .unwrap_err()
                .code(),
            "library.wrongKind"
        );
    }

    #[tokio::test]
    async fn withdraw_and_report() {
        let server = MockServer::builder().start().await;
        let sha = "a".repeat(40);
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/withdraw_library_mod"))
            .and(body_partial_json(json!({ "p_id": 6 })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(sha)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/storage/v1/object/library"))
            .and(body_partial_json(
                json!({ "prefixes": [format!("{ME}/{sha}.jar"), format!("{ME}/{sha}.mrpack"), format!("{ME}/{sha}.zip")] }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/report_library_mod"))
            .and(body_partial_json(
                json!({ "p_id": 6, "p_reason": "malware", "p_note": "stealer" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.withdraw_library_mod(6).await.unwrap();
        c.report_library_mod(6, LibraryReportReason::Malware, " stealer\u{1b} ")
            .await
            .unwrap();
    }
}
