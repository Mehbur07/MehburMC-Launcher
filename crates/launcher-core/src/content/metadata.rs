//! Name, version and icon read from inside a mod jar, for mods Modrinth
//! does not know (and offline): `fabric.mod.json`, `quilt.mod.json`,
//! `META-INF/neoforge.mods.toml` / `META-INF/mods.toml` and the legacy
//! Forge `mcmod.info`.

use std::io::Read;
use std::path::Path;

use serde_json::Value;

use crate::auth::avatar::data_uri;
use crate::skin::image::decode_limited;

/// Descriptor files are tiny; anything bigger is not read.
const MAX_TEXT: u64 = 512 * 1024;
const MAX_ICON: u64 = 256 * 1024;
const MAX_ICON_SIDE: u32 = 512;
const MAX_FIELD_CHARS: usize = 80;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalMeta {
    pub name: Option<String>,
    pub version: Option<String>,
    /// `data:image/png;base64,…`
    pub icon: Option<String>,
}

type Zip = zip::ZipArchive<std::fs::File>;

fn read_entry(zip: &mut Zip, name: &str, max: u64) -> Option<Vec<u8>> {
    let name = name.trim_start_matches('/');
    let f = zip.by_name(name).ok()?;
    if f.size() > max {
        return None;
    }
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.take(max).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn read_text(zip: &mut Zip, name: &str) -> Option<String> {
    let b = read_entry(zip, name, MAX_TEXT)?;
    Some(
        String::from_utf8_lossy(&b)
            .trim_start_matches('\u{feff}')
            .to_owned(),
    )
}

/// Single-line, bounded, no formatting codes.
pub(crate) fn clean(s: &str) -> Option<String> {
    let s: String = s
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split('§')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part
            } else {
                part.get(1..).unwrap_or("")
            }
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_FIELD_CHARS)
        .collect();
    (!s.is_empty() && !s.starts_with("${")).then_some(s)
}

/// Parses JSON the way mod loaders do (Gson, lenient): raw line breaks
/// inside strings and `//` / `/* */` comments are accepted.
pub(crate) fn lenient_json(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str(text) {
        return Some(v);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let (mut in_str, mut escaped) = (false, false);
    while let Some(c) = chars.next() {
        if in_str {
            match c {
                _ if escaped => {
                    escaped = false;
                    out.push(c);
                }
                '\\' => {
                    escaped = true;
                    out.push(c);
                }
                '"' => {
                    in_str = false;
                    out.push(c);
                }
                c if c.is_control() => out.push(' '),
                c => out.push(c),
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_str = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    serde_json::from_str(&out).ok()
}

fn icon(zip: &mut Zip, path: &str) -> Option<String> {
    if path.is_empty() || path.contains("..") {
        return None;
    }
    let bytes = read_entry(zip, path, MAX_ICON)?;
    decode_limited(&bytes, MAX_ICON as usize, MAX_ICON_SIDE).ok()?;
    Some(data_uri(&bytes))
}

/// Fabric/Quilt `icon`: a path or `{"16": path, "128": path}` (largest wins).
fn icon_path(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => m
            .iter()
            .max_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0))
            .and_then(|(_, v)| v.as_str().map(str::to_owned)),
        _ => None,
    }
}

fn from_fabric(zip: &mut Zip) -> Option<LocalMeta> {
    let v: Value = lenient_json(&read_text(zip, "fabric.mod.json")?)?;
    let icon_path = v.get("icon").and_then(icon_path);
    Some(LocalMeta {
        name: v["name"].as_str().and_then(clean),
        version: v["version"].as_str().and_then(clean),
        icon: icon_path.and_then(|p| icon(zip, &p)),
    })
}

fn from_quilt(zip: &mut Zip) -> Option<LocalMeta> {
    let v: Value = lenient_json(&read_text(zip, "quilt.mod.json")?)?;
    let ql = &v["quilt_loader"];
    let icon_path = ql["metadata"].get("icon").and_then(icon_path);
    Some(LocalMeta {
        name: ql["metadata"]["name"].as_str().and_then(clean),
        version: ql["version"].as_str().and_then(clean),
        icon: icon_path.and_then(|p| icon(zip, &p)),
    })
}

/// Value of `key = "…"` / `key = '…'` / `key = """…"""` on one line.
pub(crate) fn toml_value(line: &str, key: &str) -> Option<String> {
    let (k, rest) = line.split_once('=')?;
    if k.trim() != key {
        return None;
    }
    let rest = rest.trim();
    for q in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(inner) = rest.strip_prefix(q) {
            return inner.find(q).map(|end| inner[..end].to_owned());
        }
    }
    None
}

