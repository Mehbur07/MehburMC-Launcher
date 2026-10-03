//! Crash diagnosis: pattern matching over the game's output, crash report
//! and JVM fatal-error log (`hs_err_pid*.log`) → hints the UI can translate.
//!
//! Patterns are deliberately conservative; an unknown crash still yields the
//! first exception line as a summary.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

use regex::Regex;
use serde::Serialize;
use ts_rs::TS;

/// Files larger than this are only read from the end.
const MAX_READ: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CrashKind {
    /// The game ran out of heap.
    OutOfMemory,
    /// The JVM could not reserve the requested heap (too much RAM, 32-bit Java).
    HeapReserve,
    /// Class files need a newer Java; `detail` = required major version.
    JavaTooOld,
    /// Old (launchwrapper) versions running on Java 9+.
    JavaTooNew,
    /// A mod needs another mod that is missing; `detail` = the loader's line.
    MissingDependency,
    /// The loader reports mods that cannot run together.
    IncompatibleMods,
    /// The same mod twice in `mods/`.
    DuplicateMod,
    /// A mixin failed to apply; `detail` = mod or mixin config when known.
    MixinFailure,
    /// Missing class/method/field: a mod built for another version.
    MissingClass,
    /// OpenGL context / pixel format problems.
    GraphicsDriver,
    /// The JVM itself crashed (access violation); `detail` = problematic frame.
    NativeCrash,
    /// A corrupt jar or zip; repair usually helps.
    CorruptFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Diagnosis {
    pub kind: CrashKind,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrashInfo {
    pub instance_id: String,
    pub instance_name: String,
    pub exit_code: Option<i32>,
    /// Most specific first.
    pub diagnoses: Vec<Diagnosis>,
    /// Crash report "Description" or the first exception line.
    pub summary: Option<String>,
    pub crash_report: Option<String>,
    pub hs_err: Option<String>,
    pub log_file: Option<String>,
}

fn re(p: &str) -> Regex {
    Regex::new(p).expect("valid crash pattern")
}

struct Rule {
    kind: CrashKind,
    /// Any of these substrings triggers the rule.
    needles: &'static [&'static str],
    /// Optional detail extractor (first capture group).
    detail: Option<&'static LazyLock<Regex>>,
}

static CLASS_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    re(r"class file version (\d+)(?:\.\d+)?\)?, this version of the Java Runtime")
});
static MISSING_DEP: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?m)^\s*-?\s*(Mod '.+?' \(.+?\) .*? requires .+? which is missing.*?)\s*$|Mod ID: '([\w\-]+)', Requested by: '([\w\-]+)'",
    )
});
static INCOMPATIBLE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?m)^\s*-?\s*(Mod '.+?' \(.+?\) .*?is incompatible with .+?)\s*$"));
static DUPLICATE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)(?:duplicate mods? found|found duplicate mods?|duplicate mod)[^\n]*?[:\s]+([\w\-.]+)")
});
static MIXIN: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?:Mixin \[([\w\-.]+\.json)[:\]]|from mod ([\w\-]+)|mixin config ([\w\-.]+\.json))")
});
static MISSING_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"java\.lang\.(?:NoSuchMethodError|NoSuchFieldError|NoClassDefFoundError|ClassNotFoundException):\s*([^\s]+)",
    )
});
static FRAME: LazyLock<Regex> = LazyLock::new(|| re(r"(?m)^#\s+[CjJvV]\s+\[([^\]+]+)"));
static CORRUPT: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:ZipException|zip file|jar file)[^\n]*?([\w\-.]+\.(?:jar|zip))"));

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        Rule {
            kind: CrashKind::JavaTooOld,
            needles: &[
                "UnsupportedClassVersionError",
                "compiled by a more recent version of the Java Runtime",
            ],
            detail: Some(&CLASS_VERSION),
        },
        Rule {
            kind: CrashKind::JavaTooNew,
            needles: &[
                "cannot be cast to class java.net.URLClassLoader",
                "ClassLoaders$AppClassLoader cannot be cast",
            ],
            detail: None,
        },
        Rule {
            kind: CrashKind::HeapReserve,
            needles: &[
                "Could not reserve enough space for",
                "Invalid maximum heap size",
                "Initial heap size set to a larger value than the maximum heap size",
            ],
            detail: None,
        },
        Rule {
            kind: CrashKind::OutOfMemory,
            needles: &["java.lang.OutOfMemoryError", "GC overhead limit exceeded"],
            detail: None,
        },
        Rule {
            kind: CrashKind::MissingDependency,
            needles: &[
                "which is missing!",
                "which is missing",
                "Missing or unsupported mandatory dependencies",
                "MissingModsException",
            ],
            detail: Some(&MISSING_DEP),
        },
        Rule {
            kind: CrashKind::DuplicateMod,
            needles: &[
                "DuplicateModsFoundException",
                "Duplicate mods found",
                "Found duplicate mods",
                "duplicate mod",
            ],
            detail: Some(&DUPLICATE),
        },
        Rule {
            kind: CrashKind::IncompatibleMods,
            needles: &[
                "is incompatible with",
                "Incompatible mods found",
                "incompatible mod set",
            ],
            detail: Some(&INCOMPATIBLE),
        },
        Rule {
            kind: CrashKind::MixinFailure,
            needles: &[
                "Mixin apply failed",
                "MixinApplyError",
                "InvalidInjectionException",
                "MixinTransformerError",
                "Mixin transformation of",
            ],
            detail: Some(&MIXIN),
        },
        Rule {
            kind: CrashKind::GraphicsDriver,
            needles: &[
                "Pixel format not accelerated",
                "GLFW error 65542",
                "GLFW error 65543",
                "does not appear to support OpenGL",
                "Couldn't set pixel format",
                "No OpenGL context found",
                "OpenGL 3.2",
            ],
            detail: None,
        },
        Rule {
            kind: CrashKind::CorruptFile,
            needles: &[
                "java.util.zip.ZipException",
                "invalid CEN header",
                "zip END header not found",
                "error in opening zip file",
            ],
            detail: Some(&CORRUPT),
        },
        Rule {
            kind: CrashKind::MissingClass,
            needles: &[
                "java.lang.NoSuchMethodError",
                "java.lang.NoSuchFieldError",
                "java.lang.NoClassDefFoundError",
                "java.lang.ClassNotFoundException",
            ],
            detail: Some(&MISSING_CLASS),
        },
        Rule {
            kind: CrashKind::NativeCrash,
            needles: &[
                "EXCEPTION_ACCESS_VIOLATION",
                "A fatal error has been detected by the Java Runtime Environment",
            ],
            detail: Some(&FRAME),
        },
    ]
});

