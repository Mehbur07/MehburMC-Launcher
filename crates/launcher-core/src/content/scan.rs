//! Automatic check of a mod jar before it is uploaded to (or installed
//! from) the MehburMC Library (ARCHITECTURE K72): archive sanity, the mod
//! descriptor, and a constant-pool scan of every class for behaviour mods
//! rarely need. No scan proves a mod safe; it filters the obvious and
//! points admins at what to look at.

use std::collections::BTreeSet;
use std::io::{Cursor, Read};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::metadata::{clean, lenient_json, toml_value};

/// Upload limit (matches the `library` bucket).
pub const MAX_JAR_BYTES: usize = 25 * 1024 * 1024;
const MAX_ENTRIES: usize = 30_000;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// Nested jars (Fabric `META-INF/jars`, Forge `jarjar`) are scanned too.
const MAX_DEPTH: usize = 3;
/// Findings of one kind keep this many examples.
const MAX_EXAMPLES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    /// Worth a look by an admin; the upload is allowed.
    Warn,
    /// The jar is refused.
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Verdict {
    Pass,
    Warn,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    pub severity: Severity,
    /// Stable code, translated as `library.findings.<code>`.
    pub code: String,
    /// Where it was seen (class or file names), a few examples.
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ModLoader {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModDescriptor {
    pub mod_id: String,
    pub name: String,
    pub version: String,
    pub loaders: Vec<ModLoader>,
    /// Minecraft version requirement as written by the mod (may be empty).
    pub game_versions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanReport {
    pub verdict: Verdict,
    pub descriptor: Option<ModDescriptor>,
    pub findings: Vec<Finding>,
    pub sha1: String,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number")]
    pub classes: u32,
    /// Version of these rules, so old reports can be told apart.
    pub scanner: u32,
}

pub const SCANNER_VERSION: u32 = 1;

#[derive(Default)]
struct Collector {
    found: std::collections::BTreeMap<(Severity, &'static str), BTreeSet<String>>,
    classes: u32,
    short_names: u32,
}

impl Collector {
    fn add(&mut self, sev: Severity, code: &'static str, example: &str) {
        let set = self.found.entry((sev, code)).or_default();
        if set.len() < MAX_EXAMPLES {
            set.insert(example.chars().take(160).collect());
        }
    }

    fn has(&self, code: &str) -> bool {
        self.found.keys().any(|(_, c)| *c == code)
    }
}

/// What one class refers to, from its constant pool.
#[derive(Default, Debug)]
struct ClassRefs {
    /// `(owner class, member name)` of method references.
    methods: Vec<(String, String)>,
    classes: Vec<String>,
    /// String literals.
    strings: Vec<String>,
}

fn u16_at(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize)
}

/// Bounded constant pool parser; `None` for anything that is not a class.
fn class_refs(b: &[u8]) -> Option<ClassRefs> {
    if b.get(..4)? != [0xCA, 0xFE, 0xBA, 0xBE] {
        return None;
    }
    let count = u16_at(b, 8)?;
    // index → (tag, a, b) or utf8 text
    let mut utf8: Vec<Option<String>> = vec![None; count];
    let mut refs: Vec<(u8, usize, usize)> = vec![(0, 0, 0); count];
    let mut i = 10;
    let mut idx = 1;
    while idx < count {
        let tag = *b.get(i)?;
        i += 1;
        match tag {
            1 => {
                let len = u16_at(b, i)?;
                let bytes = b.get(i + 2..i + 2 + len)?;
                utf8[idx] = Some(String::from_utf8_lossy(bytes).into_owned());
                i += 2 + len;
            }
            3 | 4 => i += 4,
            5 | 6 => {
                i += 8;
                idx += 1; // takes two slots
            }
            7 | 8 | 16 | 19 | 20 => {
                refs[idx] = (tag, u16_at(b, i)?, 0);
                i += 2;
            }
            9..=12 | 17 | 18 => {
                refs[idx] = (tag, u16_at(b, i)?, u16_at(b, i + 2)?);
                i += 4;
            }
            15 => i += 3,
            _ => return None,
        }
        idx += 1;
    }
    let text = |n: usize| utf8.get(n).and_then(|s| s.clone());
    let class_name = |n: usize| match refs.get(n) {
        Some((7, name, _)) => text(*name),
        _ => None,
    };
    let mut out = ClassRefs::default();
    for (tag, a, c) in &refs {
        match tag {
            7 => out.classes.extend(text(*a)),
            8 => out.strings.extend(text(*a)),
            10 | 11 => {
                if let (Some(owner), Some((12, name, _))) = (class_name(*a), refs.get(*c)) {
                    out.methods.extend(text(*name).map(|n| (owner, n)));
                }
            }
            _ => {}
        }
    }
    Some(out)
}

const CREDENTIAL_MARKERS: &[&str] = &[
    "local storage\\leveldb",
    "local storage/leveldb",
    "\\login data",
    "/login data",
    "\\local state",
    "/local state",
    "launcher_accounts",
    "microsoft_accounts",
    "lunarclient/settings/game/accounts",
    ".lunarclient\\settings\\game\\accounts",
    "feather/accounts",
    "essential/microsoft_accounts",
    "wallet.dat",
    "exodus.wallet",
    "metamask",
    "\\discord\\",
    "/discord/",
    "discordcanary",
    "discordptb",
];

const EXFIL_MARKERS: &[&str] = &[
    "discord.com/api/webhooks",
    "discordapp.com/api/webhooks",
    "canary.discord.com/api/webhooks",
    "api.telegram.org/bot",
];

const SUSPICIOUS_HOSTS: &[&str] = &[
    "pastebin.com/raw",
    "hastebin.com/raw",
    "ngrok.io",
    "ngrok-free.app",
    "transfer.sh",
    "anonfiles",
    "iplogger",
    "grabify",
];

fn check_strings(c: &mut Collector, class: &str, r: &ClassRefs) {
    for s in &r.strings {
        let l = s.to_ascii_lowercase();
        if EXFIL_MARKERS.iter().any(|m| l.contains(m)) {
            c.add(Severity::Block, "webhook", class);
        }
        if CREDENTIAL_MARKERS.iter().any(|m| l.contains(m)) {
            c.add(Severity::Block, "credentialPaths", class);
        }
        if SUSPICIOUS_HOSTS.iter().any(|m| l.contains(m)) {
            c.add(Severity::Warn, "suspiciousUrl", class);
        }
        // Long base64-looking literals often carry hidden code.
        if s.len() > 4096
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        {
            c.add(Severity::Warn, "encodedBlob", class);
        }
    }
}

fn check_methods(c: &mut Collector, class: &str, r: &ClassRefs) {
    let mut base64 = false;
    let mut cipher = false;
    let mut define = false;
    for (owner, name) in &r.methods {
        match (owner.as_str(), name.as_str()) {
            ("java/lang/Runtime", "exec") | ("java/lang/ProcessBuilder", "start" | "<init>") => {
                c.add(Severity::Warn, "processExec", class)
            }
            ("java/lang/System" | "java/lang/Runtime", "load" | "loadLibrary") => {
                c.add(Severity::Warn, "nativeLoad", class)
            }
            (o, "defineClass" | "defineHiddenClass" | "defineAnonymousClass")
                if o.contains("ClassLoader") || o.ends_with("Lookup") || o.ends_with("Unsafe") =>
            {
                define = true;
                c.add(Severity::Warn, "defineClass", class)
            }
            ("java/util/Base64$Decoder", "decode") => base64 = true,
            ("javax/crypto/Cipher", "doFinal" | "init") => {
                cipher = true;
                c.add(Severity::Warn, "crypto", class)
            }
            ("java/net/URL", "openConnection" | "openStream")
            | ("java/net/http/HttpClient", "send" | "sendAsync")
            | ("java/net/Socket", "<init>")
            | ("java/net/DatagramSocket", "<init>") => c.add(Severity::Warn, "network", class),
            _ => {}
        }
    }
    // Decoding or decrypting data and turning it into classes is how
    // droppers hide their payload.
    if define && (base64 || cipher) {
        c.add(Severity::Block, "hiddenPayload", class);
    }
}

fn is_executable_name(n: &str) -> bool {
    let l = n.to_ascii_lowercase();
    [
        ".exe", ".scr", ".bat", ".cmd", ".ps1", ".vbs", ".msi", ".com", ".jse", ".wsf",
    ]
    .iter()
    .any(|e| l.ends_with(e))
}

fn is_native_name(n: &str) -> bool {
    let l = n.to_ascii_lowercase();
    [".dll", ".so", ".dylib", ".jnilib"]
        .iter()
        .any(|e| l.ends_with(e))
}

fn scan_zip(c: &mut Collector, bytes: &[u8], prefix: &str, depth: usize, total: &mut u64) {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        c.add(
            Severity::Block,
            "notAJar",
            if prefix.is_empty() { "/" } else { prefix },
        );
        return;
    };
    if zip.len() > MAX_ENTRIES {
        c.add(Severity::Block, "tooManyFiles", prefix);
        return;
    }
    for i in 0..zip.len() {
        let Ok(mut f) = zip.by_index(i) else {
            c.add(Severity::Block, "notAJar", prefix);
            return;
        };
        if f.is_dir() {
            continue;
        }
        let name = format!("{prefix}{}", f.name());
        let size = f.size();
        // Zip bombs: huge entries or absurd compression ratios.
        if size > MAX_ENTRY_BYTES || (size > 1024 * 1024 && size / f.compressed_size().max(1) > 200)
        {
            c.add(Severity::Block, "zipBomb", &name);
            return;
        }
        *total += size;
        if *total > MAX_TOTAL_BYTES {
            c.add(Severity::Block, "zipBomb", &name);
            return;
        }
        if is_executable_name(&name) {
            c.add(Severity::Block, "executable", &name);
        } else if is_native_name(&name) {
            c.add(Severity::Warn, "nativeBinary", &name);
        }
        let lower = name.to_ascii_lowercase();
        let full = lower.ends_with(".class") || lower.ends_with(".jar");
        // Other files only need their magic bytes.
        let limit = if full { size.min(MAX_ENTRY_BYTES) } else { 128 };
        let mut data = Vec::new();
        if (&mut f).take(limit).read_to_end(&mut data).is_err() {
            c.add(Severity::Block, "notAJar", &name);
            return;
        }
        // Executables renamed to something innocent (PE "MZ", ELF). Native
        // libraries are PE/ELF too and only warn (above).
        if !lower.ends_with(".class")
            && !is_native_name(&name)
            && (data.starts_with(b"MZ") && data.len() > 64 || data.starts_with(b"\x7fELF"))
        {
            c.add(Severity::Block, "executable", &name);
        }
        if lower.ends_with(".class") {
            c.classes += 1;
            match class_refs(&data) {
                Some(r) => {
                    let simple = name
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches(".class");
                    if simple.split('$').next().is_some_and(|s| s.len() <= 2) {
                        c.short_names += 1;
                    }
                    check_methods(c, &name, &r);
                    check_strings(c, &name, &r);
                }
                None => c.add(Severity::Warn, "badClass", &name),
            }
        } else if lower.ends_with(".jar") {
            if depth + 1 >= MAX_DEPTH {
                c.add(Severity::Warn, "deepNesting", &name);
            } else {
                scan_zip(c, &data, &format!("{name}!/"), depth + 1, total);
            }
        }
    }
}

fn read_text(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let f = zip.by_name(name).ok()?;
    if f.size() > 512 * 1024 {
        return None;
    }
    let mut s = String::new();
    f.take(512 * 1024).read_to_string(&mut s).ok()?;
    Some(s.trim_start_matches('\u{feff}').to_owned())
}

fn json_versions(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" || "),
        _ => String::new(),
    }
}

