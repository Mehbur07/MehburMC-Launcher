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
use crate::content::scan::{self, Finding, ModLoader, ScanReport, Severity, Verdict};
use crate::error::{CoreError, Result};
use crate::instance::LoaderKind;
use crate::instance::files::checked_name;

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

/// A file name the server accepts (`[A-Za-z0-9._+-]`, ends with `.jar`).
pub fn library_file_name(name: &str) -> String {
    let stem = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim_end_matches(".disabled");
    let stem = stem
        .strip_suffix(".jar")
        .or_else(|| stem.strip_suffix(".JAR"))
        .unwrap_or(stem);
    let mut s: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(MAX_FILE_NAME_CHARS - 4)
        .collect();
    s = s.trim_matches('.').to_owned();
    if s.is_empty() {
        s = "mod".into();
    }
    format!("{s}.jar")
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
        let loaders: Vec<ModLoader> = self
            .loaders
            .iter()
            .filter_map(|l| parse_loader(l))
            .collect();
        Some(LibraryMod {
            id: self.id,
            author: clean_text(&self.author, 16, false),
            name: clean_text(&self.name, MAX_NAME_CHARS, false),
            description: clean_text(&self.description, MAX_DESCRIPTION_CHARS, true),
            mod_id: clean_text(&self.mod_id, 64, false),
            version: clean_text(&self.version, 64, false),
            loaders,
            game_versions: clean_text(&self.game_versions, 100, false),
            sha1: self.sha1.to_ascii_lowercase(),
            size: self.size,
            file_name: library_file_name(&self.file_name),
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
    /// Checks `jar` and submits it for review; returns the library id.
    /// Blocked jars never leave the computer.
    pub async fn submit_library_mod(
        &self,
        jar: &[u8],
        file_name: &str,
        name: &str,
        description: &str,
        author: &str,
    ) -> Result<(i64, ScanReport)> {
        let report = {
            let jar = jar.to_vec();
            tokio::task::spawn_blocking(move || scan::scan(&jar))
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
        let name = match clean_text(name, MAX_NAME_CHARS, false) {
            n if n.is_empty() => d.name.chars().take(MAX_NAME_CHARS).collect(),
            n => n,
        };
        let version = match d.version.chars().take(64).collect::<String>() {
            v if v.is_empty() => "unknown".to_owned(),
            v => v,
        };
        self.ensure_identity().await?;
        let me = self.user_id().await?;
        self.send_raw(
            Method::POST,
            &format!("/storage/v1/object/{BUCKET}/{me}/{}.jar", report.sha1),
            jar.to_vec(),
            "application/java-archive",
            &[("x-upsert", "true")],
        )
        .await?;
        let loaders: Vec<&str> = d.loaders.iter().map(|l| loader_str(*l)).collect();
        let id = self
            .rpc(
                "submit_library_mod",
                json!({
                    "p_name": name,
                    "p_description": clean_text(description, MAX_DESCRIPTION_CHARS, true),
                    "p_author": author.trim(),
                    "p_mod_id": d.mod_id,
                    "p_version": version,
                    "p_loaders": loaders,
                    "p_game_versions": d.game_versions,
                    "p_sha1": report.sha1,
                    "p_size": report.size,
                    "p_file_name": library_file_name(file_name),
                    "p_scan": report,
                }),
            )
            .await?;
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
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": [format!("{me}/{}.jar", sha1.to_ascii_lowercase())] })),
            &[],
        )
        .await
        .map(drop)
    }

    /// Approved mods and the caller's own uploads (own first).
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
        self.rpc("library_list", json!({ "p_max": 300 })).await
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

    /// Downloads library mod `id` into `mods_dir` after verifying its
    /// SHA-1 and scanning it here. Refuses blocked files; asks back
    /// ([`LibraryInstall::NeedsConfirm`]) when the scan has warnings and
    /// `accept_warnings` is false.
    pub async fn install_library_mod(
        &self,
        id: i64,
        mods_dir: &Path,
        loader: LoaderKind,
        accept_warnings: bool,
    ) -> Result<LibraryInstall> {
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
        if !fits(&m.loaders, loader) {
            return Err(CoreError::Friends("library.wrongLoader"));
        }
        let file_name = checked_name(&m.file_name)?.to_owned();
        tokio::fs::create_dir_all(mods_dir)
            .await
            .map_err(|e| CoreError::io(mods_dir, e))?;
        let present = {
            let (ctx, dir) = (self.ctx().clone(), mods_dir.to_path_buf());
            tokio::task::spawn_blocking(move || crate::content::installed::hashes(&ctx, &dir))
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

        let bytes = self.library_jar(&owner, &m.sha1).await?;
        let report = tokio::task::spawn_blocking(move || {
            let r = scan::scan(&bytes);
            (r, bytes)
        })
        .await
        .map_err(|e| CoreError::FriendsServer {
            status: 0,
            reason: e.to_string(),
        })?;
        let (report, bytes) = report;
        match report.verdict {
            Verdict::Block => return Err(CoreError::Friends("library.blocked")),
            Verdict::Warn if !accept_warnings => {
                return Ok(LibraryInstall::NeedsConfirm {
                    findings: report.findings,
                });
            }
            _ => {}
        }
        let dest = mods_dir.join(&file_name);
        crate::fsutil::write_atomic(&dest, &bytes)?;
        Ok(LibraryInstall::Installed { file_name })
    }

    /// The jar from the cache or the server, verified against `sha1`.
    pub(crate) async fn library_jar(&self, owner: &str, sha1: &str) -> Result<Vec<u8>> {
        let cached = self
            .ctx()
            .paths
            .cache()
            .join("library")
            .join(format!("{sha1}.jar"));
        if let Ok(bytes) = std::fs::read(&cached)
            && sha1_hex(&bytes) == sha1
        {
            return Ok(bytes);
        }
        let bytes = self
            .send(
                Method::GET,
                &format!("/storage/v1/object/authenticated/{BUCKET}/{owner}/{sha1}.jar"),
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
            tracing::debug!(error = %e.detail(), "could not cache a library mod");
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
    use wiremock::matchers::{body_partial_json, method, path};
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
            library_file_name("Cool Mod (1.2).jar"),
            "Cool_Mod__1.2_.jar"
        );
        assert_eq!(
            library_file_name("C:\\x\\sodium+mc.jar.disabled"),
            "sodium+mc.jar"
        );
        assert_eq!(library_file_name("..jar"), "mod.jar");
        assert!(library_file_name(&"a".repeat(300)).len() <= MAX_FILE_NAME_CHARS);
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
                "p_scan": { "verdict": "pass" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(4)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let (id, report) = c
            .submit_library_mod(&bytes, "cool mod.jar", "  ", "desc", "Steve")
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
            .submit_library_mod(&evil, "e.jar", "Evil", "", "Steve")
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
        let mods = tmp.path().join("inst/mods");

        let r = c
            .install_library_mod(1, &mods, LoaderKind::Fabric, false)
            .await
            .unwrap();
        assert!(
            matches!(r, LibraryInstall::Installed { ref file_name } if file_name == "cool_mod.jar")
        );
        assert_eq!(std::fs::read(mods.join("cool_mod.jar")).unwrap(), good);
        // Same bytes again → already there.
        assert!(matches!(
            c.install_library_mod(1, &mods, LoaderKind::Fabric, false)
                .await
                .unwrap(),
            LibraryInstall::AlreadyPresent { .. }
        ));
        // Same name as an installed file → conflict (checked before downloading).
        assert_eq!(
            c.install_library_mod(2, &mods, LoaderKind::Fabric, true)
                .await
                .unwrap_err()
                .code(),
            "library.conflict"
        );
        std::fs::remove_file(mods.join("cool_mod.jar")).unwrap();
        // Warnings found here → ask first, install only after consent.
        assert!(matches!(
            c.install_library_mod(2, &mods, LoaderKind::Fabric, false)
                .await
                .unwrap(),
            LibraryInstall::NeedsConfirm { .. }
        ));
        assert!(!mods.join("cool_mod.jar").exists());
        assert!(matches!(
            c.install_library_mod(2, &mods, LoaderKind::Fabric, true)
                .await
                .unwrap(),
            LibraryInstall::Installed { .. }
        ));
        assert_eq!(std::fs::read(mods.join("cool_mod.jar")).unwrap(), warn);
        std::fs::remove_file(mods.join("cool_mod.jar")).unwrap();
        // Bytes that do not match the listed SHA-1 are never installed.
        assert_eq!(
            c.install_library_mod(3, &mods, LoaderKind::Fabric, true)
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
                c.install_library_mod(id, &mods, loader, true)
                    .await
                    .unwrap_err()
                    .code(),
                code
            );
        }
        assert!(!mods.join("cool_mod.jar").exists());
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
                json!({ "prefixes": [format!("{ME}/{sha}.jar")] }),
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
