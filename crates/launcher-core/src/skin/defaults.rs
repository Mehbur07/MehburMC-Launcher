//! The game's own default skins (Steve, Alex, …), read at runtime from an
//! installed client jar. Nothing Mojang-owned ships with the launcher (K59).

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::{SkinModel, data_uri, image};
use crate::archive;
use crate::paths::Paths;

/// 1.19.3+: `player/wide/steve.png`, `player/slim/alex.png`, …
const PLAYER_DIR: &str = "assets/minecraft/textures/entity/player/";
/// Older jars only have these two.
const LEGACY: &[(&str, SkinModel)] = &[
    (
        "assets/minecraft/textures/entity/steve.png",
        SkinModel::Classic,
    ),
    ("assets/minecraft/textures/entity/alex.png", SkinModel::Slim),
];

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DefaultSkin {
    /// Capitalised file stem, e.g. "Steve".
    pub name: String,
    pub model: SkinModel,
    pub data_uri: String,
    /// Version id of the jar the skins were read from.
    pub source: String,
}

/// Client jars under `versions/`, newest file first.
fn client_jars(paths: &Paths) -> Vec<(String, PathBuf)> {
    let Ok(dirs) = std::fs::read_dir(paths.versions()) else {
        return Vec::new();
    };
    let mut jars: Vec<(std::time::SystemTime, String, PathBuf)> = dirs
        .flatten()
        .filter_map(|d| {
            let id = d.file_name().to_str()?.to_owned();
            let jar = d.path().join(format!("{id}.jar"));
            let modified = jar.metadata().ok()?.modified().ok()?;
            Some((modified, id, jar))
        })
        .collect();
    jars.sort_by_key(|j| std::cmp::Reverse(j.0));
    jars.into_iter().map(|(_, id, jar)| (id, jar)).collect()
}