/// Driver DLLs whose presence in a native crash points at the GPU driver.
const GPU_DLLS: &[&str] = &[
    "atio6axx",
    "atioglxx",
    "amdxx",
    "nvoglv",
    "ig7icd",
    "ig75icd",
    "ig9icd",
    "igxelpicd",
    "opengl32",
];

fn capture(re: &Regex, text: &str) -> Option<String> {
    let c = re.captures(text)?;
    let joined: Vec<&str> = c.iter().skip(1).flatten().map(|m| m.as_str()).collect();
    let s = joined.join(" ← ").trim().to_owned();
    (!s.is_empty()).then(|| s.chars().take(300).collect())
}

/// Runs every rule over the combined texts. Order = rule priority.
pub fn diagnose(text: &str) -> Vec<Diagnosis> {
    let mut out: Vec<Diagnosis> = Vec::new();
    for rule in RULES.iter() {
        if !rule.needles.iter().any(|n| text.contains(n)) {
            continue;
        }
        let mut detail = rule.detail.and_then(|r| capture(r, text));
        if rule.kind == CrashKind::JavaTooOld {
            // Class file version → Java major (52 = Java 8).
            detail = detail
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|v| *v >= 45)
                .map(|v| (v - 44).to_string());
        }
        if rule.kind == CrashKind::NativeCrash
            && let Some(frame) = &detail
            && GPU_DLLS
                .iter()
                .any(|d| frame.to_ascii_lowercase().starts_with(d))
            && !out.iter().any(|d| d.kind == CrashKind::GraphicsDriver)
        {
            out.push(Diagnosis {
                kind: CrashKind::GraphicsDriver,
                detail: detail.clone(),
            });
        }
        out.push(Diagnosis {
            kind: rule.kind,
            detail,
        });
    }
    // A missing class is usually a consequence of a dependency/version issue.
    let specific = out.iter().any(|d| {
        matches!(
            d.kind,
            CrashKind::MissingDependency | CrashKind::IncompatibleMods | CrashKind::JavaTooOld
        )
    });
    if specific {
        out.retain(|d| d.kind != CrashKind::MissingClass);
    }
    out
}

static DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| re(r"(?m)^Description: (.+)$"));
static EXCEPTION_LINE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?m)^\s*(?:Caused by: |Exception in thread .+? )?((?:[a-z][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error)\b(?::[^\n]*)?)\s*$",
    )
});

/// One-line human summary: crash report description + exception, else the
/// last top-level exception in the output.
pub fn summary(crash_report: Option<&str>, output: &str) -> Option<String> {
    let clip = |s: &str| s.trim().chars().take(240).collect::<String>();
    if let Some(r) = crash_report {
        let desc = DESCRIPTION.captures(r).map(|c| c[1].to_owned());
        let exc = EXCEPTION_LINE.captures(r).map(|c| c[1].to_owned());
        match (desc, exc) {
            (Some(d), Some(e)) => return Some(clip(&format!("{d}: {e}"))),
            (Some(d), None) => return Some(clip(&d)),
            (None, Some(e)) => return Some(clip(&e)),
            _ => {}
        }
    }
    EXCEPTION_LINE
        .captures_iter(output)
        .last()
        .map(|c| clip(&c[1]))
}

