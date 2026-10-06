//! End-to-end check of the MehburMC Library (phase 19) against the real
//! Supabase project, after `supabase/phase19.sql` was run. Creates two
//! throwaway identities and deletes them. Approval needs an admin (phase
//! 20), so uploads stay pending here.
//! `cargo test -p launcher-core --test library_live -- --ignored`

use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use launcher_core::events::NullSink;
use launcher_core::friends::FriendsClient;
use launcher_core::friends::library::{LibraryInstall, LibraryReportReason, LibraryStatus};
use launcher_core::instance::LoaderKind;
use launcher_core::{Ctx, Paths};

fn client(root: &Path) -> FriendsClient {
    let paths = Paths::at(root.join("MehburMC"));
    paths.ensure_layout().unwrap();
    FriendsClient::new(Ctx::new(paths, Arc::new(NullSink), 4).unwrap())
}

/// A small Fabric mod jar, unique per `seed`.
fn mod_jar(seed: u64) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("fabric.mod.json", opts).unwrap();
        write!(
            z,
            r#"{{"schemaVersion":1,"id":"livetest","name":"Live Test Mod","version":"1.0.{seed}","depends":{{"minecraft":">=1.21"}}}}"#
        )
        .unwrap();
        z.start_file("assets/livetest/readme.txt", opts).unwrap();
        z.write_all(b"MehburMC library live test").unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn code(e: &launcher_core::CoreError) -> String {
    e.code().to_owned()
}

#[tokio::test]
#[ignore = "talks to the live friends server"]
async fn library_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (client(&tmp.path().join("a")), client(&tmp.path().join("b")));
    a.enable("LiveLibA", None).await.expect("A enable");
    b.enable("LiveLibB", None).await.expect("B enable");

    let result = std::panic::AssertUnwindSafe(scenario(&a, &b, tmp.path()));
    let outcome = futures_util::FutureExt::catch_unwind(result).await;

    let mut failed = Vec::new();
    for (n, cl) in [("A", &a), ("B", &b)] {
        if let Err(e) = cl.delete_identity().await {
            failed.push(format!("{n} delete: {e}"));
        }
    }
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
    assert!(failed.is_empty(), "{failed:?}");
    println!("cleanup done");
}

async fn scenario(a: &FriendsClient, b: &FriendsClient, root: &Path) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let author = format!("Ll{:x}", seed & 0xff_ffff);
    a.claim_name(&author).await.unwrap();
    let jar = mod_jar(seed);

    // The author must be one of the uploader's reserved names.
    assert_eq!(
        code(
            &a.submit_library_mod(&jar, "live.jar", "Live", "", "NotMine_123")
                .await
                .unwrap_err()
        ),
        "textures.authorInvalid"
    );

    let (id, report) = a
        .submit_library_mod(&jar, "live test.jar", "Live mod", "first", &author)
        .await
        .unwrap();
    assert_eq!(report.descriptor.unwrap().mod_id, "livetest");
    // Same file again updates the row instead of adding one.
    let (again, _) = a
        .submit_library_mod(&jar, "live test.jar", "Live mod 2", "second", &author)
        .await
        .unwrap();
    assert_eq!(again, id);

    let mine = a.library_mods().await.unwrap();
    let m = mine.iter().find(|m| m.id == id).expect("own upload listed");
    assert!(m.mine);
    assert_eq!(m.status, LibraryStatus::Pending);
    assert_eq!(
        (m.name.as_str(), m.description.as_str()),
        ("Live mod 2", "second")
    );
    assert_eq!(m.author, author);
    assert_eq!(m.file_name, "live_test.jar");

    // Pending uploads are invisible to everyone else.
    assert!(!b.library_mods().await.unwrap().iter().any(|m| m.id == id));
    let b_mods = root.join("b-inst/mods");
    assert_eq!(
        code(
            &b.install_library_mod(id, &b_mods, LoaderKind::Fabric, true)
                .await
                .unwrap_err()
        ),
        "library.notFound"
    );
    assert_eq!(
        code(
            &b.report_library_mod(id, LibraryReportReason::Malware, "")
                .await
                .unwrap_err()
        ),
        "library.notFound"
    );

    // The owner can install their own pending upload: download from
    // storage, SHA-1 check and a fresh scan.
    let a_mods = root.join("a-inst/mods");
    match a
        .install_library_mod(id, &a_mods, LoaderKind::Fabric, false)
        .await
        .unwrap()
    {
        LibraryInstall::Installed { file_name } => assert_eq!(file_name, "live_test.jar"),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(std::fs::read(a_mods.join("live_test.jar")).unwrap(), jar);

    // Withdraw: only the owner; afterwards it is gone.
    assert_eq!(
        code(&b.withdraw_library_mod(id).await.unwrap_err()),
        "library.notFound"
    );
    a.withdraw_library_mod(id).await.unwrap();
    assert!(!a.library_mods().await.unwrap().iter().any(|m| m.id == id));
    println!("library scenario passed");
}
