//! The game's own multiplayer list: `<instance>/servers.dat` (uncompressed
//! NBT, `{servers: [{name, ip, icon?, acceptTextures?, hidden?}]}`).
//! Unknown fields and hidden entries are preserved on write.

use std::path::Path;

use super::nbt::{self, Tag};
use super::{GameServer, ServerAddress};
use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

pub const FILE: &str = "servers.dat";
/// The game keeps a backup with this name while saving; we do the same.
const BACKUP: &str = "servers.dat_old";

fn invalid(path: &Path, reason: impl Into<String>) -> CoreError {
    CoreError::ServerListInvalid {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

/// Reads the root compound; a missing file is an empty list.
fn load(path: &Path) -> Result<Tag> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Tag::Compound(vec![(
                "servers".into(),
                Tag::List(10, vec![]),
            )]));
        }
        Err(e) => return Err(CoreError::io(path, e)),
    };
    let (_, root) = nbt::read(&bytes).map_err(|r| invalid(path, r))?;
    match root.get("servers") {
        None | Some(Tag::List(..)) => Ok(root),
        Some(_) => Err(invalid(path, "`servers` is not a list")),
    }
}

fn entries(root: &Tag) -> &[Tag] {
    match root.get("servers") {
        Some(Tag::List(_, items)) => items,
        _ => &[],
    }
}

fn entries_mut(root: &mut Tag) -> &mut Vec<Tag> {
    if !matches!(root.get("servers"), Some(Tag::List(..)))
        && let Tag::Compound(fields) = root
    {
        fields.retain(|(k, _)| k != "servers");
        fields.push(("servers".into(), Tag::List(10, vec![])));
    }
    match root.get_mut("servers") {
        Some(Tag::List(_, items)) => items,
        _ => unreachable!("inserted above"),
    }
}

fn save(path: &Path, root: &Tag) -> Result<()> {
    if path.is_file() {
        let _ = std::fs::copy(path, path.with_file_name(BACKUP));
    }
    write_atomic(path, &nbt::write("", root))
}

/// Visible entries; `index` is the position in the file (hidden entries
/// count), so it can be passed back to [`remove`].
pub fn list(path: &Path) -> Result<Vec<GameServer>> {
    let root = load(path)?;
    Ok(entries(&root)
        .iter()
        .enumerate()
        .filter(|(_, e)| e.get("hidden").and_then(Tag::as_byte).unwrap_or(0) == 0)
        .filter_map(|(i, e)| {
            let address = e.get("ip")?.as_str()?.trim().to_owned();
            if address.is_empty() {
                return None;
            }
            let name = e
                .get("name")
                .and_then(Tag::as_str)
                .unwrap_or_default()
                .to_owned();
            let icon = e
                .get("icon")
                .and_then(Tag::as_str)
                .filter(|s| !s.is_empty() && s.len() <= super::MAX_ICON_B64)
                .map(|s| format!("data:image/png;base64,{s}"));
            Some(GameServer {
                index: i as u32,
                name,
                address,
                icon,
            })
        })
        .collect())
}

/// Appends a server (like "Add Server" in the game). An entry with the same
/// address is renamed instead of duplicated.
pub fn add(path: &Path, name: &str, address: &ServerAddress) -> Result<Vec<GameServer>> {
    let mut root = load(path)?;
    let text = address.to_string();
    let items = entries_mut(&mut root);
    let existing = items.iter_mut().find(|e| {
        e.get("hidden").and_then(Tag::as_byte).unwrap_or(0) == 0
            && e.get("ip")
                .and_then(Tag::as_str)
                .and_then(|s| ServerAddress::parse(s).ok())
                .is_some_and(|a| a == *address)
    });
    match existing {
        Some(Tag::Compound(fields)) => {
            fields.retain(|(k, _)| k != "name");
            fields.insert(0, ("name".into(), Tag::String(name.to_owned())));
        }
        _ => items.push(Tag::Compound(vec![
            ("name".into(), Tag::String(name.to_owned())),
            ("ip".into(), Tag::String(text)),
        ])),
    }
    save(path, &root)?;
    list(path)
}

