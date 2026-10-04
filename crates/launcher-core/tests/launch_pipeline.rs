//! End-to-end test of the "PLAY" preparation pipeline against a mock Mojang:
//! manifest → version JSON → client jar + libraries + assets → command line.
//! No real network, no real Java (an executable named `java(.exe)` is faked).

use std::path::PathBuf;
use std::sync::Arc;

use launcher_core::auth::LaunchAccount;
use launcher_core::events::NullSink;
use launcher_core::launch::{self, LaunchOptions};
use launcher_core::net::download::Verify;
use launcher_core::net::{Allowlist, Http};
use launcher_core::{Ctx, Paths};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha1(b: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(b))
}

struct Fixture {
    _tmp: tempfile::TempDir,
    ctx: Ctx,
    opts: LaunchOptions,
}

const CLIENT: &[u8] = b"fake client jar";
const LIB: &[u8] = b"fake library jar";
const SOUND: &[u8] = b"fake sound";

/// Serves a tiny but complete modern version `t1` (each file at most `n` times).
async fn serve(server: &MockServer, n: u64) {
    let uri = server.uri();
    let asset_index =
        json!({"objects": {"minecraft/sounds/a.ogg": {"hash": sha1(SOUND), "size": SOUND.len()}}})
            .to_string();
    let version = json!({
        "id": "t1", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "assets": "t1",
        "assetIndex": {"id": "t1", "url": format!("{uri}/indexes/t1.json"), "sha1": sha1(asset_index.as_bytes()),
                       "size": asset_index.len(), "totalSize": SOUND.len()},
        "downloads": {"client": {"url": format!("{uri}/client.jar"), "sha1": sha1(CLIENT), "size": CLIENT.len()}},
        "libraries": [{"name": "com.example:lib:1.0", "downloads": {"artifact": {
            "path": "com/example/lib/1.0/lib-1.0.jar", "url": format!("{uri}/lib.jar"),
            "sha1": sha1(LIB), "size": LIB.len()}}}],
        "arguments": {
            "game": ["--username", "${auth_player_name}", "--version", "${version_name}",
                     "--gameDir", "${game_directory}", "--assetsDir", "${assets_root}",
                     "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
                     "--accessToken", "${auth_access_token}"],
            "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
        }
    })
    .to_string();
    let manifest = json!({
        "latest": {"release": "t1", "snapshot": "t1"},
        "versions": [{"id": "t1", "type": "release", "url": format!("{uri}/t1.json"),
                      "releaseTime": "2026-01-01T00:00:00+00:00", "sha1": sha1(version.as_bytes())}]
    });

    let h = sha1(SOUND);
    let routes: Vec<(String, Vec<u8>)> = vec![
        ("/manifest.json".into(), manifest.to_string().into_bytes()),
        ("/t1.json".into(), version.into_bytes()),
        ("/indexes/t1.json".into(), asset_index.into_bytes()),
        ("/client.jar".into(), CLIENT.to_vec()),
        ("/lib.jar".into(), LIB.to_vec()),
        (format!("/objects/{}/{h}", &h[..2]), SOUND.to_vec()),
    ];
    for (p, body) in routes {
        let m = Mock::given(method("GET"))
            .and(path(p.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body));
        // The manifest is cached on disk, so it is fetched at most once too.
        m.up_to_n_times(n).expect(1..=n).mount(server).await;
    }
}

fn fixture(server_uri: &str) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::at(tmp.path().join("MehburMC"));
    paths.ensure_layout().unwrap();
    let mut ctx = Ctx::new(paths.clone(), Arc::new(NullSink), 4).unwrap();
    ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
    ctx.endpoints.version_manifest = format!("{server_uri}/manifest.json");
    ctx.endpoints.resources = format!("{server_uri}/objects");

    // A stand-in Java: only its name and existence are checked here.
    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    let java: PathBuf = tmp.path().join("jdk").join("bin").join(java_name);
    std::fs::create_dir_all(java.parent().unwrap()).unwrap();
    std::fs::write(&java, b"").unwrap();

    let game_dir = paths.instances().join("test");
    std::fs::create_dir_all(&game_dir).unwrap();
    let opts = LaunchOptions {
        version_id: "t1".into(),
        game_dir,
        account: LaunchAccount::offline("Steve").unwrap(),
        max_memory_mb: 2048,
        min_memory_mb: None,
        java_path: Some(java),
        auto_download_java: false,
        extra_jvm_args: vec!["-XX:+UseG1GC".into()],
        extra_game_args: vec![],
        resolution: Some((1280, 720)),
        verify: Verify::Quick,
    };
    Fixture {
        _tmp: tmp,
        ctx,
        opts,
    }
}

