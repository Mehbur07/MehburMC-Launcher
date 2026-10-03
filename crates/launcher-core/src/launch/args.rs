//! Argument construction for both version formats.
//!
//! Modern: `arguments.jvm` / `arguments.game` arrays with rule-gated values.
//! Legacy: whitespace-separated `minecraftArguments` plus the JVM arguments the
//! official launcher hard-codes for those versions.

use std::collections::HashMap;

use crate::rules::{RuleEnv, allowed};
use crate::version::profile::{Argument, VersionJson};

/// Placeholder → value map (`auth_player_name` → `Steve`).
pub type Vars = HashMap<&'static str, String>;

/// Replaces every known `${name}`; unknown placeholders are left as-is.
pub fn substitute(input: &str, vars: &Vars) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push_str("${");
                        out.push_str(key);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn flatten(args: &[Argument], env: &RuleEnv, vars: &Vars) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Conditional { rules, value } => {
                if allowed(rules, env) {
                    out.extend(value.values().into_iter().map(|s| substitute(s, vars)));
                }
            }
        }
    }
    out
}

/// JVM arguments Mojang's launcher adds for versions without `arguments.jvm`.
fn legacy_jvm(env: &RuleEnv) -> Vec<String> {
    let mut v = Vec::new();
    if env.os.name == "osx" {
        v.push("-XstartOnFirstThread".into());
    }
    if env.os.name == "windows" {
        v.push(
            "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"
                .into(),
        );
    }
    if env.os.arch == "x86" {
        v.push("-Xss1M".into());
    }
    v.extend(
        [
            "-Djava.library.path=${natives_directory}",
            "-Dminecraft.launcher.brand=${launcher_name}",
            "-Dminecraft.launcher.version=${launcher_version}",
            "-cp",
            "${classpath}",
        ]
        .map(String::from),
    );
    v
}

/// `(jvm args, game args)` with placeholders resolved.
pub fn build(version: &VersionJson, env: &RuleEnv, vars: &Vars) -> (Vec<String>, Vec<String>) {
    match &version.arguments {
        Some(a) if !a.jvm.is_empty() || version.minecraft_arguments.is_none() => {
            let jvm = if a.jvm.is_empty() {
                legacy_jvm(env)
                    .iter()
                    .map(|s| substitute(s, vars))
                    .collect()
            } else {
                flatten(&a.jvm, env, vars)
            };
            let mut game = flatten(&a.game, env, vars);
            // Some loader profiles keep `minecraftArguments` alongside an
            // `arguments.jvm` block; the legacy string then carries the game args.
            if game.is_empty()
                && let Some(legacy) = &version.minecraft_arguments
            {
                game = legacy
                    .split_whitespace()
                    .map(|s| substitute(s, vars))
                    .collect();
            }
            (jvm, game)
        }
        _ => {
            let jvm = legacy_jvm(env)
                .iter()
                .map(|s| substitute(s, vars))
                .collect();
            let game = version
                .minecraft_arguments
                .as_deref()
                .unwrap_or_default()
                .split_whitespace()
                .map(|s| substitute(s, vars))
                .collect();
            (jvm, game)
        }
    }
}

/// Placeholders that survived substitution (diagnostics).
pub fn unresolved(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let mut rest = a.as_str();
        while let Some(s) = rest.find("${") {
            let after = &rest[s + 2..];
            if let Some(e) = after.find('}') {
                out.push(after[..e].to_owned());
                rest = &after[e + 1..];
            } else {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::os::OsInfo;

    fn fixture(name: &str) -> VersionJson {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/versions")
            .join(name);
        serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
    }

    fn vars() -> Vars {
        let mut v = Vars::new();
        for (k, val) in [
            ("auth_player_name", "Steve"),
            ("version_name", "X"),
            ("game_directory", "G"),
            ("assets_root", "A"),
            ("game_assets", "GA"),
            ("assets_index_name", "34"),
            ("auth_uuid", "uuid"),
            ("auth_access_token", "tok"),
            ("auth_session", "-"),
            ("clientid", "0"),
            ("auth_xuid", "0"),
            ("user_type", "legacy"),
            ("version_type", "release"),
            ("natives_directory", "N"),
            ("launcher_name", "MehburMC Launcher"),
            ("launcher_version", "0.1.0"),
            ("classpath", "CP"),
            ("resolution_width", "1280"),
            ("resolution_height", "720"),
        ] {
            v.insert(k, val.to_owned());
        }
        v
    }

    #[test]
    fn substitution() {
        let v = vars();
        assert_eq!(
            substitute("-Djava.library.path=${natives_directory}/java", &v),
            "-Djava.library.path=N/java"
        );
        assert_eq!(substitute("${unknown}x", &v), "${unknown}x");
        assert_eq!(substitute("tail ${", &v), "tail ${");
        assert_eq!(
            substitute("${auth_player_name}${auth_uuid}", &v),
            "Steveuuid"
        );
    }

    #[test]
    fn modern_26_3_windows() {
        let env = RuleEnv::new(OsInfo::fake("windows", "x86_64"));
        let (jvm, game) = build(&fixture("26.3.json"), &env, &vars());
        assert!(jvm.iter().any(|a| a.starts_with("-XX:HeapDumpPath=")));
        assert!(!jvm.contains(&"-XstartOnFirstThread".to_owned()));
        assert!(!jvm.contains(&"-Xss1M".to_owned()));
        assert!(jvm.contains(&"-Djava.library.path=N/java".to_owned()));
        assert!(jvm.contains(&"-Dminecraft.launcher.brand=MehburMC Launcher".to_owned()));
        let cp = jvm.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(jvm[cp + 1], "CP");

        assert_eq!(&game[..2], ["--username", "Steve"]);
        assert!(!game.contains(&"--demo".to_owned()));
        assert!(!game.contains(&"--width".to_owned()));
        assert!(!game.iter().any(|a| a.starts_with("--quickPlay")));
        assert!(unresolved(&jvm).is_empty() && unresolved(&game).is_empty());
    }

    #[test]
    fn custom_resolution_feature() {
        let env = RuleEnv::new(OsInfo::fake("windows", "x86_64"))
            .with_feature("has_custom_resolution", true);
        let (_, game) = build(&fixture("26.3.json"), &env, &vars());
        let w = game.iter().position(|a| a == "--width").unwrap();
        assert_eq!(game[w + 1], "1280");
    }

    #[test]
    fn legacy_1_12_2_and_1_5_2() {
        let env = RuleEnv::new(OsInfo::fake("windows", "x86_64"));
        let (jvm, game) = build(&fixture("1.12.2.json"), &env, &vars());
        assert_eq!(jvm.last().unwrap(), "CP");
        assert!(jvm.contains(&"-Djava.library.path=N".to_owned()));
        assert_eq!(&game[..2], ["--username", "Steve"]);
        assert!(game.contains(&"--userType".to_owned()));

        let (_, game) = build(&fixture("1.5.2.json"), &env, &vars());
        assert_eq!(&game[..4], ["Steve", "-", "--gameDir", "G"]);
        assert_eq!(game[5], "GA");
    }
}
