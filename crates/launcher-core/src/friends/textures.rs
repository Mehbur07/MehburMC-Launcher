//! Shared skins and capes (ARCHITECTURE K70): the owner uploads the PNG to
//! the private `textures` bucket as `<uid>/<sha1>.png`, then `share_texture`
//! records it as public or friends-only. Who may see a share is decided on
//! the server (`supabase/phase17.sql`); images of other users are verified
//! against their SHA-1 and cached in `cache/textures/`.

use futures_util::{StreamExt, stream};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::FriendsClient;
use super::avatar::sha1_hex;
use crate::auth::avatar::data_uri;
use crate::error::{CoreError, Result};
use crate::skin::image::{self, decode_limited};
use crate::skin::{SkinModel, TextureKind};

const BUCKET: &str = "textures";
/// Matches the bucket's file size limit.
pub const MAX_BYTES: usize = 128 * 1024;
/// HD skins up to 512×512 (and old 22×17 cape multiples).
const MAX_SIDE: u32 = 512;
const MAX_NOTE_CHARS: usize = 500;
/// Images fetched in parallel when the community list loads.
const PARALLEL_FETCHES: usize = 8;

/// Codes the server raises for shares; each has an `errors.textures.*`
/// translation.
pub const TEXTURE_ERRORS: &[&str] = &[
    "textures.authorInvalid",
    "textures.fileMissing",
    "textures.quota",
    "textures.notFound",
    "textures.ownReport",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Visibility {
    /// Every MehburMC user.
    Public,
    /// Accepted friends only.
    Friends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportReason {
    Inappropriate,
    Stolen,
    Spam,
    Other,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedTexture {
    /// Server id of the share.
    #[ts(type = "number")]
    pub id: i64,
    pub kind: TextureKind,
    pub model: SkinModel,
    pub name: String,
    /// Account name of the person who shared it.
    pub author: String,
    /// SHA-1 of the PNG (= the library id of the same image).
    pub sha1: String,
    pub visibility: Visibility,
    /// RFC 3339.
    pub created_at: String,
    /// Shared by this installation.
    pub mine: bool,
    pub data_uri: String,
}

#[derive(Debug, Deserialize)]
struct Row {
    id: i64,
    owner: String,
    kind: TextureKind,
    model: SkinModel,
    name: String,
    author: String,
    sha1: String,
    visibility: Visibility,
    created_at: String,
    mine: bool,
}

/// A PNG the server or a user sends for sharing: size, format and layout.
pub fn validate(kind: TextureKind, png: &[u8]) -> Result<()> {
    let img =
        decode_limited(png, MAX_BYTES, MAX_SIDE).map_err(|r| CoreError::SkinInvalid(r.clone()))?;
    match kind {
        TextureKind::Skin => image::check_skin(&img),
        TextureKind::Cape => image::check_cape(&img),
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl FriendsClient {
    /// Uploads `png` and shares it; returns the share id. Friends-only
    /// shares need friends turned on.
    pub async fn share_texture(
        &self,
        kind: TextureKind,
        model: SkinModel,
        name: &str,
        author: &str,
        png: &[u8],
        visibility: Visibility,
    ) -> Result<i64> {
        validate(kind, png)?;
        if visibility == Visibility::Friends && !self.is_enabled().await {
            return Err(CoreError::Friends("friends.disabled"));
        }
        self.ensure_identity().await?;
        let me = self.user_id().await?;
        let sha1 = sha1_hex(png);
        self.send_raw(
            Method::POST,
            &format!("/storage/v1/object/{BUCKET}/{me}/{sha1}.png"),
            png.to_vec(),
            "image/png",
            &[("x-upsert", "true")],
        )
        .await?;
        let model = match kind {
            TextureKind::Skin => model,
            TextureKind::Cape => SkinModel::Classic,
        };
        self.rpc(
            "share_texture",
            json!({
                "kind": kind,
                "model": model,
                "name": name.trim(),
                "author": author.trim(),
                "sha1": sha1,
                "visibility": visibility,
            }),
        )
        .await
    }

    /// Withdraws one of the caller's shares and deletes its file.
    pub async fn unshare_texture(&self, id: i64) -> Result<()> {
        let me = self.user_id().await?;
        let sha1: String = self
            .rpc("unshare_texture", json!({ "texture_id": id }))
            .await?;
        if !is_sha1(&sha1) {
            return Ok(());
        }
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": [format!("{me}/{}.png", sha1.to_ascii_lowercase())] })),
            &[],
        )
        .await
        .map(drop)
    }

    /// Shares the caller can see (own first). Entries whose image cannot be
    /// fetched or verified are left out.
    pub async fn community_textures(&self) -> Result<Vec<SharedTexture>> {
        if !self.has_identity().await {
            self.ensure_identity().await?;
        }
        let rows: Vec<Row> = self
            .rpc("community_textures", json!({ "max_rows": 300 }))
            .await?;
        let out: Vec<Option<SharedTexture>> = stream::iter(rows)
            .map(|r| async move {
                let png = self.texture_png(&r.owner, &r.sha1, r.kind).await?;
                Some(SharedTexture {
                    id: r.id,
                    kind: r.kind,
                    model: r.model,
                    name: r.name,
                    author: r.author,
                    sha1: r.sha1.to_ascii_lowercase(),
                    visibility: r.visibility,
                    created_at: r.created_at,
                    mine: r.mine,
                    data_uri: data_uri(&png),
                })
            })
            .buffered(PARALLEL_FETCHES)
            .collect()
            .await;
        Ok(out.into_iter().flatten().collect())
    }

    /// A shared image from the cache or the server, verified.
    async fn texture_png(&self, owner: &str, sha1: &str, kind: TextureKind) -> Option<Vec<u8>> {
        if !is_uuid(owner) || !is_sha1(sha1) {
            return None;
        }
        let sha1 = sha1.to_ascii_lowercase();
        let cached = self
            .ctx()
            .paths
            .cache()
            .join("textures")
            .join(format!("{owner}-{sha1}.png"));
        if let Ok(bytes) = std::fs::read(&cached)
            && sha1_hex(&bytes) == sha1
        {
            return Some(bytes);
        }
        let bytes = self
            .send(
                Method::GET,
                &format!("/storage/v1/object/authenticated/{BUCKET}/{owner}/{sha1}.png"),
                None,
                &[],
            )
            .await
            .map_err(|e| tracing::debug!(error = %e.detail(), "shared texture unavailable"))
            .ok()?;
        if sha1_hex(&bytes) != sha1 || validate(kind, &bytes).is_err() {
            tracing::warn!(%owner, %sha1, "shared texture failed verification");
            return None;
        }
        if let Err(e) = crate::fsutil::write_atomic(&cached, &bytes) {
            tracing::debug!(error = %e.detail(), "could not cache a shared texture");
        }
        Some(bytes)
    }

    pub async fn report_texture(&self, id: i64, reason: ReportReason, note: &str) -> Result<()> {
        let note: String = note
            .trim()
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .take(MAX_NOTE_CHARS)
            .collect();
        self.rpc_void(
            "report_texture",
            json!({ "texture_id": id, "reason": reason, "note": note }),
        )
        .await
    }

    /// Deletes every texture file of the caller (before `delete_me`; the
    /// rows go with the identity).
    pub(crate) async fn delete_all_textures(&self) -> Result<()> {
        #[derive(Deserialize)]
        struct Obj {
            name: String,
        }
        let me = self.user_id().await?;
        let objs: Vec<Obj> = self
            .json(
                Method::POST,
                &format!("/storage/v1/object/list/{BUCKET}"),
                Some(&json!({ "prefix": me, "limit": 1000, "offset": 0 })),
                &[],
            )
            .await?;
        if objs.is_empty() {
            return Ok(());
        }
        let paths: Vec<String> = objs.iter().map(|o| format!("{me}/{}", o.name)).collect();
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": paths })),
            &[],
        )
        .await
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, ME, signed_in};
    use super::*;
    use crate::skin::image::tests::png;

    fn skin() -> Vec<u8> {
        png(64, 64, |_, _| false)
    }

    #[test]
    fn validates_layout_and_size() {
        validate(TextureKind::Skin, &skin()).unwrap();
        validate(TextureKind::Cape, &png(64, 32, |_, _| false)).unwrap();
        assert!(validate(TextureKind::Cape, &skin()).is_err());
        assert!(validate(TextureKind::Skin, &png(30, 30, |_, _| false)).is_err());
        assert!(validate(TextureKind::Skin, b"not a png").is_err());
        assert!(validate(TextureKind::Skin, &vec![0u8; MAX_BYTES + 1]).is_err());
    }

    #[tokio::test]
    async fn shares_by_uploading_then_recording() {
        let server = MockServer::builder().start().await;
        let img = skin();
        let sha = sha1_hex(&img);
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/textures/{ME}/{sha}.png")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/share_texture"))
            .and(body_json(json!({
                "kind": "skin", "model": "slim", "name": "Ninja", "author": "Steve",
                "sha1": sha, "visibility": "public"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(7)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let id = c
            .share_texture(
                TextureKind::Skin,
                SkinModel::Slim,
                " Ninja ",
                "Steve",
                &img,
                Visibility::Public,
            )
            .await
            .unwrap();
        assert_eq!(id, 7);
        // Wrong layout never reaches the server (the mocks expect one call).
        assert!(
            c.share_texture(
                TextureKind::Cape,
                SkinModel::Classic,
                "x",
                "Steve",
                &img,
                Visibility::Public
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn server_refusals_have_codes() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/unshare_texture"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({ "code": "P0001", "message": "textures.notFound" })),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(
            c.unshare_texture(1).await.unwrap_err().code(),
            "textures.notFound"
        );
    }

    #[tokio::test]
    async fn community_list_verifies_images() {
        let server = MockServer::builder().start().await;
        let good = skin();
        let good_sha = sha1_hex(&good);
        let lying = "1111111111111111111111111111111111111111";
        let row = |id: i64, sha: &str, mine: bool| {
            json!({
                "id": id, "owner": FRIEND, "kind": "skin", "model": "classic",
                "name": format!("n{id}"), "author": "Alex", "sha1": sha,
                "visibility": "friends", "created_at": "2026-10-06T10:00:00Z", "mine": mine
            })
        };
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/community_textures"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                row(1, &good_sha, false),
                row(2, lying, false),
                row(3, "../../etc", false)
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/textures/{FRIEND}/{good_sha}.png"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(good.clone()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/textures/{FRIEND}/{lying}.png"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(good.clone()))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let list = c.community_textures().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].visibility, Visibility::Friends);
        assert!(list[0].data_uri.starts_with("data:image/png;base64,"));
        // Second load comes from the cache (the image mock expects one call).
        assert_eq!(c.community_textures().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn reports_with_a_clean_note() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/report_texture"))
            .and(body_partial_json(
                json!({ "texture_id": 5, "reason": "stolen", "note": "kopya" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.report_texture(5, ReportReason::Stolen, "  kopya\u{7} ")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn friends_only_needs_friends_on() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.set_friends_enabled(false).await.unwrap();
        let e = c
            .share_texture(
                TextureKind::Skin,
                SkinModel::Classic,
                "n",
                "Steve",
                &skin(),
                Visibility::Friends,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code(), "friends.disabled");
    }
}
