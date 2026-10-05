//! End-to-end check of the friends feature against the real Supabase project.
//! Creates three throwaway anonymous identities and deletes them at the end.
//! Not part of the normal run: `cargo test -p launcher-core --test friends_live -- --ignored`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use launcher_core::events::NullSink;
use launcher_core::friends::share::ItemSource;
use launcher_core::friends::{FriendStatus, FriendsClient};
use launcher_core::instance::Instance;
use launcher_core::{Ctx, Paths};
use serde_json::{Value, json};

fn client(root: &Path) -> FriendsClient {
    let paths = Paths::at(root.join("MehburMC"));
    paths.ensure_layout().unwrap();
    FriendsClient::new(Ctx::new(paths, Arc::new(NullSink), 4).unwrap())
}

fn instance() -> Instance {
    serde_json::from_value(json!({
        "id": "live-test", "name": "Live test", "mcVersion": "1.20.1",
        "loader": { "kind": "fabric", "version": null }
    }))
    .unwrap()
}

/// A real Fabric mod jar from Modrinth (Mod Menu for 1.20.1).
async fn modrinth_jar(c: &Ctx) -> (String, Vec<u8>) {
    let url = r#"https://api.modrinth.com/v2/project/modmenu/version?loaders=["fabric"]&game_versions=["1.20.1"]"#;
    let versions: Vec<Value> = c.http.get_json(url).await.unwrap();
    let file = &versions[0]["files"][0];
    let bytes = c
        .http
        .get_bytes(file["url"].as_str().unwrap())
        .await
        .unwrap();
    (file["filename"].as_str().unwrap().to_owned(), bytes)
}

/// A small valid zip that Modrinth does not know.
fn manual_jar() -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        z.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        write!(
            z,
            r#"{{"schemaVersion":1,"id":"mehbur_live_{stamp}","version":"1"}}"#
        )
        .unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn code(e: &launcher_core::CoreError) -> String {
    e.code().to_owned()
}

#[tokio::test]
#[ignore = "talks to the live friends server"]
async fn friends_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        client(&tmp.path().join("a")),
        client(&tmp.path().join("b")),
        client(&tmp.path().join("c")),
    );
    let pa = a.enable("LiveA", None).await.expect("A enable");
    let pb = b.enable("LiveB", None).await.expect("B enable");
    let pc = c.enable("LiveC", None).await.expect("C enable");
    println!(
        "codes: {} {} {}",
        pa.friend_code, pb.friend_code, pc.friend_code
    );
    let result = std::panic::AssertUnwindSafe(scenario(&tmp, &a, &b, &c, &pa, &pb));
    let outcome = futures_util::FutureExt::catch_unwind(result).await;

    // Always clean up the throwaway identities.
    let mut failed = Vec::new();
    for (n, cl) in [("A", &a), ("B", &b), ("C", &c)] {
        if let Err(e) = cl.disable_and_delete().await {
            failed.push(format!("{n} delete: {e}"));
        }
    }
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
    assert!(failed.is_empty(), "{failed:?}");
    // Deleted identity is really gone: a fresh client with A's code cannot be found.
    let d = client(&tmp.path().join("d"));
    d.enable("LiveD", None).await.unwrap();
    let gone = d.send_request(&pa.friend_code).await.unwrap_err();
    d.disable_and_delete().await.unwrap();
    assert_eq!(code(&gone), "friends.codeNotFound");
    println!("cleanup verified");
}

