//! Private MehburMC textures (ARCHITECTURE K74). The founder uploads them
//! to the private `private_textures` bucket as `<sha1>.png` and grants them
//! to chosen accounts (`supabase/phase21.sql`); the designs never enter the
//! repository. A receiver's launcher downloads its grants, checks them
//! against their SHA-1 and adds them to the skin library; a revoked grant
//! takes the texture away again. Private textures cannot be shared.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use super::FriendsClient;
use super::avatar::sha1_hex;
use super::textures::validate;
use crate::auth::avatar::data_uri;
use crate::error::{CoreError, Result};
use crate::skin::{SkinModel, TextureKind};

const BUCKET: &str = "private_textures";
const MAX_NAME_CHARS: usize = 48;

/// A texture granted to the signed-in account.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Grant {
    pub id: i64,
    pub kind: TextureKind,
    pub model: SkinModel,
    pub name: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrivateTexture {
    #[ts(type = "number")]
    pub id: i64,
    pub kind: TextureKind,
    pub model: SkinModel,
    pub name: String,
    pub sha1: String,
    pub created_at: String,
    #[ts(type = "number")]
    pub grants: i64,
    /// `None` if the file could not be fetched.
    pub data_uri: Option<String>,
}

#[derive(Deserialize)]
struct AdminRow {
    id: i64,
    kind: TextureKind,
    model: SkinModel,
    name: String,
    sha1: String,
    created_at: String,
    grants: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TextureGrant {
    pub user_id: String,
    pub names: Vec<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
struct GrantRow {
    user_id: String,
    names: Option<Vec<String>>,
    created_at: String,
}

fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl FriendsClient {
    /// The caller's grants (well-formed ones only).
    pub async fn my_private_textures(&self) -> Result<Vec<Grant>> {
        let rows: Vec<Grant> = self.rpc("my_private_textures", json!({})).await?;
        Ok(rows
            .into_iter()
            .filter(|g| is_sha1(&g.sha1))
            .map(|g| Grant {
                sha1: g.sha1.to_ascii_lowercase(),
                name: g
                    .name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(MAX_NAME_CHARS)
                    .collect(),
                ..g
            })
            .collect())
    }

    /// Downloads a private texture and checks its SHA-1 and layout.
    pub async fn private_texture_png(&self, sha1: &str, kind: TextureKind) -> Result<Vec<u8>> {
        if !is_sha1(sha1) {
            return Err(CoreError::Friends("textures.notFound"));
        }
        let sha1 = sha1.to_ascii_lowercase();
        let bytes = self
            .send(
                Method::GET,
                &format!("/storage/v1/object/authenticated/{BUCKET}/{sha1}.png"),
                None,
                &[],
            )
            .await?;
        if sha1_hex(&bytes) != sha1 {
            return Err(CoreError::HashMismatch {
                path: format!("{BUCKET}/{sha1}.png").into(),
                expected: sha1,
                actual: sha1_hex(&bytes),
            });
        }
        validate(kind, &bytes)?;
        Ok(bytes)
    }

    /// Founder: every private texture with a preview and its grant count.
    pub async fn admin_private_textures(&self) -> Result<Vec<PrivateTexture>> {
        let rows: Vec<AdminRow> = self.rpc("admin_private_textures", json!({})).await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let data_uri = match self.private_texture_png(&r.sha1, r.kind).await {
                Ok(png) => Some(data_uri(&png)),
                Err(e) => {
                    tracing::warn!(error = %e.detail(), sha1 = %r.sha1, "private texture unavailable");
                    None
                }
            };
            out.push(PrivateTexture {
                id: r.id,
                kind: r.kind,
                model: r.model,
                name: r.name,
                sha1: r.sha1,
                created_at: r.created_at,
                grants: r.grants,
                data_uri,
            });
        }
        Ok(out)
    }

    /// Founder: uploads a PNG as a private texture; returns its id.
    pub async fn admin_upload_private_texture(
        &self,
        kind: TextureKind,
        model: SkinModel,
        name: &str,
        png: &[u8],
    ) -> Result<i64> {
        validate(kind, png)?;
        let name: String = name
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME_CHARS)
            .collect();
        let name = if name.is_empty() {
            "MehburMC".to_owned()
        } else {
            name
        };
        let sha1 = sha1_hex(png);
        self.send_raw(
            Method::POST,
            &format!("/storage/v1/object/{BUCKET}/{sha1}.png"),
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
            "admin_add_private_texture",
            json!({ "p_kind": kind, "p_model": model, "p_name": name, "p_sha1": sha1 }),
        )
        .await
    }