/// First `[[mods]]` table of a (Neo)Forge `mods.toml`, without a TOML
/// library: only the three plain string keys are needed.
fn from_mods_toml(zip: &mut Zip) -> Option<LocalMeta> {
    let text = read_text(zip, "META-INF/neoforge.mods.toml")
        .or_else(|| read_text(zip, "META-INF/mods.toml"))?;
    let (mut name, mut version, mut logo, mut global_logo) = (None, None, None, None);
    let mut in_mods = false;
    let mut seen_mods = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            if seen_mods && in_mods {
                break; // only the first mod of the jar
            }
            in_mods = line == "[[mods]]";
            seen_mods |= in_mods;
            continue;
        }
        if in_mods {
            name = name.or_else(|| toml_value(line, "displayName"));
            version = version.or_else(|| toml_value(line, "version"));
            logo = logo.or_else(|| toml_value(line, "logoFile"));
        } else if !seen_mods {
            global_logo = global_logo.or_else(|| toml_value(line, "logoFile"));
        }
    }
    if !seen_mods {
        return None;
    }
    // `${file.jarVersion}` comes from the manifest.
    let version = match version {
        Some(v) if v.contains("${") => read_text(zip, "META-INF/MANIFEST.MF").and_then(|m| {
            m.lines()
                .find_map(|l| l.strip_prefix("Implementation-Version:"))
                .map(|v| v.trim().to_owned())
        }),
        v => v,
    };
    let icon_path = logo.or(global_logo);
    Some(LocalMeta {
        name: name.as_deref().and_then(clean),
        version: version.as_deref().and_then(clean),
        icon: icon_path.and_then(|p| icon(zip, &p)),
    })
}

/// Legacy Forge: a JSON array (or `{"modList": [...]}`).
fn from_mcmod_info(zip: &mut Zip) -> Option<LocalMeta> {
    let v: Value = lenient_json(&read_text(zip, "mcmod.info")?)?;
    let first = match &v {
        Value::Array(a) => a.first()?,
        Value::Object(o) => o.get("modList")?.as_array()?.first()?,
        _ => return None,
    };
    let logo = first["logoFile"].as_str().map(str::to_owned);
    Some(LocalMeta {
        name: first["name"].as_str().and_then(clean),
        version: first["version"].as_str().and_then(clean),
        icon: logo.and_then(|p| icon(zip, &p)),
    })
}