async fn scenario(
    tmp: &tempfile::TempDir,
    a: &FriendsClient,
    b: &FriendsClient,
    c: &FriendsClient,
    pa: &launcher_core::friends::Profile,
    pb: &launcher_core::friends::Profile,
) {
    // Errors for bad codes.
    assert_eq!(
        code(&a.send_request("MEHBUR-0000").await.unwrap_err()),
        "friends.codeNotFound"
    );
    assert_eq!(
        code(&a.send_request(&pa.friend_code).await.unwrap_err()),
        "friends.self"
    );

    // Request → incoming on B → accept.
    assert_eq!(
        a.send_request(&pb.friend_code.to_lowercase())
            .await
            .unwrap(),
        "pending"
    );
    let req = b
        .friends()
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.id == pa.id)
        .unwrap();
    assert!(req.incoming && req.status == FriendStatus::Pending);
    b.respond(req.request_id, true).await.unwrap();
    let fa = a.friends().await.unwrap();
    assert_eq!(fa.len(), 1);
    assert_eq!(fa[0].status, FriendStatus::Accepted);
    assert_eq!(
        code(&a.send_request(&pb.friend_code).await.unwrap_err()),
        "friends.alreadyFriends"
    );
    println!("request/accept ok");

    // Chat both ways, unread counter, incremental fetch.
    let m1 = a.send_message(&pb.id, "selam B").await.unwrap();
    let m2 = b.send_message(&pa.id, "selam A, naber").await.unwrap();
    let unread = b.friends().await.unwrap()[0].unread;
    assert_eq!(unread, 1);
    let all = b.messages(&pa.id, None).await.unwrap();
    assert_eq!(
        all.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![m1.id, m2.id]
    );
    assert_eq!(b.messages(&pa.id, Some(m1.id)).await.unwrap().len(), 1);
    b.mark_read(&pa.id).await.unwrap();
    assert_eq!(b.friends().await.unwrap()[0].unread, 0);
    println!("chat ok");

    // Profile photo: friend B sees it, stranger C cannot fetch it, a new
    // photo replaces the old one, and clearing it hides it again.
    let photo = |shade: u8| {
        let img = launcher_core::skin::image::Rgba {
            width: 16,
            height: 16,
            pixels: [shade, 40, 200, 255].repeat(256),
        };
        launcher_core::skin::image::encode(&img).unwrap()
    };
    let sha = |b: &[u8]| {
        use sha1::Digest;
        hex::encode(sha1::Sha1::digest(b))
    };
    let (p1, p2) = (photo(10), photo(240));
    a.sync_profile("LiveA", Some(&p1)).await.unwrap();
    let seen = b.friends().await.unwrap()[0].avatar.clone();
    assert_eq!(seen, Some(launcher_core::auth::avatar::data_uri(&p1)));
    assert!(c.friend_avatar(&pa.id, &sha(&p1)).await.is_none());
    a.sync_profile("LiveA", Some(&p2)).await.unwrap();
    let seen = b.friends().await.unwrap()[0].avatar.clone();
    assert_eq!(seen, Some(launcher_core::auth::avatar::data_uri(&p2)));
    a.sync_profile("LiveA", None).await.unwrap();
    assert!(b.friends().await.unwrap()[0].avatar.is_none());
    a.sync_profile("LiveA", Some(&p1)).await.unwrap();
    println!("avatar ok");

    // Online status: enable() already sent a heartbeat for A.
    assert!(a.is_online());
    assert!(b.friends().await.unwrap()[0].online);
    a.go_offline().await.unwrap();
    assert!(!a.is_online());
    assert!(!b.friends().await.unwrap()[0].online);
    a.heartbeat().await.unwrap();
    assert!(b.friends().await.unwrap()[0].online);
    println!("presence ok");

    // C is a stranger: cannot message A, cannot see A/B's chat.
    assert!(c.send_message(&pa.id, "spam").await.is_err());
    assert!(c.messages(&pa.id, None).await.unwrap().is_empty());
    assert!(c.messages(&pb.id, None).await.unwrap().is_empty());

    // A shares an instance with one Modrinth mod and one unknown jar.
    let a_inst = tmp.path().join("a-inst");
    let a_mods = a_inst.join("mods");
    std::fs::create_dir_all(&a_mods).unwrap();
    let (mod_name, mod_bytes) = modrinth_jar(&net_ctx(tmp.path())).await;
    std::fs::write(a_mods.join(&mod_name), &mod_bytes).unwrap();
    std::fs::write(a_mods.join("my-private-mod.jar"), manual_jar()).unwrap();
    let list = a.share_instance(&instance(), &a_inst).await.unwrap();
    assert_eq!(list.items.len(), 2);
    for it in &list.items {
        match (&it.source, it.file_name == mod_name) {
            (ItemSource::Modrinth { .. }, true) | (ItemSource::Upload { .. }, false) => {}
            other => panic!("unexpected source for {}: {other:?}", it.file_name),
        }
    }
    println!("share ok: {mod_name} via Modrinth, my-private-mod.jar uploaded");

    // B sees and installs both; a second run reports them as present.
    let lists = b.friend_lists(&pa.id).await.unwrap();
    assert_eq!(lists.len(), 1);
    let b_inst: PathBuf = tmp.path().join("b-inst");
    let names: Vec<String> = list.items.iter().map(|i| i.file_name.clone()).collect();
    let r = b
        .install_from_list(lists[0].id, &b_inst, &names)
        .await
        .unwrap();
    assert_eq!(r.installed.len(), 2, "{r:?}");
    assert_eq!(
        std::fs::read(b_inst.join("mods").join(&mod_name)).unwrap(),
        mod_bytes
    );
    assert!(b_inst.join("mods/my-private-mod.jar").is_file());
    let again = b
        .install_from_list(lists[0].id, &b_inst, &names)
        .await
        .unwrap();
    assert_eq!(again.already_present.len(), 2);
    println!("install ok");

    // C cannot read A's list or download its upload.
    assert!(c.friend_lists(&pa.id).await.unwrap().is_empty());
    let c_inst = tmp.path().join("c-inst");
    assert!(
        c.install_from_list(lists[0].id, &c_inst, &names)
            .await
            .is_err()
    );
    assert!(!c_inst.join("mods/my-private-mod.jar").exists());
    println!("RLS ok: stranger sees nothing");

    // Unshare removes the list for B.
    a.unshare_instance("live-test").await.unwrap();
    assert!(b.friend_lists(&pa.id).await.unwrap().is_empty());

    // Block: B blocks A → A cannot message B and no longer sees B.
    b.block(&pa.id).await.unwrap();
    assert!(a.send_message(&pb.id, "hey").await.is_err());
    assert!(a.friends().await.unwrap().is_empty());
    assert_eq!(b.friends().await.unwrap()[0].status, FriendStatus::Blocked);
    // A's request to B is silently swallowed (block not revealed).
    assert_eq!(a.send_request(&pb.friend_code).await.unwrap(), "pending");
    assert!(a.friends().await.unwrap().is_empty());
    b.remove(&pa.id).await.unwrap();
    assert!(b.friends().await.unwrap().is_empty());
    println!("block ok");
}