/// Reads at most [`MAX_READ`] bytes from the end of a file.
pub fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len > MAX_READ {
        f.seek(SeekFrom::Start(len - MAX_READ)).ok()?;
    }
    let mut buf = Vec::new();
    f.take(MAX_READ).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Newest `hs_err_pid*.log` in the game directory written since `since`.
pub fn newest_hs_err(game_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    std::fs::read_dir(game_dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            n.starts_with("hs_err_pid") && n.ends_with(".log")
        })
        .filter_map(|e| {
            let m = e.metadata().ok()?.modified().ok()?;
            (m >= since).then(|| (m, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

pub struct Inputs<'a> {
    pub game_dir: &'a Path,
    pub exit_code: Option<i32>,
    pub crash_report: Option<&'a Path>,
    /// Last lines the game printed (stdout + stderr).
    pub output: &'a str,
    pub started: SystemTime,
}

/// Collects every source and diagnoses the crash.
pub fn analyze(instance_id: &str, instance_name: &str, i: Inputs<'_>) -> CrashInfo {
    let report = i.crash_report.and_then(read_tail);
    let hs_err = newest_hs_err(i.game_dir, i.started);
    let hs_text = hs_err.as_deref().and_then(read_tail);
    let log_file = i.game_dir.join("logs").join("latest.log");
    let log_file = log_file
        .metadata()
        .and_then(|m| m.modified())
        .is_ok_and(|t| t >= i.started)
        .then_some(log_file);
    let log_text = log_file.as_deref().and_then(read_tail);

    let mut all = String::new();
    for part in [
        report.as_deref(),
        hs_text.as_deref(),
        log_text.as_deref(),
        Some(i.output),
    ]
    .into_iter()
    .flatten()
    {
        all.push_str(part);
        all.push('\n');
    }
    let mut diagnoses = diagnose(&all);
    // Windows NTSTATUS codes (negative) without any other clue: native crash.
    if diagnoses.is_empty()
        && let Some(c) = i.exit_code
        && c < 0
    {
        diagnoses.push(Diagnosis {
            kind: CrashKind::NativeCrash,
            detail: Some(format!("0x{:08X}", c as u32)),
        });
    }
    CrashInfo {
        instance_id: instance_id.to_owned(),
        instance_name: instance_name.to_owned(),
        exit_code: i.exit_code,
        diagnoses,
        summary: summary(
            report.as_deref(),
            &format!("{}\n{}", log_text.as_deref().unwrap_or(""), i.output),
        ),
        crash_report: i.crash_report.map(|p| p.display().to_string()),
        hs_err: hs_err.map(|p| p.display().to_string()),
        log_file: log_file.map(|p| p.display().to_string()),
    }
}

/// Analyses an existing crash report (Logs tab).
pub fn analyze_file(instance_id: &str, instance_name: &str, path: &Path) -> CrashInfo {
    let text = read_tail(path).unwrap_or_default();
    CrashInfo {
        instance_id: instance_id.to_owned(),
        instance_name: instance_name.to_owned(),
        exit_code: None,
        diagnoses: diagnose(&text),
        summary: summary(Some(&text), &text),
        crash_report: Some(path.display().to_string()),
        hs_err: None,
        log_file: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<CrashKind> {
        diagnose(text).into_iter().map(|d| d.kind).collect()
    }

    #[test]
    fn java_version_problems() {
        let d = diagnose(
            "Exception in thread \"main\" java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main \
             has been compiled by a more recent version of the Java Runtime (class file version 69.0), \
             this version of the Java Runtime only recognizes class file versions up to 61.0",
        );
        assert_eq!(d[0].kind, CrashKind::JavaTooOld);
        assert_eq!(d[0].detail.as_deref(), Some("25"));
        assert_eq!(
            kinds(
                "java.lang.ClassCastException: class jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to class java.net.URLClassLoader"
            ),
            [CrashKind::JavaTooNew]
        );
    }

    #[test]
    fn memory() {
        assert_eq!(
            kinds("java.lang.OutOfMemoryError: Java heap space"),
            [CrashKind::OutOfMemory]
        );
        assert_eq!(
            kinds(
                "Error occurred during initialization of VM\nCould not reserve enough space for 8388608KB object heap"
            ),
            [CrashKind::HeapReserve]
        );
    }

    #[test]
    fn fabric_dependency_and_incompatibility() {
        let log = "net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an incompatible mod set!\n\
                   A potential solution has been determined:\n\
                   \t - Install fabric-api, any version.\n\
                   Unmet dependency listing:\n\
                   \t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of 'fabric-api', which is missing!\n\
                   \t - Mod 'A' (a) 1.0 is incompatible with any version of mod 'B' (b)\n\
                   java.lang.NoClassDefFoundError: net/fabricmc/fabric/api/Foo";
        let d = diagnose(log);
        assert_eq!(d[0].kind, CrashKind::MissingDependency);
        assert!(d[0].detail.as_deref().unwrap().contains("fabric-api"));
        assert!(d.iter().any(|x| x.kind == CrashKind::IncompatibleMods));
        assert!(
            !d.iter().any(|x| x.kind == CrashKind::MissingClass),
            "consequence hidden"
        );
    }

    #[test]
    fn forge_missing_mod() {
        let d = diagnose(
            "Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'mowziesmobs', Expected range: '[4.2,)'",
        );
        assert_eq!(d[0].kind, CrashKind::MissingDependency);
        assert_eq!(d[0].detail.as_deref(), Some("geckolib ← mowziesmobs"));
    }

    #[test]
    fn mixin_and_missing_class() {
        let d = diagnose(
            "org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered\n\
             Caused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [iris.mixins.json:MixinFoo] from mod iris failed",
        );
        assert_eq!(d[0].kind, CrashKind::MixinFailure);
        assert_eq!(d[0].detail.as_deref(), Some("iris.mixins.json"));
        let d = diagnose("java.lang.NoSuchMethodError: 'void net.minecraft.class_310.method_1()'");
        assert_eq!(d[0].kind, CrashKind::MissingClass);
    }

    #[test]
    fn native_crash_in_gpu_driver() {
        let hs = "# A fatal error has been detected by the Java Runtime Environment:\n\
                  #  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ff9, pid=1234\n\
                  # Problematic frame:\n# C  [atio6axx.dll+0x1b2c3d]\n";
        let d = diagnose(hs);
        assert_eq!(d[0].kind, CrashKind::GraphicsDriver);
        assert_eq!(d[1].kind, CrashKind::NativeCrash);
        assert_eq!(d[1].detail.as_deref(), Some("atio6axx.dll"));
        assert!(
            kinds("GLFW error 65542: WGL: The driver does not appear to support OpenGL")
                .contains(&CrashKind::GraphicsDriver)
        );
    }

    #[test]
    fn corrupt_and_duplicate() {
        assert_eq!(
            diagnose("java.util.zip.ZipException: invalid CEN header (bad entry name) in jar file sodium-0.5.jar")[0].detail.as_deref(),
            Some("sodium-0.5.jar")
        );
        assert_eq!(
            kinds("net.minecraftforge.fml.ModLoadingException: DuplicateModsFoundException"),
            [CrashKind::DuplicateMod]
        );
        assert!(kinds("[main/INFO]: Loading 42 mods").is_empty());
    }

    #[test]
    fn summary_prefers_crash_report() {
        let report = "---- Minecraft Crash Report ----\nTime: x\nDescription: Rendering overlay\n\njava.lang.NullPointerException: Cannot invoke \"Foo.bar()\"\n\tat a.b.C.d(C.java:1)";
        assert_eq!(
            summary(Some(report), "").as_deref(),
            Some("Rendering overlay: java.lang.NullPointerException: Cannot invoke \"Foo.bar()\"")
        );
        assert_eq!(
            summary(None, "boot\njava.lang.IllegalStateException: first\nmore\nCaused by: java.io.IOException: disk").as_deref(),
            Some("java.io.IOException: disk")
        );
    }

    #[test]
    fn analyze_collects_sources_and_exit_codes() {
        let tmp = tempfile::tempdir().unwrap();
        let since = SystemTime::now() - std::time::Duration::from_secs(2);
        std::fs::write(
            tmp.path().join("hs_err_pid42.log"),
            "EXCEPTION_ACCESS_VIOLATION\n# C  [lwjgl.dll+0x1]\n",
        )
        .unwrap();
        let info = analyze(
            "i",
            "I",
            Inputs {
                game_dir: tmp.path(),
                exit_code: Some(-1073741819),
                crash_report: None,
                output: "",
                started: since,
            },
        );
        assert_eq!(info.diagnoses[0].kind, CrashKind::NativeCrash);
        assert_eq!(info.diagnoses[0].detail.as_deref(), Some("lwjgl.dll"));
        assert!(info.hs_err.is_some());

        let empty = tempfile::tempdir().unwrap();
        let info = analyze(
            "i",
            "I",
            Inputs {
                game_dir: empty.path(),
                exit_code: Some(-1073741819),
                crash_report: None,
                output: "",
                started: since,
            },
        );
        assert_eq!(info.diagnoses[0].detail.as_deref(), Some("0xC0000005"));
    }
}
