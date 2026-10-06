//! End-to-end check of shared skins/capes (phase 17) against the real
//! Supabase project, after `supabase/phase17.sql` was run. Creates three
//! throwaway identities (A and B friends, C a stranger) and deletes them.
//! `cargo test -p launcher-core --test textures_live -- --ignored`

use std::path::Path;
use std::sync::Arc;

use launcher_core::events::NullSink;
use launcher_core::friends::FriendsClient;
use launcher_core::friends::textures::{ReportReason, SharedTexture, Visibility};
use launcher_core::skin::{SkinModel, TextureKind};
use launcher_core::{Ctx, Paths};

fn client(root: &Path) -> FriendsClient {
    let paths = Paths::at(root.join("MehburMC"));
    paths.ensure_layout().unwrap();
    FriendsClient::new(Ctx::new(paths, Arc::new(NullSink), 4).unwrap())
}

/// A unique, valid PNG of `w`×`h` (the seed changes one pixel).
fn png(w: u32, h: u32, seed: u64) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        let mut px = vec![0u8; (w * h * 4) as usize];
        px[..8].copy_from_slice(&seed.to_le_bytes());
        px[3] = 255;
        wr.write_image_data(&px).unwrap();
    }
    out
}

fn code(e: &launcher_core::CoreError) -> String {
    e.code().to_owned()
}

fn ids(list: &[SharedTexture]) -> Vec<i64> {
    list.iter().map(|s| s.id).collect()
}

#[tokio::test]
#[ignore = "talks to the live friends server"]
async fn sharing_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        client(&tmp.path().join("a")),
        client(&tmp.path().join("b")),
        client(&tmp.path().join("c")),
    );
    let pa = a.enable("LiveTexA", None).await.expect("A enable");
    let pb = b.enable("LiveTexB", None).await.expect("B enable");
    c.enable("LiveTexC", None).await.expect("C enable");

    let result = std::panic::AssertUnwindSafe(scenario(&a, &b, &c, &pa.id, &pb.friend_code));
    let outcome = futures_util::FutureExt::catch_unwind(result).await;

    let mut failed = Vec::new();
    for (n, cl) in [("A", &a), ("B", &b), ("C", &c)] {
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

async fn scenario(
    a: &FriendsClient,
    b: &FriendsClient,
    c: &FriendsClient,
    a_id: &str,
    b_code: &str,
) {
    // A and B become friends.
    a.send_request(b_code).await.unwrap();
    let req = b
        .friends()
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.id == a_id)
        .unwrap();
    b.respond(req.request_id, true).await.unwrap();

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let author = format!("Lt{:x}", seed & 0xff_ffff);
    a.claim_name(&author).await.unwrap();

    // The author must be one of the sharer's reserved names.
    let skin = png(64, 64, seed);
    assert_eq!(
        code(
            &a.share_texture(
                TextureKind::Skin,
                SkinModel::Classic,
                "x",
                "NotMine_123",
                &skin,
                Visibility::Public
            )
            .await
            .unwrap_err()
        ),
        "textures.authorInvalid"
    );

    let public = a
        .share_texture(
            TextureKind::Skin,
            SkinModel::Slim,
            "Live skin",
            &author,
            &skin,
            Visibility::Public,
        )
        .await
        .unwrap();
    let cape = png(64, 32, seed + 1);
    let private = a
        .share_texture(
            TextureKind::Cape,
            SkinModel::Classic,
            "Live cape",
            &author,
            &cape,
            Visibility::Friends,
        )
        .await
        .unwrap();
    // Sharing the same image again updates the row, no duplicate.
    let again = a
        .share_texture(
            TextureKind::Skin,
            SkinModel::Slim,
            "Live skin 2",
            &author,
            &skin,
            Visibility::Public,
        )
        .await
        .unwrap();
    assert_eq!(again, public);

    let mine = a.community_textures().await.unwrap();
    let own: Vec<_> = mine.iter().filter(|s| s.mine).collect();
    assert_eq!(own.len(), 2, "{own:?}");
    assert_eq!(
        own.iter().find(|s| s.id == public).unwrap().name,
        "Live skin 2"
    );

    // Friend B sees both, stranger C only the public one; images verified.
    let seen_b = b.community_textures().await.unwrap();
    assert!(ids(&seen_b).contains(&public) && ids(&seen_b).contains(&private));
    let fb = seen_b.iter().find(|s| s.id == private).unwrap();
    assert_eq!(
        (fb.kind, fb.visibility, fb.mine),
        (TextureKind::Cape, Visibility::Friends, false)
    );
    assert_eq!(fb.author, author);
    let seen_c = c.community_textures().await.unwrap();
    assert!(ids(&seen_c).contains(&public));
    assert!(!ids(&seen_c).contains(&private));

    // Reports: not your own; reported shares disappear for the reporter.
    assert_eq!(
        code(
            &a.report_texture(public, ReportReason::Other, "")
                .await
                .unwrap_err()
        ),
        "textures.ownReport"
    );
    assert_eq!(
        code(
            &c.report_texture(private, ReportReason::Spam, "")
                .await
                .unwrap_err()
        ),
        "textures.notFound"
    );
    c.report_texture(public, ReportReason::Stolen, "live test")
        .await
        .unwrap();
    assert!(!ids(&c.community_textures().await.unwrap()).contains(&public));
    assert!(ids(&b.community_textures().await.unwrap()).contains(&public));

    // Withdraw: gone for everyone; only the owner can withdraw.
    assert_eq!(
        code(&b.unshare_texture(public).await.unwrap_err()),
        "textures.notFound"
    );
    a.unshare_texture(public).await.unwrap();
    assert!(!ids(&b.community_textures().await.unwrap()).contains(&public));
    assert_eq!(
        code(&a.unshare_texture(public).await.unwrap_err()),
        "textures.notFound"
    );
    println!("sharing scenario passed");
}
