//! Copies a premium player's public skin (and cape) into the library:
//! name → UUID (`api.minecraftservices.com`), UUID → textures property
//! (`sessionserver.mojang.com`), then the PNGs from `textures.minecraft.net`.
//! No sign-in is involved; these endpoints are public.

use base64::Engine;
use serde::Deserialize;

use super::SkinModel;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::net::join_url;

pub struct PlayerTextures {
    /// Name with Mojang's capitalisation.
    pub name: String,
    pub skin: Option<(Vec<u8>, SkinModel)>,
    pub cape: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct Lookup {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Profile {
    #[serde(default)]
    properties: Vec<Property>,
}

#[derive(Deserialize)]
struct Property {
    name: String,
    value: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TexturesValue {
    textures: Textures,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "UPPERCASE")]
struct Textures {
    skin: Option<Texture>,
    cape: Option<Texture>,
}

#[derive(Deserialize)]
struct Texture {
    url: String,
    #[serde(default)]
    metadata: Option<TextureMeta>,
}

#[derive(Deserialize)]
struct TextureMeta {
    #[serde(default)]
    model: Option<String>,
}

/// Mojang player names: 1–16 of `[A-Za-z0-9_]` (some legacy names are short).
fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The textures property still advertises `http://`; the host serves HTTPS
/// and the allowlist only permits HTTPS.
fn https(url: &str) -> String {
    match url.strip_prefix("http://textures.minecraft.net/") {
        Some(rest) => format!("https://textures.minecraft.net/{rest}"),
        None => url.to_owned(),
    }
}

pub async fn fetch_player(ctx: &Ctx, name: &str) -> Result<PlayerTextures> {
    let name = name.trim();
    if !valid_name(name) {
        return Err(CoreError::PlayerNotFound(name.to_owned()));
    }
    let lookup_url = join_url(
        &ctx.endpoints.mojang_services,
        &["minecraft", "profile", "lookup", "name", name],
    )?;
    let (status, body) = ctx
        .http
        .request_json(reqwest::Method::GET, &lookup_url, None, &[])
        .await?;
    match status {
        200 => {}
        204 | 404 => return Err(CoreError::PlayerNotFound(name.to_owned())),
        s => {
            return Err(CoreError::HttpStatus {
                url: lookup_url,
                status: s,
            });
        }
    }
    let lookup: Lookup = serde_json::from_slice(&body).map_err(|source| CoreError::Json {
        path: lookup_url.into(),
        source,
    })?;
    if lookup.id.len() != 32 || !lookup.id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CoreError::PlayerNotFound(name.to_owned()));
    }

    let profile_url = join_url(
        &ctx.endpoints.session_server,
        &["session", "minecraft", "profile", &lookup.id],
    )?;
    let profile: Profile = ctx.http.get_json(&profile_url).await?;
    let textures = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .and_then(|p| {
            base64::engine::general_purpose::STANDARD
                .decode(&p.value)
                .ok()
        })
        .and_then(|raw| serde_json::from_slice::<TexturesValue>(&raw).ok())
        .unwrap_or_default()
        .textures;

    let skin = match textures.skin {
        Some(t) => {
            let model = match t.metadata.and_then(|m| m.model).as_deref() {
                Some("slim") => SkinModel::Slim,
                _ => SkinModel::Classic,
            };
            Some((ctx.http.get_bytes(&https(&t.url)).await?, model))
        }
        None => None,
    };
    let cape = match textures.cape {
        Some(t) => Some(ctx.http.get_bytes(&https(&t.url)).await?),
        None => None,
    };
    Ok(PlayerTextures {
        name: lookup.name,
        skin,
        cape,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::NullSink;
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;
    use crate::skin::image::tests::png;

    async fn ctx(server: &MockServer) -> (tempfile::TempDir, Ctx) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.mojang_services = server.uri();
        ctx.endpoints.session_server = server.uri();
        (tmp, ctx)
    }

    #[test]
    fn upgrades_texture_urls() {
        assert_eq!(
            https("http://textures.minecraft.net/texture/abc"),
            "https://textures.minecraft.net/texture/abc"
        );
        assert_eq!(https("http://evil.example/x"), "http://evil.example/x");
    }

    #[tokio::test]
    async fn fetches_skin_and_cape() {
        let server = MockServer::start().await;
        let (_tmp, ctx) = ctx(&server).await;
        let id = "853c80ef3c3749fdaa49938b674adae6";
        let skin = png(64, 64, |_, _| false);
        let cape = png(64, 32, |_, _| false);
        let value = json!({"textures": {
            "SKIN": {"url": format!("{}/t/skin", server.uri()), "metadata": {"model": "slim"}},
            "CAPE": {"url": format!("{}/t/cape", server.uri())}
        }});
        let encoded = base64::engine::general_purpose::STANDARD.encode(value.to_string());
        Mock::given(method("GET"))
            .and(path("/minecraft/profile/lookup/name/JEB_"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": id, "name": "jeb_"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/session/minecraft/profile/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": id, "name": "jeb_",
                "properties": [{"name": "textures", "value": encoded}]
            })))
            .mount(&server)
            .await;
        Mock::given(path("/t/skin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(skin.clone()))
            .mount(&server)
            .await;
        Mock::given(path("/t/cape"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(cape.clone()))
            .mount(&server)
            .await;

        let p = fetch_player(&ctx, " JEB_ ").await.unwrap();
        assert_eq!(p.name, "jeb_");
        assert_eq!(p.skin.unwrap(), (skin, SkinModel::Slim));
        assert_eq!(p.cape.unwrap(), cape);
    }

    #[tokio::test]
    async fn unknown_player() {
        let server = MockServer::start().await;
        let (_tmp, ctx) = ctx(&server).await;
        Mock::given(path("/minecraft/profile/lookup/name/nobody"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(json!({"errorMessage": "Couldn't find"})),
            )
            .mount(&server)
            .await;
        let e = fetch_player(&ctx, "nobody").await.err().unwrap();
        assert_eq!(e.code(), "skin.playerNotFound");
        let e = fetch_player(&ctx, "bad name!").await.err().unwrap();
        assert_eq!(e.code(), "skin.playerNotFound");
    }
}