fn valid_id(id: &str) -> Option<String> {
    let id = id.trim().to_ascii_lowercase();
    (!id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')))
    .then_some(id)
}

/// The jar's own mod descriptor (root only).
pub fn descriptor(bytes: &[u8]) -> Option<ModDescriptor> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let short = |s: &str| {
        clean(s)
            .unwrap_or_default()
            .chars()
            .take(100)
            .collect::<String>()
    };
    if let Some(v) = read_text(&mut zip, "fabric.mod.json").and_then(|t| lenient_json(&t)) {
        let id = valid_id(v["id"].as_str()?)?;
        return Some(ModDescriptor {
            name: v["name"]
                .as_str()
                .and_then(clean)
                .unwrap_or_else(|| id.clone()),
            version: v["version"].as_str().and_then(clean).unwrap_or_default(),
            mod_id: id,
            loaders: vec![ModLoader::Fabric, ModLoader::Quilt],
            game_versions: short(&json_versions(&v["depends"]["minecraft"])),
        });
    }
    if let Some(v) = read_text(&mut zip, "quilt.mod.json").and_then(|t| lenient_json(&t)) {
        let ql = &v["quilt_loader"];
        let id = valid_id(ql["id"].as_str()?)?;
        let mc = ql["depends"]
            .as_array()
            .and_then(|d| {
                d.iter()
                    .find(|x| x["id"] == "minecraft")
                    .map(|x| json_versions(&x["versions"]))
            })
            .unwrap_or_default();
        return Some(ModDescriptor {
            name: ql["metadata"]["name"]
                .as_str()
                .and_then(clean)
                .unwrap_or_else(|| id.clone()),
            version: ql["version"].as_str().and_then(clean).unwrap_or_default(),
            mod_id: id,
            loaders: vec![ModLoader::Quilt],
            game_versions: short(&mc),
        });
    }
    for (file, loader) in [
        ("META-INF/neoforge.mods.toml", ModLoader::NeoForge),
        ("META-INF/mods.toml", ModLoader::Forge),
    ] {
        let Some(text) = read_text(&mut zip, file) else {
            continue;
        };
        let (mut id, mut name, mut version, mut mc) = (None, None, None, None);
        let mut section = String::new();
        let mut dep_is_mc = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('[') {
                section = line.to_owned();
                dep_is_mc = false;
                continue;
            }
            if section == "[[mods]]" {
                id = id.or_else(|| toml_value(line, "modId"));
                name = name.or_else(|| toml_value(line, "displayName"));
                version = version.or_else(|| toml_value(line, "version"));
            } else if section.starts_with("[[dependencies") {
                if toml_value(line, "modId").as_deref() == Some("minecraft") {
                    dep_is_mc = true;
                }
                if dep_is_mc {
                    mc = mc.or_else(|| toml_value(line, "versionRange"));
                }
            }
        }
        let id = valid_id(&id?)?;
        let version = match version {
            Some(v) if v.contains("${") => read_text(&mut zip, "META-INF/MANIFEST.MF")
                .and_then(|m| {
                    m.lines()
                        .find_map(|l| l.strip_prefix("Implementation-Version:"))
                        .map(|v| v.trim().to_owned())
                })
                .unwrap_or_default(),
            v => v.unwrap_or_default(),
        };
        return Some(ModDescriptor {
            name: name
                .as_deref()
                .and_then(clean)
                .unwrap_or_else(|| id.clone()),
            version: clean(&version).unwrap_or_default(),
            mod_id: id,
            loaders: vec![loader],
            game_versions: short(&mc.unwrap_or_default()),
        });
    }
    None
}

fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(bytes))
}

/// Checks a mod jar. Never panics on hostile input.
pub fn scan(bytes: &[u8]) -> ScanReport {
    let mut c = Collector::default();
    if bytes.len() > MAX_JAR_BYTES {
        c.add(Severity::Block, "tooLarge", "/");
    } else {
        let mut total = 0;
        scan_zip(&mut c, bytes, "", 0, &mut total);
    }
    let descriptor = if c.has("notAJar") {
        None
    } else {
        descriptor(bytes)
    };
    if descriptor.is_none() && !c.has("notAJar") && !c.has("tooLarge") {
        c.add(Severity::Block, "noDescriptor", "/");
    }
    if c.classes >= 20 && c.short_names * 2 > c.classes {
        c.add(
            Severity::Warn,
            "obfuscated",
            &format!("{}/{}", c.short_names, c.classes),
        );
    }
    let findings: Vec<Finding> = c
        .found
        .iter()
        .rev() // Block first
        .map(|((sev, code), ex)| Finding {
            severity: *sev,
            code: (*code).to_owned(),
            examples: ex.iter().cloned().collect(),
        })
        .collect();
    let verdict = if findings.iter().any(|f| f.severity == Severity::Block) {
        Verdict::Block
    } else if findings.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Warn
    };
    ScanReport {
        verdict,
        descriptor,
        findings,
        sha1: sha1_hex(bytes),
        size: bytes.len() as u64,
        classes: c.classes,
        scanner: SCANNER_VERSION,
    }
}