fn title(stem: &str) -> String {
    let mut c = stem.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn read_skins(jar: &Path, source: &str) -> Vec<DefaultSkin> {
    let Ok(names) = archive::entry_names(jar) else {
        return Vec::new();
    };
    let mut wanted: Vec<(String, SkinModel)> = names
        .iter()
        .filter_map(|n| {
            let rest = n.strip_prefix(PLAYER_DIR)?;
            let (dir, file) = rest.split_once('/')?;
            let model = match dir {
                "wide" => SkinModel::Classic,
                "slim" => SkinModel::Slim,
                _ => return None,
            };
            (file.ends_with(".png") && !file.contains('/')).then(|| (n.clone(), model))
        })
        .collect();
    if wanted.is_empty() {
        wanted = LEGACY
            .iter()
            .filter(|(n, _)| names.iter().any(|x| x == n))
            .map(|(n, m)| ((*n).to_owned(), *m))
            .collect();
    }
    // Alphabetical by character, classic before slim.
    wanted.sort_by_key(|(n, m)| {
        let stem = n.rsplit('/').next().unwrap_or("").to_owned();
        (stem, *m == SkinModel::Slim)
    });
    let entries: Vec<String> = wanted.iter().map(|(n, _)| n.clone()).collect();
    let Ok(contents) = archive::read_entries(jar, &entries) else {
        return Vec::new();
    };
    wanted
        .into_iter()
        .zip(contents)
        .filter_map(|((entry, model), bytes)| {
            let bytes = bytes?;
            // Same checks as an imported file.
            let img = image::decode(&bytes).ok()?;
            image::check_skin(&img).ok()?;
            let stem = entry.rsplit('/').next()?.trim_end_matches(".png");
            Some(DefaultSkin {
                name: title(stem),
                model,
                data_uri: data_uri(&bytes),
                source: source.to_owned(),
            })
        })
        .collect()
}

/// Default skins from the newest installed client jar that has any; empty
/// when no vanilla jar is installed yet.
pub fn list(paths: &Paths) -> Vec<DefaultSkin> {
    client_jars(paths)
        .into_iter()
        .map(|(id, jar)| read_skins(&jar, &id))
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

/// Java `UUID.hashCode()` of a hyphenated UUID.
fn java_uuid_hash(uuid: &str) -> Option<i32> {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let msb = u64::from_str_radix(&hex[..16], 16).ok()?;
    let lsb = u64::from_str_radix(&hex[16..], 16).ok()?;
    let hilo = msb ^ lsb;
    Some(((hilo >> 32) as i32) ^ (hilo as i32))
}

/// Order of `DefaultPlayerSkin.DEFAULT_SKINS` (1.19.3+): slim first, then
/// wide, each alphabetical.
const MODERN_ORDER: &[&str] = &[
    "Alex", "Ari", "Efe", "Kai", "Makena", "Noor", "Steve", "Sunny", "Zuri",
];

/// The skin the game shows for `uuid` when the account has none of its own:
/// one of 18 by `floorMod(uuid.hashCode(), 18)` on 1.19.3+, Steve or Alex by
/// the lowest hash bit before that.
pub fn for_uuid<'a>(defaults: &'a [DefaultSkin], uuid: &str) -> Option<&'a DefaultSkin> {
    let hash = java_uuid_hash(uuid)?;
    let find =
        |name: &str, model: SkinModel| defaults.iter().find(|d| d.name == name && d.model == model);
    let modern: Vec<&DefaultSkin> = [SkinModel::Slim, SkinModel::Classic]
        .into_iter()
        .flat_map(|m| MODERN_ORDER.iter().map(move |n| (*n, m)))
        .filter_map(|(n, m)| find(n, m))
        .collect();
    if modern.len() == MODERN_ORDER.len() * 2 {
        return Some(modern[hash.rem_euclid(modern.len() as i32) as usize]);
    }
    if hash & 1 == 1 {
        find("Alex", SkinModel::Slim)
    } else {
        find("Steve", SkinModel::Classic)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::super::image::tests::png;
    use super::*;

    fn jar(path: &Path, entries: &[(&str, Vec<u8>)]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn skin(name: &str, model: SkinModel) -> DefaultSkin {
        DefaultSkin {
            name: name.into(),
            model,
            data_uri: String::new(),
            source: String::new(),
        }
    }

    #[test]
    fn picks_the_default_skin_like_the_game() {
        // Java: UUID.fromString(..).hashCode()
        assert_eq!(
            java_uuid_hash("00000000-0000-0001-0000-000000000000"),
            Some(1)
        );
        assert_eq!(
            java_uuid_hash("ffffffff-ffff-ffff-0000-000000000000"),
            Some(0)
        );
        assert_eq!(java_uuid_hash("nope"), None);

        let legacy = [
            skin("Steve", SkinModel::Classic),
            skin("Alex", SkinModel::Slim),
        ];
        let even = "00000000-0000-0002-0000-000000000000";
        let odd = "00000000-0000-0001-0000-000000000000";
        assert_eq!(for_uuid(&legacy, even).unwrap().name, "Steve");
        assert_eq!(for_uuid(&legacy, odd).unwrap().name, "Alex");

        let modern: Vec<DefaultSkin> = [SkinModel::Classic, SkinModel::Slim]
            .into_iter()
            .flat_map(|m| MODERN_ORDER.iter().map(move |n| skin(n, m)))
            .collect();
        // hash 1 → index 1 = slim Ari; hash 9 → wide Alex; hash -1 → wide Zuri.
        let pick = |u: &str| {
            let s = for_uuid(&modern, u).unwrap();
            (s.name.clone(), s.model)
        };
        assert_eq!(pick(odd), ("Ari".into(), SkinModel::Slim));
        assert_eq!(
            pick("00000000-0000-0009-0000-000000000000"),
            ("Alex".into(), SkinModel::Classic)
        );
        assert_eq!(
            pick("00000000-ffff-ffff-0000-000000000000"),
            ("Zuri".into(), SkinModel::Classic)
        );
        assert!(for_uuid(&[], odd).is_none());
    }

    #[test]
    fn reads_modern_and_legacy_layouts() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        assert!(list(&paths).is_empty());

        let skin = png(64, 64, |_, _| false);
        let v = paths.versions();
        jar(
            &v.join("1.8.9/1.8.9.jar"),
            &[(LEGACY[0].0, skin.clone()), ("other.txt", b"x".to_vec())],
        );
        let old = list(&paths);
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].name, "Steve");
        assert_eq!(old[0].source, "1.8.9");

        // Make the newer jar strictly newer on coarse-mtime filesystems.
        std::thread::sleep(std::time::Duration::from_millis(20));
        jar(
            &v.join("1.21.4/1.21.4.jar"),
            &[
                (&format!("{PLAYER_DIR}wide/steve.png"), skin.clone()),
                (&format!("{PLAYER_DIR}slim/alex.png"), skin.clone()),
                (&format!("{PLAYER_DIR}slim/broken.png"), b"nope".to_vec()),
                (&format!("{PLAYER_DIR}other/x.png"), skin.clone()),
            ],
        );
        let new = list(&paths);
        let names: Vec<_> = new.iter().map(|s| (s.name.as_str(), s.model)).collect();
        assert_eq!(
            names,
            [("Alex", SkinModel::Slim), ("Steve", SkinModel::Classic)]
        );
        assert!(new[0].data_uri.starts_with("data:image/png;base64,"));
    }
}