/// Removes entry `index` if it still has `address` (the file may have been
/// changed by the game in the meantime).
pub fn remove(path: &Path, index: u32, address: &str) -> Result<Vec<GameServer>> {
    let mut root = load(path)?;
    let items = entries_mut(&mut root);
    let i = index as usize;
    let matches = items
        .get(i)
        .and_then(|e| e.get("ip"))
        .and_then(Tag::as_str)
        .is_some_and(|ip| ip.trim() == address.trim());
    if !matches {
        return Err(CoreError::ServerNotFound(address.to_owned()));
    }
    items.remove(i);
    save(path, &root)?;
    list(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> ServerAddress {
        ServerAddress::parse(s).unwrap()
    }

    /// A file shaped like the one the game writes, with an unknown field and
    /// a hidden (quick play) entry.
    fn game_file(dir: &Path) -> std::path::PathBuf {
        let root = Tag::Compound(vec![(
            "servers".into(),
            Tag::List(
                10,
                vec![
                    Tag::Compound(vec![
                        ("ip".into(), Tag::String("mc.hypixel.net".into())),
                        ("name".into(), Tag::String("Hypixel".into())),
                        ("icon".into(), Tag::String("iVBORw0KGgo=".into())),
                        ("acceptTextures".into(), Tag::Byte(1)),
                    ]),
                    Tag::Compound(vec![
                        ("ip".into(), Tag::String("hidden.example".into())),
                        ("name".into(), Tag::String("Hidden".into())),
                        ("hidden".into(), Tag::Byte(1)),
                    ]),
                    Tag::Compound(vec![
                        ("ip".into(), Tag::String("play.example.org:25570".into())),
                        ("name".into(), Tag::String("Örnek".into())),
                    ]),
                ],
            ),
        )]);
        let p = dir.join(FILE);
        std::fs::write(&p, nbt::write("", &root)).unwrap();
        p
    }

    #[test]
    fn missing_file_is_empty_and_add_creates_it() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        assert!(list(&p).unwrap().is_empty());
        let l = add(&p, "Yerel", &addr("localhost")).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].address, "localhost");
        assert_eq!(l[0].name, "Yerel");
        assert!(p.is_file());
    }

    #[test]
    fn lists_visible_entries_with_icons() {
        let d = tempfile::tempdir().unwrap();
        let p = game_file(d.path());
        let l = list(&p).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].name, "Hypixel");
        assert_eq!(
            l[0].icon.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!((l[1].index, l[1].name.as_str()), (2, "Örnek"));
    }

    #[test]
    fn add_and_remove_keep_other_fields() {
        let d = tempfile::tempdir().unwrap();
        let p = game_file(d.path());
        // Same address (default port spelled out) renames, no duplicate.
        let l = add(&p, "Hypixel 2", &addr("MC.hypixel.net:25565")).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].name, "Hypixel 2");
        let l = add(&p, "Yeni", &addr("new.example:1234")).unwrap();
        assert_eq!(l.last().unwrap().address, "new.example:1234");

        // Stale index/address pair is refused.
        assert!(remove(&p, 0, "other.example").is_err());
        let l = remove(&p, 2, "play.example.org:25570").unwrap();
        assert_eq!(
            l.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["Hypixel 2", "Yeni"]
        );

        let (_, root) = nbt::read(&std::fs::read(&p).unwrap()).unwrap();
        let items = entries(&root);
        assert_eq!(items[0].get("acceptTextures"), Some(&Tag::Byte(1)));
        assert_eq!(items[1].get("hidden"), Some(&Tag::Byte(1)));
        assert!(d.path().join(BACKUP).is_file());
    }

    #[test]
    fn corrupt_file_is_reported() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        std::fs::write(&p, b"not nbt").unwrap();
        let e = list(&p).unwrap_err();
        assert_eq!(e.code(), "server.listInvalid");
        // Never overwritten by an add.
        assert!(add(&p, "x", &addr("a.b")).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"not nbt");
    }
}
