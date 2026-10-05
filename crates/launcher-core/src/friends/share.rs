//! Sharing an instance's mods with friends and installing a friend's mods.
//!
//! Files Modrinth knows byte-for-byte (same SHA-1) are referenced by their
//! Modrinth CDN URL; other jars are uploaded to the private `mods` bucket
//! under `<owner>/<sha1>.jar`. Installing re-downloads exactly those bytes and
//! verifies the SHA-1, so a friend always gets the same file (K64).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::{FriendsClient, check_uuid};
use crate::content::{install::quiet_progress, installed, modrinth};
use crate::error::{CoreError, Result};
use crate::hash::Checksum;
use crate::instance::Instance;
use crate::instance::files::checked_name;
use crate::net::download::{DownloadItem, Verify};

/// Supabase free-tier upload limit (also set on the bucket).
pub const MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;
const BUCKET: &str = "mods";
const MODRINTH_CDN: &str = "https://cdn.modrinth.com/";
const DISABLED: &str = ".disabled";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum ItemSource {
    /// Identical file on Modrinth.
    Modrinth { url: String, project_id: String },
    /// Uploaded by the owner (not verified by Modrinth).
    Upload { path: String },
    /// Bigger than [`MAX_UPLOAD_BYTES`]; listed but not downloadable.
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedItem {
    pub file_name: String,
    pub sha1: String,
    #[ts(type = "number")]
    pub size: u64,
    pub enabled: bool,
    pub title: Option<String>,
    pub version_number: Option<String>,
    pub source: ItemSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedList {
    #[ts(type = "number")]
    pub id: i64,
    pub owner: String,
    pub instance_id: String,
    pub instance_name: String,
    pub mc_version: String,
    pub loader: String,
    pub items: Vec<SharedItem>,
    pub updated_at: String,
}

/// Database row (snake_case columns).
#[derive(Deserialize)]
struct ListRow {
    id: i64,
    owner: String,
    instance_id: String,
    instance_name: String,
    mc_version: String,
    loader: String,
    items: Vec<SharedItem>,
    updated_at: String,
}

impl From<ListRow> for SharedList {
    fn from(r: ListRow) -> Self {
        Self {
            id: r.id,
            owner: r.owner,
            instance_id: r.instance_id,
            instance_name: r.instance_name,
            mc_version: r.mc_version,
            loader: r.loader,
            items: r.items,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FriendInstallResult {
    pub installed: Vec<String>,
    /// Identical file already in the folder.
    pub already_present: Vec<String>,
    /// A different file with the same name exists; left untouched.
    pub conflicts: Vec<String>,
    /// Not downloadable (too large or invalid entry).
    pub skipped: Vec<String>,
}

const LIST_COLUMNS: &str = "id,owner,instance_id,instance_name,mc_version,loader,items,updated_at";

fn loader_name(inst: &Instance) -> String {
    serde_json::to_value(inst.loader.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn object_path(owner: &str, sha1: &str) -> String {
    format!("{owner}/{sha1}.jar")
}

fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl FriendsClient {
    /// Instance ids the caller currently shares.
    pub async fn my_shares(&self) -> Result<Vec<SharedList>> {
        let me = self.user_id().await?;
        let rows: Vec<ListRow> = self
            .json(
                Method::GET,
                &format!("/rest/v1/shared_lists?select={LIST_COLUMNS}&owner=eq.{me}"),
                None,
                &[],
            )
            .await?;
        Ok(rows.into_iter().map(SharedList::from).collect())
    }

    pub async fn friend_lists(&self, friend: &str) -> Result<Vec<SharedList>> {
        check_uuid(friend)?;
        let rows: Vec<ListRow> = self
            .json(
                Method::GET,
                &format!(
                    "/rest/v1/shared_lists?select={LIST_COLUMNS}&owner=eq.{friend}&order=instance_name.asc"
                ),
                None,
                &[],
            )
            .await?;
        Ok(rows.into_iter().map(SharedList::from).collect())
    }

    async fn list_by_id(&self, id: i64) -> Result<SharedList> {
        let mut rows: Vec<ListRow> = self
            .json(
                Method::GET,
                &format!("/rest/v1/shared_lists?select={LIST_COLUMNS}&id=eq.{id}"),
                None,
                &[],
            )
            .await?;
        rows.pop()
            .map(SharedList::from)
            .ok_or(CoreError::Friends("friends.listNotFound"))
    }

    async fn stored_objects(&self, me: &str) -> Result<HashSet<String>> {
        #[derive(Deserialize)]
        struct Obj {
            name: String,
        }
        let objs: Vec<Obj> = self
            .json(
                Method::POST,
                &format!("/storage/v1/object/list/{BUCKET}"),
                Some(&json!({ "prefix": me, "limit": 1000, "offset": 0 })),
                &[],
            )
            .await?;
        Ok(objs
            .into_iter()
            .map(|o| format!("{me}/{}", o.name))
            .collect())
    }

    /// Publishes (or refreshes) the mod list of `inst`, uploading jars that
    /// Modrinth does not know. Returns the stored list.
    pub async fn share_instance(&self, inst: &Instance, instance_dir: &Path) -> Result<SharedList> {
        let me = self.user_id().await?;
        let mods_dir = instance_dir.join("mods");
        let files = {
            let (ctx, dir) = (self.ctx().clone(), mods_dir.clone());
            tokio::task::spawn_blocking(move || installed::hashes(&ctx, &dir))
                .await
                .map_err(|e| CoreError::FriendsServer {
                    status: 0,
                    reason: e.to_string(),
                })??
        };
        let sha1s: Vec<String> = files.iter().map(|f| f.1.clone()).collect();
        // Offline Modrinth just means everything gets uploaded.
        let known = modrinth::versions_by_sha1(self.ctx(), &sha1s)
            .await
            .unwrap_or_default();
        let stored = self.stored_objects(&me).await?;

        let mut items = Vec::with_capacity(files.len());
        for (name, sha1, size) in files {
            let enabled = !name.ends_with(DISABLED);
            let version = known.get(&sha1);
            let modrinth_file = version.and_then(|v| {
                v.files
                    .iter()
                    .find(|f| f.hashes.sha1.as_deref() == Some(sha1.as_str()))
            });
            let source = match (version, modrinth_file) {
                (Some(v), Some(f)) if f.url.starts_with(MODRINTH_CDN) => ItemSource::Modrinth {
                    url: f.url.clone(),
                    project_id: v.project_id.clone(),
                },
                _ if size > MAX_UPLOAD_BYTES => ItemSource::TooLarge,
                _ => {
                    let path = object_path(&me, &sha1);
                    if !stored.contains(&path) {
                        let bytes = tokio::fs::read(mods_dir.join(&name))
                            .await
                            .map_err(|e| CoreError::io(mods_dir.join(&name), e))?;
                        self.send_raw(
                            Method::POST,
                            &format!("/storage/v1/object/{BUCKET}/{path}"),
                            bytes,
                            "application/java-archive",
                            &[("x-upsert", "true")],
                        )
                        .await?;
                    }
                    ItemSource::Upload { path }
                }
            };
            items.push(SharedItem {
                file_name: name,
                sha1,
                size,
                enabled,
                title: version.map(|v| v.name.clone()),
                version_number: version.map(|v| v.version_number.clone()),
                source,
            });
        }

        let mut rows: Vec<ListRow> = self
            .json(
                Method::POST,
                &format!(
                    "/rest/v1/shared_lists?on_conflict=owner,instance_id&select={LIST_COLUMNS}"
                ),
                Some(&json!({
                    "owner": me,
                    "instance_id": inst.id,
                    "instance_name": inst.name.chars().take(64).collect::<String>(),
                    "mc_version": inst.mc_version,
                    "loader": loader_name(inst),
                    "items": items,
                })),
                &[(
                    "Prefer",
                    "resolution=merge-duplicates,return=representation",
                )],
            )
            .await?;
        let list = rows
            .pop()
            .map(SharedList::from)
            .ok_or(CoreError::Friends("friends.listNotFound"))?;
        self.prune_uploads(&me).await?;
        Ok(list)
    }

    pub async fn unshare_instance(&self, instance_id: &str) -> Result<()> {
        let me = self.user_id().await?;
        let id: String = instance_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        self.send(
            Method::DELETE,
            &format!("/rest/v1/shared_lists?owner=eq.{me}&instance_id=eq.{id}"),
            None,
            &[],
        )
        .await?;
        self.prune_uploads(&me).await
    }

    /// Deletes every jar the caller uploaded. Storage objects can only be
    /// removed through the Storage API, not from SQL (`delete_me`).
    pub(crate) async fn delete_all_uploads(&self) -> Result<()> {
        let me = self.user_id().await?;
        let all: Vec<String> = self.stored_objects(&me).await?.into_iter().collect();
        if !all.is_empty() {
            self.send(
                Method::DELETE,
                &format!("/storage/v1/object/{BUCKET}"),
                Some(&json!({ "prefixes": all })),
                &[],
            )
            .await?;
        }
        Ok(())
    }

    /// Deletes uploaded jars no shared list references any more.
    async fn prune_uploads(&self, me: &str) -> Result<()> {
        let used: HashSet<String> = self
            .my_shares()
            .await?
            .into_iter()
            .flat_map(|l| l.items)
            .filter_map(|i| match i.source {
                ItemSource::Upload { path } => Some(path),
                _ => None,
            })
            .collect();
        let stale: Vec<String> = self
            .stored_objects(me)
            .await?
            .into_iter()
            .filter(|p| !used.contains(p))
            .collect();
        if !stale.is_empty() {
            self.send(
                Method::DELETE,
                &format!("/storage/v1/object/{BUCKET}"),
                Some(&json!({ "prefixes": stale })),
                &[],
            )
            .await?;
        }
        Ok(())
    }

    /// Time-limited download URL for an uploaded jar.
    async fn signed_url(&self, path: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Signed {
            #[serde(rename = "signedURL")]
            signed_url: String,
        }
        let s: Signed = self
            .json(
                Method::POST,
                &format!("/storage/v1/object/sign/{BUCKET}/{path}"),
                Some(&json!({ "expiresIn": 600 })),
                &[],
            )
            .await?;
        Ok(format!(
            "{}/storage/v1{}",
            self.ctx().endpoints.friends.trim_end_matches('/'),
            s.signed_url
        ))
    }

    /// Installs `files` of a friend's list into `instance_dir/mods`.
    pub async fn install_from_list(
        &self,
        list_id: i64,
        instance_dir: &Path,
        files: &[String],
    ) -> Result<FriendInstallResult> {
        let list = self.list_by_id(list_id).await?;
        let mods_dir = instance_dir.join("mods");
        tokio::fs::create_dir_all(&mods_dir)
            .await
            .map_err(|e| CoreError::io(&mods_dir, e))?;
        let present: HashMap<String, String> = {
            let (ctx, dir) = (self.ctx().clone(), mods_dir.clone());
            tokio::task::spawn_blocking(move || installed::hashes(&ctx, &dir))
                .await
                .map_err(|e| CoreError::FriendsServer {
                    status: 0,
                    reason: e.to_string(),
                })??
                .into_iter()
                .map(|(name, sha1, _)| (name, sha1))
                .collect()
        };
        let present_hashes: HashSet<&String> = present.values().collect();

        let mut result = FriendInstallResult::default();
        let mut downloads = Vec::new();
        let wanted: HashSet<&String> = files.iter().collect();
        for item in list.items.iter().filter(|i| wanted.contains(&i.file_name)) {
            // Installed enabled, under the owner's file name.
            let base = item.file_name.trim_end_matches(DISABLED);
            let name_ok = checked_name(base).is_ok() && base.to_ascii_lowercase().ends_with(".jar");
            if !name_ok || !is_sha1(&item.sha1) {
                result.skipped.push(item.file_name.clone());
                continue;
            }
            if present_hashes.contains(&item.sha1.to_ascii_lowercase()) {
                result.already_present.push(base.to_owned());
                continue;
            }
            if present.contains_key(base) || present.contains_key(&format!("{base}{DISABLED}")) {
                result.conflicts.push(base.to_owned());
                continue;
            }
            let url = match &item.source {
                ItemSource::Modrinth { url, .. } if url.starts_with(MODRINTH_CDN) => url.clone(),
                ItemSource::Upload { path } if *path == object_path(&list.owner, &item.sha1) => {
                    self.signed_url(path).await?
                }
                _ => {
                    result.skipped.push(base.to_owned());
                    continue;
                }
            };
            downloads.push(DownloadItem {
                url,
                dest: mods_dir.join(base),
                checksum: Some(Checksum::Sha1(item.sha1.to_ascii_lowercase())),
                size: Some(item.size),
            });
            result.installed.push(base.to_owned());
        }
        if !downloads.is_empty() {
            // A bad hash will not fix itself; retry only briefly.
            self.ctx()
                .downloader()
                .with_backoff(std::time::Duration::from_millis(200), 2)
                .run(
                    downloads,
                    Verify::Full,
                    &quiet_progress(),
                    &CancellationToken::new(),
                )
                .await?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sha1::{Digest, Sha1};
    use wiremock::matchers::{body_partial_json, header, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, ME, signed_in};
    use super::*;

    fn sha1_hex(b: &[u8]) -> String {
        hex::encode(Sha1::digest(b))
    }

    fn instance() -> Instance {
        serde_json::from_value(json!({
            "id": "inst-1", "name": "Survival", "mcVersion": "1.21.4",
            "loader": { "kind": "fabric" }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn shares_modrinth_files_by_url_and_uploads_the_rest() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let inst_dir = tmp.path().join("inst");
        std::fs::create_dir_all(inst_dir.join("mods")).unwrap();
        let (known, own) = (b"sodium-bytes".to_vec(), b"my-own-mod".to_vec());
        std::fs::write(inst_dir.join("mods/sodium.jar"), &known).unwrap();
        std::fs::write(inst_dir.join("mods/own.jar.disabled"), &own).unwrap();
        let (known_sha, own_sha) = (sha1_hex(&known), sha1_hex(&own));

        let mut by_hash = serde_json::Map::new();
        by_hash.insert(
            known_sha.clone(),
            json!({
                    "id": "ver1", "project_id": "AANobbMI", "name": "Sodium 0.6",
                    "version_number": "0.6.0", "version_type": "release",
                    "game_versions": ["1.21.4"], "loaders": ["fabric"],
                    "date_published": "2026-01-01T00:00:00Z", "dependencies": [],
                    "files": [{ "url": "https://cdn.modrinth.com/data/AANobbMI/versions/ver1/sodium.jar",
                                "filename": "sodium.jar", "primary": true, "size": 12,
                                "hashes": { "sha1": known_sha.clone() } }]
            }),
        );
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::Value::Object(by_hash)),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/list/{BUCKET}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/{BUCKET}/{ME}/{own_sha}.jar"
            )))
            .and(header("x-upsert", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Key": "k" })))
            .expect(1)
            .mount(&server)
            .await;
        let row = |items: serde_json::Value| {
            json!([{ "id": 5, "owner": ME, "instance_id": "inst-1", "instance_name": "Survival",
                     "mc_version": "1.21.4", "loader": "fabric", "items": items,
                     "updated_at": "2026-10-05T10:00:00Z" }])
        };
        Mock::given(method("POST"))
            .and(path("/rest/v1/shared_lists"))
            .and(body_partial_json(
                json!({ "loader": "fabric", "instance_id": "inst-1" }),
            ))
            .respond_with(move |req: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                ResponseTemplate::new(201).set_body_json(row(body["items"].clone()))
            })
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/shared_lists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let c = signed_in(&tmp, &server).await;
        let list = c.share_instance(&instance(), &inst_dir).await.unwrap();
        let by_name: HashMap<_, _> = list
            .items
            .iter()
            .map(|i| (i.file_name.as_str(), i))
            .collect();
        assert!(matches!(
            by_name["sodium.jar"].source,
            ItemSource::Modrinth { .. }
        ));
        assert_eq!(by_name["sodium.jar"].title.as_deref(), Some("Sodium 0.6"));
        let own_item = by_name["own.jar.disabled"];
        assert!(!own_item.enabled);
        assert_eq!(
            own_item.source,
            ItemSource::Upload {
                path: format!("{ME}/{own_sha}.jar")
            }
        );
    }

    #[tokio::test]
    async fn installs_verified_files_and_refuses_bad_entries() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let inst_dir = tmp.path().join("inst");
        std::fs::create_dir_all(inst_dir.join("mods")).unwrap();
        // Already have one of them, and a different file named like another.
        std::fs::write(inst_dir.join("mods/have.jar"), b"have").unwrap();
        std::fs::write(inst_dir.join("mods/clash.jar"), b"mine").unwrap();

        let good = b"friend-mod".to_vec();
        let tampered = b"tampered".to_vec();
        let item = |name: &str, sha1: String, source: serde_json::Value| {
            json!({ "fileName": name, "sha1": sha1, "size": 10, "enabled": true,
                    "title": null, "versionNumber": null, "source": source })
        };
        let items = json!([
            item(
                "good.jar.disabled",
                sha1_hex(&good),
                json!({ "kind": "upload", "path": format!("{FRIEND}/{}.jar", sha1_hex(&good)) })
            ),
            item("have.jar", sha1_hex(b"have"), json!({ "kind": "tooLarge" })),
            item(
                "clash.jar",
                sha1_hex(b"theirs"),
                json!({ "kind": "tooLarge" })
            ),
            item(
                "../evil.jar",
                sha1_hex(&good),
                json!({ "kind": "tooLarge" })
            ),
            item("big.jar", sha1_hex(b"big"), json!({ "kind": "tooLarge" })),
            item(
                "phish.jar",
                sha1_hex(&good),
                json!({ "kind": "modrinth", "url": "https://evil.example/x.jar", "projectId": "p" })
            ),
            item(
                "other.jar",
                sha1_hex(&good),
                json!({ "kind": "upload", "path": format!("{ME}/{}.jar", sha1_hex(&good)) })
            ),
            item(
                "tampered.jar",
                sha1_hex(b"expected"),
                json!({ "kind": "upload", "path": format!("{FRIEND}/{}.jar", sha1_hex(b"expected")) })
            ),
        ]);
        Mock::given(method("GET"))
            .and(path("/rest/v1/shared_lists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": 9, "owner": FRIEND, "instance_id": "x", "instance_name": "X",
                "mc_version": "1.21.4", "loader": "fabric", "items": items,
                "updated_at": "2026-10-05T10:00:00Z" }])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path_regex(r"^/storage/v1/object/sign/mods/.+"))
            .respond_with(move |req: &wiremock::Request| {
                let obj = req
                    .url
                    .path()
                    .trim_start_matches("/storage/v1/object/sign/mods/");
                ResponseTemplate::new(200).set_body_json(
                    json!({ "signedURL": format!("/object/sign/mods/{obj}?token=t") }),
                )
            })
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/sign/mods/{FRIEND}/{}.jar",
                sha1_hex(&good)
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(good.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/sign/mods/{FRIEND}/{}.jar",
                sha1_hex(b"expected")
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tampered.clone()))
            .mount(&server)
            .await;

        let c = signed_in(&tmp, &server).await;
        let all: Vec<String> = [
            "good.jar.disabled",
            "have.jar",
            "clash.jar",
            "../evil.jar",
            "big.jar",
            "phish.jar",
            "other.jar",
        ]
        .map(String::from)
        .to_vec();
        let r = c.install_from_list(9, &inst_dir, &all).await.unwrap();
        assert_eq!(r.installed, ["good.jar"]);
        assert_eq!(r.already_present, ["have.jar"]);
        assert_eq!(r.conflicts, ["clash.jar"]);
        assert_eq!(
            r.skipped,
            ["../evil.jar", "big.jar", "phish.jar", "other.jar"]
        );
        assert_eq!(std::fs::read(inst_dir.join("mods/good.jar")).unwrap(), good);
        assert_eq!(
            std::fs::read(inst_dir.join("mods/clash.jar")).unwrap(),
            b"mine"
        );

        // A file whose bytes do not match the listed SHA-1 is never installed.
        let e = c
            .install_from_list(9, &inst_dir, &["tampered.jar".to_owned()])
            .await
            .unwrap_err();
        assert_eq!(e.code(), "download.hashMismatch");
        assert!(!inst_dir.join("mods/tampered.jar").exists());
    }
}
