//! Launch pipeline: resolve version → download/verify files → pick Java →
//! build the command line. [`process::run`] then starts the game.

pub mod args;
pub mod cmdline;
pub mod log4j;
pub mod process;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio_util::sync::CancellationToken;

use crate::assets;
use crate::auth::LaunchAccount;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::events::{Progress, Stage};
use crate::hash::Checksum;
use crate::java::{self, JavaInstall};
use crate::library;
use crate::natives;
use crate::net::download::{DownloadItem, Verify};
use crate::rules::RuleEnv;
use crate::version;

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub version_id: String,
    pub game_dir: PathBuf,
    pub account: LaunchAccount,
    pub max_memory_mb: u32,
    pub min_memory_mb: Option<u32>,
    /// Explicit `java` executable; otherwise chosen automatically.
    pub java_path: Option<PathBuf>,
    pub auto_download_java: bool,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub resolution: Option<(u32, u32)>,
    pub verify: Verify,
    /// Join this multiplayer server once the game has started.
    pub join_server: Option<crate::servers::ServerAddress>,
}

#[derive(Debug, Clone)]
pub struct PreparedLaunch {
    pub version_id: String,
    pub java: JavaInstall,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Command line with secrets replaced, safe to log or display.
    pub masked_command: String,
    /// Values masked in game output (access token).
    pub secrets: Vec<String>,
    /// Files to delete once the game exits (argfiles).
    pub temp_files: Vec<PathBuf>,
}

