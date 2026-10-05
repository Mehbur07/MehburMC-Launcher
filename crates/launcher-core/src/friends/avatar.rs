//! Profile photos shown to friends (ARCHITECTURE K65): the selected
//! account's photo (or skin head) is uploaded to the private `avatars` bucket
//! as `<uid>/<sha1>.png`; friends' photos are fetched once per SHA-1 and kept
//! in `cache/avatars/`.

use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::FriendsClient;
use crate::auth::avatar::data_uri;
use crate::error::{CoreError, Result};
use crate::skin::image::decode_limited;

const BUCKET: &str = "avatars";
/// Matches the bucket's file size limit.
pub const MAX_BYTES: usize = 256 * 1024;
/// Side of the uploaded image.
pub const PUBLIC_SIZE: u32 = 128;

pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(bytes))
}

fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Server user ids are UUIDs; anything else must not reach a path.
fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

impl FriendsClient {
    async fn avatar_objects(&self, me: &str) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Obj {
            name: String,
        }
        let objs: Vec<Obj> = self
            .json(
                Method::POST,
                &format!("/storage/v1/object/list/{BUCKET}"),
                Some(&json!({ "prefix": me, "limit": 100, "offset": 0 })),
                &[],
            )
            .await?;
        Ok(objs
            .into_iter()
            .map(|o| format!("{me}/{}", o.name))
            .collect())
    }

    async fn delete_avatar_objects(&self, paths: Vec<String>) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        self.send(
            Method::DELETE,
            &format!("/storage/v1/object/{BUCKET}"),
            Some(&json!({ "prefixes": paths })),
            &[],
        )
        .await
        .map(drop)
    }

    /// Makes the server photo match `png` (`None` = no photo). `current` is
    /// the SHA-1 the profile row has now; nothing is sent when it matches.
    pub(crate) async fn sync_avatar(
        &self,
        current: Option<&str>,
        png: Option<&[u8]>,
    ) -> Result<()> {
        let wanted = png.map(sha1_hex);
        if current == wanted.as_deref() {
            return Ok(());
        }
        let me = self.user_id().await?;
        let keep = wanted.as_ref().map(|sha| format!("{me}/{sha}.png"));
        if let (Some(bytes), Some(path)) = (png, &keep) {
            if bytes.len() > MAX_BYTES {
                return Err(CoreError::AvatarInvalid("photo is too large".into()));
            }
            self.send_raw(
                Method::POST,
                &format!("/storage/v1/object/{BUCKET}/{path}"),
                bytes.to_vec(),
                "image/png",
                &[("x-upsert", "true")],
            )
            .await?;
        }
        self.rpc_void("set_avatar", json!({ "sha1": wanted }))
            .await?;
        // Older photos are no longer referenced.
        let stale = self
            .avatar_objects(&me)
            .await?
            .into_iter()
            .filter(|p| Some(p) != keep.as_ref())
            .collect();
        self.delete_avatar_objects(stale).await
    }

    /// Removes every uploaded photo of the caller (before `delete_me`).
    pub(crate) async fn delete_all_avatars(&self) -> Result<()> {
        let me = self.user_id().await?;
        let all = self.avatar_objects(&me).await?;
        self.delete_avatar_objects(all).await
    }

    /// A friend's photo as a `data:` URI, from the cache or the server.
    /// Failures just mean "no photo" (the UI falls back to an icon).
    pub async fn friend_avatar(&self, uid: &str, sha1: &str) -> Option<String> {
        if !is_uuid(uid) || !is_sha1(sha1) {
            return None;
        }
        let sha1 = sha1.to_ascii_lowercase();
        let cached = self
            .ctx()
            .paths
            .cache()
            .join("avatars")
            .join(format!("{uid}-{sha1}.png"));
        if let Ok(bytes) = std::fs::read(&cached) {
            return Some(data_uri(&bytes));
        }
        let bytes = self
            .send(
                Method::GET,
                &format!("/storage/v1/object/authenticated/{BUCKET}/{uid}/{sha1}.png"),
                None,
                &[],
            )
            .await
            .map_err(|e| tracing::debug!(error = %e.detail(), "friend photo unavailable"))
            .ok()?;
        // Same bytes the owner uploaded, and a real PNG of sane size.
        if bytes.len() > MAX_BYTES
            || sha1_hex(&bytes) != sha1
            || decode_limited(&bytes, MAX_BYTES, 1024).is_err()
        {
            return None;
        }
        if let Err(e) = crate::fsutil::write_atomic(&cached, &bytes) {
            tracing::debug!(error = %e.detail(), "could not cache a friend photo");
        }
        Some(data_uri(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{FRIEND, ME, signed_in};
    use super::sha1_hex;
    use crate::skin::image::tests::png;

    #[tokio::test]
    async fn uploads_a_new_photo_and_drops_old_ones() {
        let server = MockServer::builder().start().await;
        let img = png(16, 16, |_, _| false);
        let sha = sha1_hex(&img);
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/avatars/{ME}/{sha}.png")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/set_avatar"))
            .and(body_json(json!({ "sha1": sha })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/storage/v1/object/list/avatars"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "name": format!("{sha}.png") }, { "name": "old.png" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/storage/v1/object/avatars"))
            .and(body_json(json!({ "prefixes": [format!("{ME}/old.png")] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.sync_avatar(Some("0000000000000000000000000000000000000000"), Some(&img))
            .await
            .unwrap();
        // Unchanged photo: no request at all (the mocks above expect one).
        c.sync_avatar(Some(&sha), Some(&img)).await.unwrap();
    }

    #[tokio::test]
    async fn friend_photos_are_verified_and_cached() {
        let server = MockServer::builder().start().await;
        let img = png(8, 8, |_, _| false);
        let sha = sha1_hex(&img);
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/avatars/{FRIEND}/{sha}.png"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(img.clone()))
            .expect(1)
            .mount(&server)
            .await;
        let bad = "1111111111111111111111111111111111111111";
        Mock::given(method("GET"))
            .and(path(format!(
                "/storage/v1/object/authenticated/avatars/{FRIEND}/{bad}.png"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(img.clone()))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let first = c.friend_avatar(FRIEND, &sha).await.unwrap();
        assert!(first.starts_with("data:image/png;base64,"));
        // Second time from the cache (the mock expects a single request).
        assert_eq!(c.friend_avatar(FRIEND, &sha).await.unwrap(), first);
        // Content that does not match its name is refused.
        assert!(c.friend_avatar(FRIEND, bad).await.is_none());
        assert!(c.friend_avatar("../x", &sha).await.is_none());
    }
}
