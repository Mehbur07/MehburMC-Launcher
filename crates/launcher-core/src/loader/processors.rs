//! Runs the "processors" of a modern Forge/NeoForge installer
//! (`install_profile.json` spec 1): small Java tools that patch the vanilla
//! client jar, extract data and remap classes.
//!
//! Value syntax (same as the official installer):
//! - `[group:artifact:version]` → path of that library
//! - `'literal'`                → the literal text
//! - `{KEY}`                    → a data entry (may appear inside a string)
//! - `/path/in/installer`       → (data entries only) file extracted from the installer

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::archive;
use crate::error::{CoreError, Result};
use crate::events::Progress;
use crate::hash::sha1_file;
use crate::java::JavaInstall;
use crate::maven::Coordinate;

#[derive(Debug, Clone, Deserialize)]
pub struct Processor {
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// `{path expression: sha1 expression}`
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    /// Missing = both sides.
    #[serde(default)]
    pub sides: Option<Vec<String>>,
}

impl Processor {
    pub fn runs_on_client(&self) -> bool {
        self.sides
            .as_ref()
            .is_none_or(|s| s.iter().any(|x| x == "client"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SidedValue {
    pub client: String,
}

pub struct Env<'a> {
    pub loader: &'static str,
    pub libraries: &'a Path,
    /// Scratch folder for files extracted from the installer.
    pub work_dir: &'a Path,
    pub java: &'a JavaInstall,
    pub data: HashMap<String, String>,
}

fn fail(loader: &str, reason: impl Into<String>) -> CoreError {
    CoreError::LoaderInstall {
        loader: loader.to_owned(),
        reason: reason.into(),
    }
}

pub fn library_path(libraries: &Path, coord: &str) -> Option<PathBuf> {
    let c = Coordinate::parse(coord)?;
    Some(
        c.path()
            .split('/')
            .fold(libraries.to_path_buf(), |p, s| p.join(s)),
    )
}

/// Resolves the installer's `data` section for the client side.
pub fn resolve_data(
    raw: &HashMap<String, SidedValue>,
    builtins: &[(&str, String)],
    loader: &'static str,
    libraries: &Path,
    installer: &Path,
    work_dir: &Path,
) -> Result<HashMap<String, String>> {
    let mut out: HashMap<String, String> = builtins
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    for (key, value) in raw {
        let v = value.client.as_str();
        let resolved = if let Some(coord) = v.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            library_path(libraries, coord)
                .ok_or_else(|| fail(loader, format!("bad coordinate {coord}")))?
                .display()
                .to_string()
        } else if let Some(lit) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            lit.to_owned()
        } else if let Some(entry) = v.strip_prefix('/') {
            let dest = archive::safe_join(work_dir, entry)
                .ok_or_else(|| fail(loader, format!("unsafe installer path {v}")))?;
            if !archive::extract_entry(installer, entry, &dest)? {
                return Err(fail(
                    loader,
                    format!("{entry} is missing from the installer"),
                ));
            }
            dest.display().to_string()
        } else {
            v.to_owned()
        };
        out.insert(key.clone(), resolved);
    }
    Ok(out)
}

/// Expands one argument / output expression.
pub fn expand(expr: &str, env: &Env<'_>) -> Result<String> {
    if let Some(coord) = expr.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return library_path(env.libraries, coord)
            .map(|p| p.display().to_string())
            .ok_or_else(|| fail(env.loader, format!("bad coordinate {coord}")));
    }
    if let Some(lit) = expr.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(lit.to_owned());
    }
    let mut out = String::with_capacity(expr.len());
    let mut rest = expr;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let key = &after[..end];
        let value = env
            .data
            .get(key)
            .ok_or_else(|| fail(env.loader, format!("unknown installer variable {{{key}}}")))?;
        // Data values may themselves be literals ('…').
        out.push_str(
            value
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .unwrap_or(value),
        );
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// `true` if every declared output exists with the expected SHA-1.
fn outputs_valid(p: &Processor, env: &Env<'_>) -> Result<bool> {
    if p.outputs.is_empty() {
        return Ok(false);
    }
    for (file, sha) in &p.outputs {
        let path = PathBuf::from(expand(file, env)?);
        let expected = expand(sha, env)?;
        if !path.is_file() || !sha1_file(&path)?.eq_ignore_ascii_case(&expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub async fn run_all(
    processors: &[Processor],
    env: &Env<'_>,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<()> {
    let client: Vec<&Processor> = processors.iter().filter(|p| p.runs_on_client()).collect();
    progress.set_totals(client.len() as u64, 0);
    for (i, p) in client.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if outputs_valid(p, env)? {
            tracing::debug!(jar = %p.jar, "processor outputs up to date, skipping");
            progress.item_done();
            continue;
        }
        run_one(i, p, env, cancel).await?;
        for (file, sha) in &p.outputs {
            let path = PathBuf::from(expand(file, env)?);
            let expected = expand(sha, env)?;
            let actual = sha1_file(&path)?;
            if !actual.eq_ignore_ascii_case(&expected) {
                let _ = std::fs::remove_file(&path);
                return Err(CoreError::HashMismatch {
                    path,
                    expected,
                    actual,
                });
            }
        }
        progress.item_done();
    }
    Ok(())
}

async fn run_one(
    index: usize,
    p: &Processor,
    env: &Env<'_>,
    cancel: &CancellationToken,
) -> Result<()> {
    let jar = library_path(env.libraries, &p.jar)
        .ok_or_else(|| fail(env.loader, format!("bad processor {}", p.jar)))?;
    let main = archive::main_class(&jar)?
        .ok_or_else(|| fail(env.loader, format!("{} has no Main-Class", p.jar)))?;
    let mut cp = vec![jar.display().to_string()];
    for c in &p.classpath {
        let path = library_path(env.libraries, c)
            .ok_or_else(|| fail(env.loader, format!("bad coordinate {c}")))?;
        cp.push(path.display().to_string());
    }
    let sep = if cfg!(windows) { ";" } else { ":" };
    let args = p
        .args
        .iter()
        .map(|a| expand(a, env))
        .collect::<Result<Vec<_>>>()?;

    tracing::info!(index, jar = %p.jar, %main, "running installer processor");
    tracing::debug!(?args, "processor arguments");
    let mut cmd = Command::new(env.java.executable_path());
    cmd.arg("-cp")
        .arg(cp.join(sep))
        .arg(&main)
        .args(&args)
        .current_dir(env.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = cmd.spawn().map_err(|source| CoreError::Spawn {
        program: env.java.executable_path(),
        source,
    })?;
    let output = tokio::select! {
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        o = child.wait_with_output() => o.map_err(|e| CoreError::io(env.work_dir, e))?,
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for line in text.lines() {
        tracing::debug!(target: "processor", "{line}");
    }
    if !output.status.success() {
        let tail: Vec<&str> = text.lines().rev().take(8).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(fail(
            env.loader,
            format!(
                "{} exited with {:?}: {}",
                p.jar,
                output.status.code(),
                tail.join(" | ")
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java() -> JavaInstall {
        JavaInstall {
            executable: "java".into(),
            home: String::new(),
            major: 21,
            version: "21".into(),
            vendor: None,
            arch: None,
            managed: false,
        }
    }

    #[test]
    fn expands_all_value_forms() {
        let j = java();
        let libs = Path::new("L");
        let mut data = HashMap::new();
        data.insert("ROOT".to_owned(), "R".to_owned());
        data.insert("SHA".to_owned(), "'abc'".to_owned());
        let env = Env {
            loader: "Forge",
            libraries: libs,
            work_dir: Path::new("W"),
            java: &j,
            data,
        };
        assert_eq!(expand("{ROOT}/libraries/", &env).unwrap(), "R/libraries/");
        assert_eq!(expand("{SHA}", &env).unwrap(), "abc");
        assert_eq!(expand("'lit'", &env).unwrap(), "lit");
        assert_eq!(expand("--flag", &env).unwrap(), "--flag");
        assert_eq!(
            PathBuf::from(expand("[de.oceanlabs.mcp:mcp_config:1.20.1@zip]", &env).unwrap()),
            libs.join("de")
                .join("oceanlabs")
                .join("mcp")
                .join("mcp_config")
                .join("1.20.1")
                .join("mcp_config-1.20.1.zip")
        );
        assert_eq!(
            expand("{NOPE}", &env).unwrap_err().code(),
            "loader.installFailed"
        );
    }

    #[test]
    fn data_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let installer = dir.path().join("installer.jar");
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(std::fs::File::create(&installer).unwrap());
            w.start_file("data/client.lzma", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"patch").unwrap();
            w.finish().unwrap();
        }
        let raw: HashMap<String, SidedValue> = serde_json::from_str(
            r#"{"BINPATCH":{"client":"/data/client.lzma","server":"/data/server.lzma"},
                "PATCHED_SHA":{"client":"'6254'","server":"'x'"},
                "PATCHED":{"client":"[net.minecraftforge:forge:26.3-66.0.9:client]","server":"[x:y:z]"}}"#,
        )
        .unwrap();
        let work = dir.path().join("work");
        let libs = dir.path().join("libs");
        let data = resolve_data(
            &raw,
            &[("SIDE", "client".into())],
            "Forge",
            &libs,
            &installer,
            &work,
        )
        .unwrap();
        assert_eq!(data["SIDE"], "client");
        assert_eq!(data["PATCHED_SHA"], "6254");
        assert!(data["PATCHED"].ends_with("forge-26.3-66.0.9-client.jar"));
        assert_eq!(std::fs::read(&data["BINPATCH"]).unwrap(), b"patch");

        let bad: HashMap<String, SidedValue> =
            serde_json::from_str(r#"{"X":{"client":"/../../evil"}}"#).unwrap();
        assert!(resolve_data(&bad, &[], "Forge", &libs, &installer, &work).is_err());
    }

    #[test]
    fn sides() {
        let p: Processor = serde_json::from_str(r#"{"jar":"a:b:1","sides":["server"]}"#).unwrap();
        assert!(!p.runs_on_client());
        let p: Processor = serde_json::from_str(r#"{"jar":"a:b:1"}"#).unwrap();
        assert!(p.runs_on_client());
    }
}