pub async fn prepare(
    ctx: &Ctx,
    opts: &LaunchOptions,
    task: &str,
    cancel: &CancellationToken,
) -> Result<PreparedLaunch> {
    let meta = Progress::new(ctx.events.clone(), task, Stage::Metadata);
    meta.set_totals(1, 0);
    let resolved = version::resolve(ctx, &opts.version_id).await?;
    let v = &resolved.json;
    meta.finish();
    tracing::info!(version = %v.id, chain = ?resolved.chain, java = v.java_major(), "version resolved");

    let mut env = RuleEnv::new(ctx.os.clone())
        .with_feature("is_demo_user", false)
        .with_feature("has_custom_resolution", opts.resolution.is_some());
    for f in [
        "has_quick_plays_support",
        "is_quick_play_singleplayer",
        "is_quick_play_multiplayer",
        "is_quick_play_realms",
    ] {
        env = env.with_feature(f, false);
    }

    // --- libraries, client jar, logging config ---
    let libs = library::resolve(&v.libraries, &env, &ctx.paths.libraries());
    let jar_path = ctx
        .paths
        .versions()
        .join(&resolved.jar_id)
        .join(format!("{}.jar", resolved.jar_id));
    let mut items = libs.download_items();
    let client = v.downloads.as_ref().and_then(|d| d.client.clone());
    match &client {
        Some(c) => items.push(DownloadItem {
            url: c.url.clone(),
            dest: jar_path.clone(),
            checksum: c.sha1.clone().map(Checksum::Sha1),
            size: c.size,
        }),
        None if !jar_path.is_file() => {
            return Err(CoreError::InvalidVersion {
                id: v.id.clone(),
                reason: "no client jar".into(),
            });
        }
        None => {}
    }
    let log_cfg = v.logging.as_ref().and_then(|l| l.client.clone());
    let log_cfg_path = log_cfg
        .as_ref()
        .map(|l| ctx.paths.assets().join("log_configs").join(&l.file.id));
    if let (Some(l), Some(p)) = (&log_cfg, &log_cfg_path) {
        items.push(DownloadItem {
            url: l.file.url.clone(),
            dest: p.clone(),
            checksum: l.file.sha1.clone().map(Checksum::Sha1),
            size: l.file.size,
        });
    }
    if let Some(index_ref) = &v.asset_index {
        items.push(assets::index_item(&ctx.paths, index_ref));
    }
    let lib_progress = Progress::new(ctx.events.clone(), task, Stage::Libraries);
    let stats = ctx
        .downloader()
        .run(items, opts.verify, &lib_progress, cancel)
        .await?;
    tracing::info!(?stats, "libraries ready");

    // Installer-generated libraries (no URL) must exist locally.
    if let Some(missing) = libs
        .classpath
        .iter()
        .find(|a| a.url.is_none() && !a.path.is_file())
    {
        return Err(CoreError::io(
            &missing.path,
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "library missing; reinstall the loader",
            ),
        ));
    }

    // --- natives ---
    let natives_dir = ctx.paths.versions().join(&v.id).join("natives");
    let native_progress = Progress::new(ctx.events.clone(), task, Stage::Natives);
    native_progress.set_totals(libs.natives.len() as u64, 0);
    let jars = libs.natives.clone();
    let nd = natives_dir.clone();
    tokio::task::spawn_blocking(move || natives::extract_all(&jars, &nd))
        .await
        .expect("natives task panicked")?;
    native_progress.finish();

    // --- assets ---
    std::fs::create_dir_all(&opts.game_dir).map_err(|e| CoreError::io(&opts.game_dir, e))?;
    let assets_id = v.assets_id();
    let layout = match &v.asset_index {
        Some(r) => {
            let index = assets::read_index(&ctx.paths, &r.id)?;
            let objects = assets::object_items(ctx, &index);
            let asset_progress = Progress::new(ctx.events.clone(), task, Stage::Assets);
            let s = ctx
                .downloader()
                .run(objects, opts.verify, &asset_progress, cancel)
                .await?;
            tracing::info!(?s, "assets ready");
            let layout = assets::layout(&ctx.paths, &r.id, &index, &opts.game_dir);
            let (p, i, l) = (ctx.paths.clone(), index, layout.clone());
            tokio::task::spawn_blocking(move || assets::materialize(&p, &i, &l))
                .await
                .expect("assets task panicked")?;
            layout
        }
        None => assets::AssetLayout {
            assets_root: ctx.paths.assets(),
            game_assets: ctx.paths.assets(),
        },
    };

    // --- java ---
    let java_progress = Progress::new(ctx.events.clone(), task, Stage::Java);
    java_progress.set_totals(1, 0);
    let java = java::resolve(
        ctx,
        v.java_major(),
        opts.java_path.as_deref(),
        opts.auto_download_java,
        cancel,
    )
    .await?;
    java_progress.finish();

    // --- command line ---
    let sep = ctx.os.classpath_separator();
    let mut cp: Vec<String> = libs
        .classpath
        .iter()
        .map(|a| a.path.display().to_string())
        .collect();
    cp.push(jar_path.display().to_string());
    let classpath = cp.join(sep);

    let a = &opts.account;
    let (width, height) = opts.resolution.unwrap_or((854, 480));
    let mut vars = args::Vars::new();
    let path_str = |p: &Path| p.display().to_string();
    for (k, val) in [
        ("auth_player_name", a.name.clone()),
        ("auth_uuid", a.uuid_simple()),
        ("auth_access_token", a.access_token.clone()),
        ("auth_session", a.session()),
        ("auth_xuid", a.xuid.clone()),
        ("clientid", a.client_id.clone()),
        ("user_type", a.user_type.clone()),
        ("user_properties", "{}".into()),
        ("version_name", v.id.clone()),
        (
            "version_type",
            v.kind.clone().unwrap_or_else(|| "release".into()),
        ),
        ("game_directory", path_str(&opts.game_dir)),
        ("assets_root", path_str(&layout.assets_root)),
        ("game_assets", path_str(&layout.game_assets)),
        ("assets_index_name", assets_id.clone()),
        ("library_directory", path_str(&ctx.paths.libraries())),
        ("classpath_separator", sep.into()),
        ("natives_directory", path_str(&natives_dir)),
        ("launcher_name", crate::LAUNCHER_NAME.into()),
        ("launcher_version", crate::LAUNCHER_VERSION.into()),
        ("classpath", classpath),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
    ] {
        vars.insert(k, val);
    }

    let (mut jvm, game) = args::build(v, &env, &vars);
    fix_ignore_list(&mut jvm, &resolved.jar_id);
    let mut full = Vec::new();
    let max = opts.max_memory_mb.max(512);
    let min = opts.min_memory_mb.unwrap_or(max.min(1024)).min(max);
    full.push(format!("-Xms{min}M"));
    full.push(format!("-Xmx{max}M"));
    full.extend(opts.extra_jvm_args.iter().cloned());
    full.extend(jvm);
    if let (Some(l), Some(p)) = (&log_cfg, &log_cfg_path) {
        let mut lv = args::Vars::new();
        lv.insert("path", path_str(p));
        full.push(args::substitute(&l.argument, &lv));
    }
    full.push(v.main_class.clone().expect("checked in resolve"));
    full.extend(game);
    // Legacy versions have no rule-gated resolution arguments; 1.6+ still
    // understand --width/--height.
    if v.arguments.is_none()
        && let Some((w, h)) = opts.resolution
    {
        full.extend([
            "--width".into(),
            w.to_string(),
            "--height".into(),
            h.to_string(),
        ]);
    }
    full.extend(opts.extra_game_args.iter().cloned());
    if let Some(server) = &opts.join_server {
        tracing::info!(%server, "joining server after start");
        full.extend(args::join_server(v, server));
    }

    let left = args::unresolved(&full);
    if !left.is_empty() {
        tracing::warn!(?left, "unresolved placeholders in launch arguments");
    }

    let program = java.executable_path();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let argfile = ctx
        .paths
        .cache()
        .join("launch")
        .join(format!("{}-{stamp}.args", sanitize(&v.id)));
    let temp_files = cmdline::fit(&program, &mut full, java.major, &argfile)?
        .into_iter()
        .collect();

    let secrets = if a.offline {
        vec![]
    } else {
        vec![a.access_token.clone()]
    };
    let masked_command = mask_command(&program, &full, &secrets);

    Ok(PreparedLaunch {
        version_id: v.id.clone(),
        java,
        program,
        args: full,
        cwd: opts.game_dir.clone(),
        masked_command,
        secrets,
        temp_files,
    })
}

