//! Adoptium runtime install against a mock API: package lookup, SHA-256
//! verification, extraction (top folder stripped) and `release` parsing.

use std::io::Write;
use std::sync::Arc;

use launcher_core::events::NullSink;
use launcher_core::java::{self, adoptium};
use launcher_core::net::{Allowlist, Http};
use launcher_core::{Ctx, Paths};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sha256(b: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(b))
}

/// A JRE zip shaped like Temurin's: `jdk-21.0.5+11-jre/{release,bin/java}`.
fn jre_zip() -> Vec<u8> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("jdk-21.0.5+11-jre/release", opts).unwrap();
        z.write_all(
            b"IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"21.0.5\"\nOS_ARCH=\"x86_64\"\n",
        )
        .unwrap();
        z.start_file(format!("jdk-21.0.5+11-jre/bin/{exe}"), opts)
            .unwrap();
        z.write_all(b"not really java").unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

async fn setup(archive: Vec<u8>, advertised_sha: String) -> (MockServer, tempfile::TempDir, Ctx) {
    let server = MockServer::builder().start().await;
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path_regex(r"^/v3/assets/latest/21/hotspot$"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"binary": {"package": {
            "link": format!("{uri}/OpenJDK21U-jre.zip"), "checksum": advertised_sha,
            "name": "OpenJDK21U-jre.zip", "size": archive.len()}}}])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/OpenJDK21U-jre.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::at(tmp.path().join("MehburMC"));
    paths.ensure_layout().unwrap();
    let mut ctx = Ctx::new(paths, Arc::new(NullSink), 2).unwrap();
    ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
    ctx.endpoints.adoptium = uri;
    (server, tmp, ctx)
}

#[tokio::test]
async fn installs_and_detects_a_managed_runtime() {
    let zip = jre_zip();
    let sha = sha256(&zip);
    let (_server, _tmp, ctx) = setup(zip, sha).await;

    let j = adoptium::install(&ctx, 21, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(j.major, 21);
    assert_eq!(j.version, "21.0.5");
    assert!(j.managed);
    assert!(java::executable_in(&java::managed_home(&ctx, 21)).is_file());
    // Nothing left in the download/staging area.
    assert!(!ctx.paths.runtime().join(".java21.staging").exists());

    // The managed runtime is now found without network.
    let again = java::resolve(&ctx, 21, None, false, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again.executable, j.executable);
    assert!(java::managed(&ctx).iter().any(|m| m.major == 21));
}

#[tokio::test]
async fn refuses_a_tampered_archive() {
    let (_server, _tmp, ctx) = setup(jre_zip(), "0".repeat(64)).await;
    let e = adoptium::install(&ctx, 21, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(e.code(), "download.hashMismatch");
    assert!(!java::managed_home(&ctx, 21).exists());
}
