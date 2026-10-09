//! Live check against skinmc.net (K78). Ignored by default; run with
//! `cargo test -p launcher-core --test skinmc_live -- --ignored`.

use std::sync::Arc;

use launcher_core::ctx::Ctx;
use launcher_core::events::NullSink;
use launcher_core::paths::Paths;
use launcher_core::skinmc::{SkinMcSort, browse};

#[tokio::test]
#[ignore = "talks to skinmc.net"]
async fn latest_page_and_a_tag_page() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::at(tmp.path().join("MehburMC"));
    paths.ensure_layout().unwrap();
    let ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();

    let page = browse(&ctx, SkinMcSort::Latest, None, None).await.unwrap();
    assert!(page.skins.len() >= 20, "only {} skins", page.skins.len());
    assert!(page.skins.iter().any(|s| !s.author.is_empty()));
    let next = page.next.expect("a next page");
    let second = browse(&ctx, SkinMcSort::Latest, None, Some(&next))
        .await
        .unwrap();
    assert!(!second.skins.is_empty());
    assert!(
        second
            .skins
            .iter()
            .all(|s| page.skins.iter().all(|p| p.id != s.id))
    );

    let cats = browse(&ctx, SkinMcSort::Latest, Some("cat"), None)
        .await
        .unwrap();
    assert!(!cats.skins.is_empty());
    println!(
        "latest {} (+{}), cat {}, slim {}",
        page.skins.len(),
        second.skins.len(),
        cats.skins.len(),
        page.skins
            .iter()
            .filter(|s| s.model == launcher_core::skin::SkinModel::Slim)
            .count()
    );
}