/// Modern Forge keeps the vanilla jar off its module layer via
/// `-DignoreList=…,${version_name}.jar`, assuming the official launcher's
/// layout where the client jar is copied to `versions/<loader id>/`. We share
/// `versions/<mc>/<mc>.jar` instead, so its file name is added explicitly.
fn fix_ignore_list(jvm: &mut [String], jar_id: &str) {
    let jar = format!("{jar_id}.jar");
    for a in jvm.iter_mut() {
        if a.starts_with("-DignoreList=") && !a.split(',').any(|x| x == jar) {
            a.push(',');
            a.push_str(&jar);
        }
    }
}

/// Makes sure the vanilla version JSON and client jar of `mc` are present
/// (loader installers patch the client jar). Returns the resolved version
/// and the jar path.
pub async fn ensure_client(
    ctx: &Ctx,
    mc: &str,
    progress: &Progress,
    cancel: &CancellationToken,
) -> Result<(version::ResolvedVersion, PathBuf)> {
    let resolved = version::resolve(ctx, mc).await?;
    let jar = ctx
        .paths
        .versions()
        .join(&resolved.jar_id)
        .join(format!("{}.jar", resolved.jar_id));
    if let Some(c) = resolved
        .json
        .downloads
        .as_ref()
        .and_then(|d| d.client.clone())
    {
        ctx.downloader()
            .run(
                vec![DownloadItem {
                    url: c.url,
                    dest: jar.clone(),
                    checksum: c.sha1.map(Checksum::Sha1),
                    size: c.size,
                }],
                Verify::Quick,
                progress,
                cancel,
            )
            .await?;
    }
    if !jar.is_file() {
        return Err(CoreError::InvalidVersion {
            id: mc.to_owned(),
            reason: "no client jar".into(),
        });
    }
    Ok((resolved, jar))
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn mask_command(program: &Path, args: &[String], secrets: &[String]) -> String {
    let mut out = program.display().to_string();
    let mut mask_next = false;
    for a in args {
        out.push(' ');
        let shown = if mask_next
            || secrets
                .iter()
                .any(|s| !s.is_empty() && a.contains(s.as_str()))
        {
            "***".to_owned()
        } else if a.len() > 300 {
            format!(
                "{}…({} chars)",
                a.chars().take(120).collect::<String>(),
                a.len()
            )
        } else {
            a.clone()
        };
        mask_next = a == "--accessToken";
        out.push_str(&shown);
    }
    crate::logging::mask_secrets(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_list_gets_shared_jar() {
        let mut jvm = vec![
            "-DignoreList=asm,client-extra,forge-,forge-1.20.1-47.4.26.jar".to_owned(),
            "-Dx=1".to_owned(),
        ];
        fix_ignore_list(&mut jvm, "1.20.1");
        assert_eq!(
            jvm[0],
            "-DignoreList=asm,client-extra,forge-,forge-1.20.1-47.4.26.jar,1.20.1.jar"
        );
        fix_ignore_list(&mut jvm, "1.20.1");
        assert!(jvm[0].ends_with(",1.20.1.jar") && !jvm[0].ends_with("1.20.1.jar,1.20.1.jar"));
        assert_eq!(jvm[1], "-Dx=1");
    }

    #[test]
    fn command_masking() {
        let s = mask_command(
            Path::new("java"),
            &[
                "--accessToken".into(),
                "eyJsecret".into(),
                "--username".into(),
                "Steve".into(),
            ],
            &["eyJsecret".into()],
        );
        assert_eq!(s, "java --accessToken *** --username Steve");
    }
}