    /// Founder: deletes a private texture, its grants and its file.
    pub async fn admin_delete_private_texture(&self, id: i64) -> Result<()> {
        let sha1: String = self
            .rpc("admin_delete_private_texture", json!({ "p_id": id }))
            .await?;
        if !is_sha1(&sha1) {
            return Ok(());
        }
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": [format!("{}.png", sha1.to_ascii_lowercase())] })),
            &[],
        )
        .await
        .map(drop)
    }

    pub async fn admin_texture_grants(&self, id: i64) -> Result<Vec<TextureGrant>> {
        let rows: Vec<GrantRow> = self
            .rpc("admin_texture_grants", json!({ "p_id": id }))
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| TextureGrant {
                user_id: r.user_id,
                names: r.names.unwrap_or_default(),
                created_at: r.created_at,
            })
            .collect())
    }

    /// Founder: gives (`true`) or takes back a private texture.
    pub async fn admin_set_texture_grant(&self, id: i64, user_id: &str, grant: bool) -> Result<()> {
        super::check_uuid(user_id)?;
        self.rpc_void(
            if grant {
                "admin_grant_texture"
            } else {
                "admin_revoke_texture"
            },
            json!({ "p_id": id, "p_user": user_id }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, signed_in};
    use super::*;
    use crate::skin::image::tests::png;

    #[tokio::test]
    async fn grants_are_verified_downloads() {
        let server = MockServer::builder().start().await;
        let skin = png(64, 64, |x, _| x == 1);
        let sha = sha1_hex(&skin);
        let lying = "1".repeat(40);
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/my_private_textures"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 1, "kind": "skin", "model": "slim", "name": "MehburMC\u{7}", "sha1": sha },
                { "id": 2, "kind": "skin", "model": "classic", "name": "Bad", "sha1": "../x" },
            ])))
            .mount(&server)
            .await;
        for s in [&sha, &lying] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/storage/v1/object/authenticated/private_textures/{s}.png"
                )))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(skin.clone()))
                .mount(&server)
                .await;
        }
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let grants = c.my_private_textures().await.unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(
            (grants[0].name.as_str(), grants[0].model),
            ("MehburMC", SkinModel::Slim)
        );
        assert_eq!(
            c.private_texture_png(&sha, TextureKind::Skin)
                .await
                .unwrap(),
            skin
        );
        // Wrong bytes for the SHA-1, or a skin where a cape is expected.
        assert_eq!(
            c.private_texture_png(&lying, TextureKind::Skin)
                .await
                .unwrap_err()
                .code(),
            "download.hashMismatch"
        );
        assert!(
            c.private_texture_png(&sha, TextureKind::Cape)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn founder_uploads_then_records() {
        let server = MockServer::builder().start().await;
        let cape = png(64, 32, |_, _| false);
        let sha = sha1_hex(&cape);
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/private_textures/{sha}.png"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_add_private_texture"))
            .and(body_json(json!({
                "p_kind": "cape", "p_model": "classic", "p_name": "MehburMC", "p_sha1": sha
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(5)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/admin_grant_texture"))
            .and(body_json(json!({ "p_id": 5, "p_user": FRIEND })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let id = c
            .admin_upload_private_texture(TextureKind::Cape, SkinModel::Slim, "  ", &cape)
            .await
            .unwrap();
        assert_eq!(id, 5);
        // A skin-sized image is not a cape; nothing is uploaded.
        assert!(
            c.admin_upload_private_texture(
                TextureKind::Cape,
                SkinModel::Classic,
                "x",
                &png(64, 64, |_, _| false)
            )
            .await
            .is_err()
        );
        c.admin_set_texture_grant(5, FRIEND, true).await.unwrap();
        assert!(c.admin_set_texture_grant(5, "nope", true).await.is_err());
    }
}
