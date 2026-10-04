//! Modrinth content install against a mock API: dependency resolution,
//! hash verification, "already present" detection and loader filtering.

use std::sync::Arc;

use launcher_core::content::install::{self, InstallRequest};
use launcher_core::content::modrinth::ProjectType;
use launcher_core::events::NullSink;
use launcher_core::instance::{Instance, LoaderKind, LoaderSpec};
use launcher_core::net::{Allowlist, Http};
use launcher_core::{Ctx, Paths};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha1(b: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(b))
}
fn sha512(b: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha512::digest(b))
}

fn version(uri: &str, id: &str, project: &str, file: &str, body: &[u8], deps: Value) -> Value {
    json!({
        "id": id, "project_id": project, "version_number": "1.0", "version_type": "release",
        "game_versions": ["1.20.1"], "loaders": ["fabric"],
        "files": [{"url": format!("{uri}/dl/{file}"), "filename": file, "primary": true,
                   "hashes": {"sha1": sha1(body), "sha512": sha512(body)}, "size": body.len()}],
        "dependencies": deps
    })
}

async fn json_route(server: &MockServer, p: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn fabric_instance() -> Instance {
    serde_json::from_value(json!({
        "name": "t", "mcVersion": "1.20.1",
        "loader": LoaderSpec { kind: LoaderKind::Fabric, version: None }
    }))
    .unwrap()
}

async fn setup() -> (MockServer, tempfile::TempDir, Ctx, std::path::PathBuf) {
    let server = MockServer::builder().start().await;
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::at(tmp.path().join("MehburMC"));
    paths.ensure_layout().unwrap();
    let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
    ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
    ctx.endpoints.modrinth = server.uri();
    let game = tmp.path().join("game");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    // Nothing installed yet: hash lookup finds nothing.
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    (server, tmp, ctx, game)
}

#[tokio::test]
async fn installs_a_mod_with_its_required_dependency() {
    let (server, _tmp, ctx, game) = setup().await;
    let uri = server.uri();
    let (extra, api) = (b"sodium extra jar".as_slice(), b"fabric api jar".as_slice());

    // Root: newest compatible version of sodium-extra, filtered by loader + MC.
    Mock::given(method("GET"))
        .and(path("/v2/project/sodium-extra/version"))
        .and(query_param("loaders", r#"["fabric"]"#))
        .and(query_param("game_versions", r#"["1.20.1"]"#))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &uri,
            "v-se",
            "SE",
            "sodium-extra-1.0.jar",
            extra,
            json!([{"project_id": "FA", "dependency_type": "required"},
                   {"project_id": "XX", "dependency_type": "optional"}])
        )])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/FA/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &uri,
            "v-fa",
            "FA",
            "fabric-api-0.92.jar",
            api,
            json!([])
        )])))
        .mount(&server)
        .await;
    json_route(
        &server,
        "/v2/project/SE",
        json!({"id": "SE", "slug": "sodium-extra", "title": "Sodium Extra"}),
    )
    .await;
    json_route(
        &server,
        "/v2/project/FA",
        json!({"id": "FA", "slug": "fabric-api", "title": "Fabric API"}),
    )
    .await;
    for (f, b) in [
        ("sodium-extra-1.0.jar", extra),
        ("fabric-api-0.92.jar", api),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/dl/{f}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b.to_vec()))
            .expect(1)
            .mount(&server)
            .await;
    }

    let req = [InstallRequest {
        project: "sodium-extra".into(),
        project_type: ProjectType::Mod,
        version_id: None,
    }];
    let r = install::install(
        &ctx,
        &fabric_instance(),
        &game,
        &req,
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let mut names = r.installed.clone();
    names.sort();
    assert_eq!(names, ["fabric-api-0.92.jar", "sodium-extra-1.0.jar"]);
    assert_eq!(
        std::fs::read(game.join("mods/fabric-api-0.92.jar")).unwrap(),
        api
    );
    assert_eq!(
        std::fs::read(game.join("mods/sodium-extra-1.0.jar")).unwrap(),
        extra
    );

    // Installing again: both projects are recognised by file name and skipped.
    let again = install::install(
        &ctx,
        &fabric_instance(),
        &game,
        &req,
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(again.installed.is_empty());
    assert_eq!(again.already_present, ["Sodium Extra"]);
}

#[tokio::test]
async fn reports_unavailable_and_rejects_vanilla_mods() {
    let (server, _tmp, ctx, game) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/project/nothing/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let req = |p: &str| {
        [InstallRequest {
            project: p.into(),
            project_type: ProjectType::Mod,
            version_id: None,
        }]
    };
    let e = install::install(
        &ctx,
        &fabric_instance(),
        &game,
        &req("nothing"),
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code(), "loader.addonUnavailable");

    let mut vanilla = fabric_instance();
    vanilla.loader.kind = LoaderKind::Vanilla;
    let e = install::install(
        &ctx,
        &vanilla,
        &game,
        &req("sodium"),
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code(), "loader.addonUnavailable");
}

#[tokio::test]
async fn corrupted_downloads_are_rejected() {
    let (server, _tmp, ctx, game) = setup().await;
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/v2/project/bad/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([version(
            &uri,
            "v-bad",
            "BD",
            "bad.jar",
            b"expected",
            json!([])
        )])))
        .mount(&server)
        .await;
    json_route(
        &server,
        "/v2/project/BD",
        json!({"id": "BD", "slug": "bad", "title": "Bad"}),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/dl/bad.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tampered".to_vec()))
        .mount(&server)
        .await;
    let req = [InstallRequest {
        project: "bad".into(),
        project_type: ProjectType::Mod,
        version_id: None,
    }];
    let e = install::install(
        &ctx,
        &fabric_instance(),
        &game,
        &req,
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code(), "download.hashMismatch");
    assert!(
        !game.join("mods/bad.jar").exists(),
        "no unverified file is left behind"
    );
}