/// Everything the JVM will see: args plus the contents of any `@argfile`.
fn full_command(p: &launch::PreparedLaunch) -> String {
    let mut out = p.args.join(" ");
    for a in &p.args {
        if let Some(file) = a.strip_prefix('@') {
            out.push(' ');
            out.push_str(&std::fs::read_to_string(file).unwrap_or_default());
        }
    }
    out
}

#[tokio::test]
async fn prepares_a_complete_launch_and_reuses_files() {
    let server = MockServer::start().await;
    serve(&server, 1).await;
    let f = fixture(&server.uri());
    let cancel = CancellationToken::new();

    let p = launch::prepare(&f.ctx, &f.opts, "t", &cancel)
        .await
        .unwrap();
    let paths = &f.ctx.paths;
    assert_eq!(p.program, f.opts.java_path.clone().unwrap());
    assert_eq!(p.cwd, f.opts.game_dir);
    assert!(paths.versions().join("t1/t1.jar").is_file());
    assert!(
        paths
            .libraries()
            .join("com/example/lib/1.0/lib-1.0.jar")
            .is_file()
    );
    let h = sha1(SOUND);
    assert_eq!(
        std::fs::read(paths.assets().join("objects").join(&h[..2]).join(&h)).unwrap(),
        SOUND
    );

    let cmd = full_command(&p);
    for needle in [
        "net.minecraft.client.main.Main",
        "--username Steve",
        "--version t1",
        "--assetIndex t1",
        "-Xmx2048M",
        "-XX:+UseG1GC",
        "lib-1.0.jar",
        "t1.jar",
    ] {
        assert!(cmd.contains(needle), "missing {needle:?} in {cmd}");
    }
    // The access token never shows up in the loggable command.
    assert!(!p.masked_command.contains("--accessToken 0 "));

    // Second start: everything is cached and only checked (Quick verify);
    // the mocks allow each file once, so any re-download would fail.
    let again = launch::prepare(&f.ctx, &f.opts, "t", &cancel)
        .await
        .unwrap();
    assert_eq!(again.program, p.program);
    for t in p.temp_files.iter().chain(&again.temp_files) {
        let _ = std::fs::remove_file(t);
    }
}

#[tokio::test]
async fn works_offline_after_the_first_start() {
    let server = MockServer::start().await;
    serve(&server, 1).await;
    let f = fixture(&server.uri());
    let cancel = CancellationToken::new();
    launch::prepare(&f.ctx, &f.opts, "t", &cancel)
        .await
        .unwrap();

    // Network gone: the cached manifest and local files are enough (K20).
    drop(server);
    let mut offline = f.ctx.clone();
    offline.endpoints.version_manifest = "http://127.0.0.1:9/manifest.json".into();
    let p = launch::prepare(&offline, &f.opts, "t", &cancel)
        .await
        .unwrap();
    assert!(full_command(&p).contains("--username Steve"));

    let mut unknown = f.opts.clone();
    unknown.version_id = "does-not-exist".into();
    let e = launch::prepare(&offline, &unknown, "t", &cancel)
        .await
        .unwrap_err();
    assert_eq!(e.code(), "version.notFound");
}

#[tokio::test]
async fn rejects_a_non_java_override() {
    let server = MockServer::start().await;
    serve(&server, 1).await;
    let mut f = fixture(&server.uri());
    let evil = f
        .opts
        .game_dir
        .join(if cfg!(windows) { "cmd.exe" } else { "sh" });
    std::fs::write(&evil, b"").unwrap();
    f.opts.java_path = Some(evil);
    let e = launch::prepare(&f.ctx, &f.opts, "t", &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(e.code(), "instance.invalid");
}