/// Reads a jar for the library, at most [`MAX_JAR_BYTES`].
pub fn read_jar(path: &std::path::Path) -> crate::error::Result<Vec<u8>> {
    use crate::error::CoreError;
    let f = std::fs::File::open(path).map_err(|e| CoreError::io(path, e))?;
    let mut bytes = Vec::new();
    f.take(MAX_JAR_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| CoreError::io(path, e))?;
    Ok(bytes)
}

/// Scans a file on disk; a file over the limit reports `tooLarge` with its
/// real size.
pub fn scan_file(path: &std::path::Path) -> crate::error::Result<ScanReport> {
    let bytes = read_jar(path)?;
    let mut r = scan(&bytes);
    if bytes.len() > MAX_JAR_BYTES {
        r.size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(r.size);
    }
    Ok(r)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use super::*;

    /// A minimal class file whose constant pool holds the given method
    /// references and string literals.
    pub fn class(methods: &[(&str, &str)], strings: &[&str]) -> Vec<u8> {
        let mut pool: Vec<Vec<u8>> = Vec::new();
        let utf8 = |pool: &mut Vec<Vec<u8>>, s: &str| {
            let mut e = vec![1];
            e.extend_from_slice(&(s.len() as u16).to_be_bytes());
            e.extend_from_slice(s.as_bytes());
            pool.push(e);
            pool.len() as u16
        };
        for (owner, name) in methods {
            let o = utf8(&mut pool, owner);
            pool.push([vec![7], o.to_be_bytes().to_vec()].concat());
            let class_idx = pool.len() as u16;
            let n = utf8(&mut pool, name);
            let d = utf8(&mut pool, "()V");
            pool.push([vec![12], n.to_be_bytes().to_vec(), d.to_be_bytes().to_vec()].concat());
            let nat = pool.len() as u16;
            pool.push(
                [
                    vec![10],
                    class_idx.to_be_bytes().to_vec(),
                    nat.to_be_bytes().to_vec(),
                ]
                .concat(),
            );
        }
        for s in strings {
            let u = utf8(&mut pool, s);
            pool.push([vec![8], u.to_be_bytes().to_vec()].concat());
        }
        // A long and a double take two slots each; make sure we handle it.
        pool.push([vec![5], vec![0; 8]].concat());
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        out.extend_from_slice(&((pool.len() + 2) as u16).to_be_bytes());
        for e in &pool {
            out.extend_from_slice(e);
        }
        out.extend_from_slice(&[0; 10]);
        out
    }

    pub fn jar(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (n, b) in files {
                z.start_file(*n, zip::write::SimpleFileOptions::default())
                    .unwrap();
                z.write_all(b).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    pub fn fabric_json() -> Vec<u8> {
        br#"{"schemaVersion":1,"id":"coolmod","name":"Cool Mod","version":"1.2.0","depends":{"minecraft":">=1.21"}}"#.to_vec()
    }

    fn codes(r: &ScanReport) -> Vec<&str> {
        r.findings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn clean_mod_passes() {
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "com/cool/Mod.class",
                class(&[("java/util/List", "add")], &["hello"]),
            ),
        ]));
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.findings);
        let d = r.descriptor.unwrap();
        assert_eq!(
            (d.mod_id.as_str(), d.version.as_str()),
            ("coolmod", "1.2.0")
        );
        assert_eq!(d.game_versions, ">=1.21");
        assert_eq!(r.classes, 1);
        assert_eq!(r.sha1.len(), 40);
    }

    #[test]
    fn stealer_patterns_block() {
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "a/Grab.class",
                class(
                    &[("java/net/URL", "openConnection")],
                    &[
                        "\\AppData\\Roaming\\discord\\Local Storage\\leveldb",
                        "https://discord.com/api/webhooks/1/abc",
                    ],
                ),
            ),
        ]));
        assert_eq!(r.verdict, Verdict::Block);
        let c = codes(&r);
        assert!(c.contains(&"webhook") && c.contains(&"credentialPaths") && c.contains(&"network"));
        // Block findings come first.
        assert_eq!(r.findings[0].severity, Severity::Block);
    }

    #[test]
    fn hidden_payload_and_executables_block() {
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "x/Loader.class",
                class(
                    &[
                        ("java/util/Base64$Decoder", "decode"),
                        ("java/lang/ClassLoader", "defineClass"),
                    ],
                    &[],
                ),
            ),
            ("assets/run.bat", b"@echo off".to_vec()),
            ("assets/icon.png", [b"MZ".to_vec(), vec![0; 100]].concat()),
        ]));
        assert_eq!(r.verdict, Verdict::Block);
        let c = codes(&r);
        assert!(c.contains(&"hiddenPayload") && c.contains(&"executable"));
        let exe = r.findings.iter().find(|f| f.code == "executable").unwrap();
        assert_eq!(exe.examples, ["assets/icon.png", "assets/run.bat"]);
    }

    #[test]
    fn risky_but_legit_behaviour_warns() {
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "m/Update.class",
                class(
                    &[
                        ("java/net/http/HttpClient", "send"),
                        ("java/lang/ProcessBuilder", "start"),
                        ("java/lang/System", "loadLibrary"),
                    ],
                    &["https://pastebin.com/raw/x"],
                ),
            ),
            ("natives/lib.dll", [b"MZ".to_vec(), vec![0; 100]].concat()),
            ("natives/lib.so", b"ELF....".to_vec()),
        ]));
        assert_eq!(r.verdict, Verdict::Warn, "{:?}", r.findings);
        let c = codes(&r);
        for want in [
            "network",
            "processExec",
            "nativeLoad",
            "nativeBinary",
            "suspiciousUrl",
        ] {
            assert!(c.contains(&want), "{want} missing in {c:?}");
        }
    }

    #[test]
    fn nested_jars_are_scanned() {
        let inner = jar(&[(
            "evil/Steal.class",
            class(&[], &["https://discordapp.com/api/webhooks/9/x"]),
        )]);
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            ("META-INF/jars/lib.jar", inner),
        ]));
        assert_eq!(r.verdict, Verdict::Block);
        let f = r.findings.iter().find(|f| f.code == "webhook").unwrap();
        assert_eq!(f.examples, ["META-INF/jars/lib.jar!/evil/Steal.class"]);
    }

    #[test]
    fn not_a_mod_or_garbage_blocks() {
        assert_eq!(codes(&scan(b"not a zip")), ["notAJar"]);
        let r = scan(&jar(&[("a.txt", b"hi".to_vec())]));
        assert_eq!(codes(&r), ["noDescriptor"]);
        // Truncated / random class bytes do not panic.
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            (
                "a/B.class",
                vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65, 0xFF, 0xFF, 7],
            ),
        ]));
        assert_eq!(codes(&r), ["badClass"]);
        assert_eq!(r.verdict, Verdict::Warn);
    }

    #[test]
    fn zip_bomb_blocks() {
        let r = scan(&jar(&[
            ("fabric.mod.json", fabric_json()),
            ("big.txt", vec![0u8; 8 * 1024 * 1024]),
        ]));
        assert!(codes(&r).contains(&"zipBomb"), "{:?}", r.findings);
    }

    #[test]
    fn forge_descriptor_with_minecraft_range() {
        let toml = b"modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\nversion=\"19.0\"\ndisplayName=\"Just Enough Items\"\n[[dependencies.jei]]\nmodId=\"forge\"\nversionRange=\"[47,)\"\n[[dependencies.jei]]\nmodId=\"minecraft\"\nversionRange=\"[1.20.1,1.21)\"\n";
        let d = descriptor(&jar(&[("META-INF/mods.toml", toml.to_vec())])).unwrap();
        assert_eq!(d.mod_id, "jei");
        assert_eq!(d.loaders, [ModLoader::Forge]);
        assert_eq!(d.game_versions, "[1.20.1,1.21)");
        let d = descriptor(&jar(&[("META-INF/neoforge.mods.toml", toml.to_vec())])).unwrap();
        assert_eq!(d.loaders, [ModLoader::NeoForge]);
        assert!(
            descriptor(&jar(&[(
                "fabric.mod.json",
                br#"{"id":"Bad Id!"}"#.to_vec()
            )]))
            .is_none()
        );
    }

    #[test]
    fn obfuscation_warns() {
        let cls = class(&[], &[]);
        let build = |names: &[String]| {
            let mut files = vec![("fabric.mod.json", fabric_json())];
            files.extend(names.iter().map(|n| (n.as_str(), cls.clone())));
            scan(&jar(&files))
        };
        // a/a.class … a/z.class: all one-letter names.
        let short: Vec<String> = (b'a'..=b'z')
            .map(|c| format!("a/{}.class", c as char))
            .collect();
        assert!(codes(&build(&short)).contains(&"obfuscated"));
        let normal: Vec<String> = (0..26)
            .map(|i| format!("com/cool/Feature{i}.class"))
            .collect();
        assert_eq!(build(&normal).verdict, Verdict::Pass);
    }

    /// Scans real jars (read-only) and prints the verdicts:
    /// `MEHBUR_JARS=<dir> cargo test -p launcher-core --lib real_jars -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_jars() {
        let dir = std::env::var("MEHBUR_JARS").expect("MEHBUR_JARS");
        let mut stack = vec![std::path::PathBuf::from(dir)];
        let mut seen = std::collections::HashSet::new();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "jar")
                    && p.parent()
                        .and_then(|d| d.file_name())
                        .is_some_and(|d| d == "mods")
                {
                    let bytes = std::fs::read(&p).unwrap();
                    let r = scan(&bytes);
                    if !seen.insert(r.sha1.clone()) {
                        continue;
                    }
                    let f: Vec<String> = r
                        .findings
                        .iter()
                        .map(|f| {
                            format!(
                                "{}({})",
                                f.code,
                                f.examples.first().cloned().unwrap_or_default()
                            )
                        })
                        .collect();
                    println!(
                        "{:?}	{}	{}",
                        r.verdict,
                        p.file_name().unwrap().to_string_lossy(),
                        f.join(" ")
                    );
                }
            }
        }
    }
}
