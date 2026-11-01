//! Live check of sending mods to another version on the real Modrinth (K99).
//! Ignored by default; run with
//! `cargo test -p launcher-core --test transfer_live -- --ignored --nocapture`.
//!
//! By default a few well-known mods go from Fabric 1.20.1 to Fabric 1.21.4
//! and NeoForge 1.21.1. To try a real folder (it is copied, never changed):
//! `MEHBUR_MODS=<mods dir> MEHBUR_FROM=26.2:fabric MEHBUR_TO=1.21.11:fabric`.

use std::path::Path;
use std::sync::Arc;

use launcher_core::content::install::{self, InstallRequest};
use launcher_core::content::modrinth::{self, ProjectType};
use launcher_core::content::transfer::{TransferResult, transfer};
use launcher_core::content::{installed, metadata};
use launcher_core::ctx::Ctx;
use launcher_core::events::{NullSink, Progress, Stage};
use launcher_core::instance::Instance;
use launcher_core::paths::Paths;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn instance(spec: &str) -> Instance {
    let (mc, loader) = spec.split_once(':').expect("mc:loader");
    serde_json::from_value(json!({
        "id": spec.replace([':', '.'], "-"), "name": spec, "mcVersion": mc,
        "loader": { "kind": loader }
    }))
    .unwrap()
}

/// Modrinth's name for an instance's loader.
fn loader_name(spec: &str) -> String {
    spec.split_once(':').unwrap().1.to_ascii_lowercase()
}

fn context(tmp: &Path) -> Ctx {
    let paths = Paths::at(tmp.join("MehburMC"));
    paths.ensure_layout().unwrap();
    Ctx::new(paths, Arc::new(NullSink), 8).unwrap()
}

fn count(dir: &Path, suffix: &str) -> usize {
    std::fs::read_dir(dir).map_or(0, |rd| {
        rd.flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(suffix))
            .count()
    })
}

fn print(label: &str, names: &[String]) {
    println!("{label} ({}): {}", names.len(), names.join(", "));
}

/// Every file Modrinth knows in the target must be made for the target.
async fn check_target(ctx: &Ctx, dir: &Path, spec: &str, result: &TransferResult) {
    let (mc, _) = spec.split_once(':').unwrap();
    let loader = loader_name(spec);
    let hashes = installed::hashes(ctx, dir).unwrap();
    let list: Vec<String> = hashes.iter().map(|h| h.1.clone()).collect();
    let known = modrinth::versions_by_sha1(ctx, &list).await.unwrap();
    for (name, sha1, _) in &hashes {
        let version = known.get(sha1);
        let v = version.unwrap_or_else(|| panic!("{name} is not a Modrinth file"));
        assert!(
            v.game_versions.iter().any(|g| g == mc),
            "{name} is for {:?}, not {mc}",
            v.game_versions
        );
        assert!(
            v.loaders.iter().any(|l| *l == loader
                // Quilt also runs Fabric mods.
                || (loader == "quilt" && l == "fabric")),
            "{name} is for {:?}, not {loader}",
            v.loaders
        );
        assert!(
            metadata::read_light(&dir.join(name)).is_some(),
            "{name} has no mod descriptor"
        );
    }
    assert_eq!(
        hashes.len(),
        result.transferred.len() + result.dependencies.len(),
        "one file for each mod reported"
    );
}

async fn send(ctx: &Ctx, from: &str, source_dir: &Path, to: &str, tmp: &Path) {
    let (source, target) = (instance(from), instance(to));
    let target_dir = tmp.join(to.replace([':', '.'], "-"));
    let mods = target_dir.join("mods");
    let progress = Progress::new(ctx.events.clone(), "live", Stage::Content);
    let cancel = CancellationToken::new();

    let start = std::time::Instant::now();
    let r = transfer(
        ctx,
        &source,
        source_dir,
        &target,
        &target_dir,
        &progress,
        &cancel,
    )
    .await
    .unwrap();
    println!("\n{from} -> {to} in {:?}", start.elapsed());
    print("transferred", &r.transferred);
    print("dependencies", &r.dependencies);
    print("already present", &r.already_present);
    print("not found", &r.not_found);
    print("missing dependencies", &r.missing_dependencies);
    check_target(ctx, &mods, to, &r).await;

    // A second run finds everything in place and changes nothing.
    let files = count(&mods, "");
    let again = transfer(
        ctx,
        &source,
        source_dir,
        &target,
        &target_dir,
        &progress,
        &cancel,
    )
    .await
    .unwrap();
    assert!(again.transferred.is_empty(), "{:?}", again.transferred);
    assert!(again.dependencies.is_empty(), "{:?}", again.dependencies);
    assert_eq!(count(&mods, ""), files);
}

#[tokio::test]
#[ignore = "talks to Modrinth and downloads mods"]
async fn sends_real_mods_to_other_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = context(tmp.path());
    let source_dir = tmp.path().join("source");
    let source_mods = source_dir.join("mods");
    std::fs::create_dir_all(&source_mods).unwrap();

    if let Ok(real) = std::env::var("MEHBUR_MODS") {
        for e in std::fs::read_dir(real).unwrap().flatten() {
            if e.path().is_file() {
                std::fs::copy(e.path(), source_mods.join(e.file_name())).unwrap();
            }
        }
        let from = std::env::var("MEHBUR_FROM").expect("MEHBUR_FROM=mc:loader");
        let to = std::env::var("MEHBUR_TO").expect("MEHBUR_TO=mc:loader");
        println!("source: {} files", count(&source_mods, ""));
        send(&ctx, &from, &source_dir, &to, tmp.path()).await;
        return;
    }

    let from = "1.20.1:fabric";
    let roots: Vec<InstallRequest> = ["sodium", "iris", "lithium", "modmenu", "cloth-config"]
        .map(|project| InstallRequest {
            project: project.into(),
            project_type: ProjectType::Mod,
            version_id: None,
        })
        .to_vec();
    let installed = install::install(
        &ctx,
        &instance(from),
        &source_dir,
        &roots,
        &install::quiet_progress(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    println!("source mods: {:?}", installed.installed);
    // A disabled mod has to arrive disabled.
    let lithium = std::fs::read_dir(&source_mods)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("lithium"))
        .unwrap();
    std::fs::rename(
        lithium.path(),
        source_mods.join(format!(
            "{}.disabled",
            lithium.file_name().to_string_lossy()
        )),
    )
    .unwrap();

    for to in ["1.21.4:fabric", "1.21.1:neoForge"] {
        send(&ctx, from, &source_dir, to, tmp.path()).await;
    }
    let target = tmp.path().join("1-21-4-fabric").join("mods");
    assert_eq!(count(&target, ".disabled"), 1, "lithium stays disabled");
}