/// Plain context for fetching test fixtures from Modrinth.
fn net_ctx(root: &Path) -> Ctx {
    let paths = Paths::at(root.join("net"));
    paths.ensure_layout().unwrap();
    Ctx::new(paths, Arc::new(NullSink), 4).unwrap()
}

/// Scripted peer for manual UI testing: sends a request to the code in
/// `MEHBUR_PEER_CODE`, waits for acceptance, chats, shares two mods, waits
/// for a reply and then deletes itself.
/// `MEHBUR_PEER_CODE=MEHBUR-XXXX cargo test -p launcher-core --test friends_live peer -- --ignored --nocapture`
#[tokio::test]
#[ignore = "manual UI test helper"]
async fn peer() {
    let target = std::env::var("MEHBUR_PEER_CODE").expect("MEHBUR_PEER_CODE");
    let tmp = tempfile::tempdir().unwrap();
    let me = client(&tmp.path().join("peer"));
    // A recognisable photo: lime/teal checkerboard.
    let photo = launcher_core::skin::image::encode(&launcher_core::skin::image::Rgba {
        width: 8,
        height: 8,
        pixels: (0..64)
            .flat_map(|i| {
                if (i / 8 + i % 8) % 2 == 0 {
                    [180, 255, 30, 255]
                } else {
                    [0, 180, 200, 255]
                }
            })
            .collect(),
    })
    .unwrap();
    me.enable("TestArkadas", Some(&photo)).await.unwrap();
    let outcome = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
        println!("request: {}", me.send_request(&target).await.unwrap());
        let wait = |secs: u64| tokio::time::sleep(std::time::Duration::from_secs(secs));
        let friend = loop {
            if let Some(f) = me
                .friends()
                .await
                .unwrap()
                .into_iter()
                .find(|f| f.friend_code == target && f.status == FriendStatus::Accepted)
            {
                break f;
            }
            wait(2).await;
        };
        println!(
            "accepted by {} (their photo visible: {})",
            friend.display_name,
            friend.avatar.is_some()
        );
        let inst = tmp.path().join("peer-inst");
        std::fs::create_dir_all(inst.join("mods")).unwrap();
        let (name, bytes) = modrinth_jar(&net_ctx(tmp.path())).await;
        std::fs::write(inst.join("mods").join(name), bytes).unwrap();
        std::fs::write(inst.join("mods/ozel-mod.jar"), manual_jar()).unwrap();
        me.share_instance(&instance(), &inst).await.unwrap();
        let first = me
            .send_message(&friend.id, "Selam! Bu bir test mesajı 👋")
            .await
            .unwrap();
        println!("shared + messaged; waiting for a reply");
        for _ in 0..150 {
            let msgs = me.messages(&friend.id, Some(first.id)).await.unwrap();
            if let Some(m) = msgs.iter().find(|m| m.sender == friend.id) {
                println!("reply: {}", m.body);
                me.send_message(&friend.id, &format!("Aldım: {}", m.body))
                    .await
                    .unwrap();
                break;
            }
            wait(2).await;
        }
        wait(30).await;
    }))
    .await;
    me.disable_and_delete().await.unwrap();
    println!("peer deleted");
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
}