/// Metadata of a mod jar; `None` if it has no known descriptor.
pub fn read(path: &Path) -> Option<LocalMeta> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let meta = from_fabric(&mut zip)
        .or_else(|| from_quilt(&mut zip))
        .or_else(|| from_mods_toml(&mut zip))
        .or_else(|| from_mcmod_info(&mut zip))?;
    (meta != LocalMeta::default()).then_some(meta)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::skin::image::tests::png;

    fn jar(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for (n, b) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
        p
    }

    #[test]
    fn fabric_with_icon_map() {
        let d = tempfile::tempdir().unwrap();
        let icon = png(16, 16, |_, _| false);
        let p = jar(
            d.path(),
            "a.jar",
            &[
                (
                    "fabric.mod.json",
                    // `§a` is a colour code that must not show up.
                    concat!(
                        r#"{"schemaVersion":1,"id":"a","name":"§aSodium Extra","version":"0.6.0","#,
                        r#""icon":{"16":"x.png","128":"assets/a/icon.png"}}"#
                    )
                    .as_bytes(),
                ),
                ("assets/a/icon.png", &icon),
            ],
        );
        let m = read(&p).unwrap();
        assert_eq!(m.name.as_deref(), Some("Sodium Extra"));
        assert_eq!(m.version.as_deref(), Some("0.6.0"));
        assert!(m.icon.unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn lenient_like_the_loaders() {
        let v = lenient_json(
            "{
 // comment
 \"name\": \"A\", /* block */
 \"description\": \"two
lines \\\" q\", \"url\": \"http://x\"
}",
        )
        .unwrap();
        assert_eq!(v["name"], "A");
        assert_eq!(v["description"], "two lines \" q");
        assert_eq!(v["url"], "http://x");
        assert!(lenient_json("{nope").is_none());
    }

    #[test]
    fn quilt() {
        let d = tempfile::tempdir().unwrap();
        let p = jar(
            d.path(),
            "q.jar",
            &[(
                "quilt.mod.json",
                br#"{"quilt_loader":{"id":"q","version":"1.2","metadata":{"name":"Quilty"}}}"#,
            )],
        );
        let m = read(&p).unwrap();
        assert_eq!(
            (m.name.as_deref(), m.version.as_deref()),
            (Some("Quilty"), Some("1.2"))
        );
        assert_eq!(m.icon, None);
    }

    #[test]
    fn mods_toml_with_manifest_version() {
        let d = tempfile::tempdir().unwrap();
        let logo = png(32, 32, |_, _| false);
        let toml = b"modLoader=\"javafml\"\nlogoFile=\"logo.png\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName='Just Enough Items'\ndescription='''\nmulti line\n'''\n[[dependencies.jei]]\nmodId=\"forge\"\n[[mods]]\ndisplayName=\"Second\"\n";
        let p = jar(
            d.path(),
            "jei.jar",
            &[
                ("META-INF/mods.toml", toml),
                (
                    "META-INF/MANIFEST.MF",
                    b"Manifest-Version: 1.0\r\nImplementation-Version: 19.21.0.247\r\n",
                ),
                ("logo.png", &logo),
            ],
        );
        let m = read(&p).unwrap();
        assert_eq!(m.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(m.version.as_deref(), Some("19.21.0.247"));
        assert!(m.icon.is_some());
    }

    #[test]
    fn legacy_mcmod_info_and_garbage() {
        let d = tempfile::tempdir().unwrap();
        let p = jar(
            d.path(),
            "old.jar",
            &[(
                "mcmod.info",
                br#"{"modListVersion":2,"modList":[{"modid":"x","name":"Old Mod","version":"1.7.10-2"}]}"#,
            )],
        );
        let m = read(&p).unwrap();
        assert_eq!(m.name.as_deref(), Some("Old Mod"));

        // No descriptor, broken JSON, a path-escaping icon, not a zip.
        let p = jar(d.path(), "none.jar", &[("a.class", b"\xca\xfe")]);
        assert_eq!(read(&p), None);
        let p = jar(d.path(), "bad.jar", &[("fabric.mod.json", b"{nope")]);
        assert_eq!(read(&p), None);
        let p = jar(
            d.path(),
            "esc.jar",
            &[(
                "fabric.mod.json",
                br#"{"name":"E","version":"1","icon":"../../secret.png"}"#,
            )],
        );
        assert_eq!(read(&p).unwrap().icon, None);
        let p = d.path().join("plain.jar");
        std::fs::write(&p, b"not a zip").unwrap();
        assert_eq!(read(&p), None);
        // Placeholders are not shown as versions.
        let p = jar(
            d.path(),
            "ph.jar",
            &[("fabric.mod.json", br#"{"name":"P","version":"${version}"}"#)],
        );
        assert_eq!(read(&p).unwrap().version, None);
    }
}
