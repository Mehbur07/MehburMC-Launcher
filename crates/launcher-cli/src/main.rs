//! `mehbur-cli` — exercises launcher-core without the UI.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use launcher_core::auth::LaunchAccount;
use launcher_core::events::{CoreEvent, EventSink, LogStream};
use launcher_core::instance::{LoaderKind, LoaderSpec};
use launcher_core::launch::{self, LaunchOptions, process};
use launcher_core::net::download::Verify;
use launcher_core::{Ctx, DataMode, Paths, Settings, java, loader, version};
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(name = "mehbur-cli", version, about = "MehburMC Launcher core CLI")]
struct Cli {
    /// Use this MehburMC root instead of the platform data directory.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Write launcher logs to stderr as well.
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print resolved data paths as JSON.
    Paths,
    /// Create the data folder layout (MehburMC/game/mc/...).
    Init,
    /// Print the effective settings as JSON.
    Settings,
    /// List Minecraft versions from the Mojang manifest.
    Versions {
        /// release | snapshot | old_beta | old_alpha | all
        #[arg(long, default_value = "release")]
        r#type: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Java runtimes.
    Java {
        #[command(subcommand)]
        action: JavaAction,
    },
    /// Modpacks (.mrpack / CurseForge .zip).
    Modpack {
        #[command(subcommand)]
        action: ModpackAction,
    },
    /// Modrinth content.
    Content {
        #[command(subcommand)]
        action: ContentAction,
    },
    /// Mod loaders.
    Loader {
        #[command(subcommand)]
        action: LoaderAction,
    },
    /// Ping a multiplayer server (status, players, latency) and print JSON.
    Ping {
        /// host[:port]
        address: String,
    },
    /// Download (if needed) and launch a version with an offline account.
    Launch {
        /// Version id, e.g. 26.3, 1.12.2 (default: latest release).
        version: Option<String>,
        /// Offline player name.
        #[arg(long, default_value = "Player")]
        offline: String,
        /// Max memory in MB (default: settings.defaultMemoryMb).
        #[arg(long)]
        memory: Option<u32>,
        /// Game directory (default: instances/cli-<version>).
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Explicit java executable.
        #[arg(long)]
        java: Option<PathBuf>,
        /// Re-hash every existing file (repair).
        #[arg(long)]
        verify: bool,
        /// Only prepare and print the (masked) command line.
        #[arg(long)]
        dry_run: bool,
        /// Stop the game N seconds after the main menu is reached (testing).
        #[arg(long)]
        exit_when_ready: Option<u64>,
        /// Mod loader: fabric, quilt, legacy-fabric, forge, neoforge or
        /// optifine, optionally with a version (`fabric:0.19.5`).
        #[arg(long)]
        loader: Option<String>,
        /// Reinstall the loader even if it is already present.
        #[arg(long)]
        reinstall: bool,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        /// Join this server (host[:port]) once the game has started.
        #[arg(long)]
        server: Option<String>,
    },
}

#[derive(Subcommand)]
enum ModpackAction {
    /// Import a pack as a new instance (CurseForge needs settings.curseforgeApiKey).
    Import { file: PathBuf },
}

#[derive(Subcommand)]
enum ContentAction {
    /// Search Modrinth.
    Search {
        query: String,
        /// mod | modpack | resourcepack | shader
        #[arg(long, default_value = "mod")]
        r#type: String,
        #[arg(long)]
        mc: Option<String>,
        #[arg(long)]
        loader: Option<String>,
    },
    /// List an instance's content with Modrinth metadata.
    Scan {
        instance: String,
        #[arg(long)]
        updates: bool,
    },
    /// Install a Modrinth project into an instance.
    Install { instance: String, project: String },
}

#[derive(Subcommand)]
enum LoaderAction {
    /// List loader versions for a Minecraft version.
    List {
        /// fabric | quilt | legacy-fabric | forge | neoforge | optifine
        kind: String,
        mc: String,
        #[arg(long, default_value_t = 15)]
        limit: usize,
    },
    /// Import a user-downloaded OptiFine jar.
    ImportOptifine { jar: PathBuf },
    /// Install Iris + Sodium (or Oculus + Embeddium) into a mods folder.
    Shaders {
        mc: String,
        loader: String,
        /// Game directory (mods go into its `mods/`).
        dir: PathBuf,
    },
}

fn parse_loader(s: &str) -> Result<LoaderSpec> {
    let (kind, version) = match s.split_once(':') {
        Some((k, v)) => (k, Some(v.to_owned())),
        None => (s, None),
    };
    let kind = match kind.to_ascii_lowercase().as_str() {
        "vanilla" => LoaderKind::Vanilla,
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "legacy-fabric" | "legacyfabric" => LoaderKind::LegacyFabric,
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        "optifine" => LoaderKind::Optifine,
        other => return Err(anyhow!("unknown loader {other}")),
    };
    Ok(LoaderSpec { kind, version })
}

#[derive(Subcommand)]
enum JavaAction {
    /// List managed and system Java installations.
    List,
    /// Download a Temurin JRE into runtime/java<major>.
    Install { major: u32 },
}

/// Prints progress on one line and game output as-is.
struct CliSink {
    ready: AtomicBool,
}

impl EventSink for CliSink {
    fn emit(&self, event: CoreEvent) {
        match event {
            CoreEvent::Progress {
                stage,
                done,
                total,
                bytes_done,
                bytes_total,
                ..
            } => {
                let mb = |b: u64| b as f64 / 1_048_576.0;
                eprint!(
                    "\r[{stage:?}] {done}/{total} files  {:.1}/{:.1} MB        ",
                    mb(bytes_done),
                    mb(bytes_total)
                );
                if total > 0 && done >= total {
                    eprintln!();
                }
                let _ = std::io::stderr().flush();
            }
            CoreEvent::GameLog { stream, line, .. } => {
                if process::is_ready_line(&line) && !self.ready.swap(true, Ordering::SeqCst) {
                    eprintln!(">>> MAIN MENU READY <<<");
                }
                match stream {
                    LogStream::Stdout => println!("{line}"),
                    LogStream::Stderr => eprintln!("{line}"),
                }
            }
            CoreEvent::Task { .. } | CoreEvent::GameCrashed { .. } => {}
            CoreEvent::GameExited {
                code, crash_report, ..
            } => {
                eprintln!(">>> game exited with code {code:?}");
                if let Some(c) = crash_report {
                    eprintln!(">>> crash report: {c}");
                }
            }
        }
    }
}

fn print_crash(info: &launcher_core::crash::CrashInfo) {
    if let Some(s) = &info.summary {
        eprintln!(">>> crash: {s}");
    }
    for d in &info.diagnoses {
        eprintln!(
            ">>> diagnosis: {:?} {}",
            d.kind,
            d.detail.as_deref().unwrap_or("")
        );
    }
}

fn core_err(e: launcher_core::CoreError) -> anyhow::Error {
    anyhow!("[{}] {}", e.code(), e.detail())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = match &cli.root {
        Some(root) => Paths::from_root(root.clone(), DataMode::Standard),
        None => Paths::resolve(None),
    }
    .map_err(core_err)?;

    match cli.command {
        Command::Paths => println!("{}", serde_json::to_string_pretty(&paths.info())?),
        Command::Init => {
            paths.ensure_layout().map_err(core_err)?;
            println!("layout ready at {}", paths.mc().display());
        }
        Command::Settings => {
            let s = Settings::load(&paths).map_err(core_err)?;
            println!("{}", serde_json::to_string_pretty(&s)?);
        }
        command => {
            paths.ensure_layout().map_err(core_err)?;
            let settings = Settings::load(&paths).map_err(core_err)?;
            let _guard =
                launcher_core::logging::init(&paths.logs(), cli.verbose || settings.debug_logging)?;
            let sink = Arc::new(CliSink {
                ready: AtomicBool::new(false),
            });
            let ctx = Ctx::new(paths, sink.clone(), settings.download_concurrency as usize)
                .map_err(core_err)?;
            run(command, &ctx, &settings, sink).await?;
        }
    }
    Ok(())
}

async fn run(command: Command, ctx: &Ctx, settings: &Settings, sink: Arc<CliSink>) -> Result<()> {
    let cancel = CancellationToken::new();
    {
        let c = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                eprintln!("\ncancelling…");
                c.cancel();
            }
        });
    }

    match command {
        Command::Versions { r#type, limit } => {
            let m = version::manifest(ctx).await.map_err(core_err)?;
            println!(
                "latest release: {}  snapshot: {}",
                m.latest.release, m.latest.snapshot
            );
            for v in m
                .versions
                .iter()
                .filter(|v| r#type == "all" || v.kind == r#type)
                .take(limit)
            {
                println!("{:<24} {:<10} {}", v.id, v.kind, v.release_time);
            }
        }
        Command::Java { action } => match action {
            JavaAction::List => {
                for j in java::managed(ctx).into_iter().chain(java::scan_system()) {
                    println!(
                        "java{:<3} {:<14} {:<8} {:<22} {}",
                        j.major,
                        j.version,
                        if j.managed { "managed" } else { "system" },
                        j.vendor.unwrap_or_default(),
                        j.executable
                    );
                }
            }
            JavaAction::Install { major } => {
                let j = java::adoptium::install(ctx, major, &cancel)
                    .await
                    .map_err(core_err)?;
                println!("installed Java {} at {}", j.version, j.home);
            }
        },
        Command::Modpack { action } => match action {
            ModpackAction::Import { file } => {
                use launcher_core::content::modpack;
                let store = launcher_core::instance::InstanceStore::new(ctx.paths.clone());
                let progress = launcher_core::events::Progress::new(
                    ctx.events.clone(),
                    "modpack",
                    launcher_core::events::Stage::Content,
                );
                let r = match modpack::detect(&file).map_err(core_err)? {
                    modpack::PackKind::Modrinth => {
                        modpack::import_mrpack(ctx, &store, &file, &progress, &cancel).await
                    }
                    modpack::PackKind::CurseForge => {
                        modpack::import_curseforge(
                            ctx,
                            &store,
                            &file,
                            settings.curseforge_api_key.as_deref(),
                            &progress,
                            &cancel,
                        )
                        .await
                    }
                }
                .map_err(core_err)?;
                println!(
                    "instance {} ({}): mc {} loader {:?} {}",
                    r.instance.id,
                    r.instance.name,
                    r.instance.mc_version,
                    r.instance.loader.kind,
                    r.instance.loader.version.as_deref().unwrap_or("-")
                );
                println!(
                    "dir {}",
                    store.dir(&r.instance.id).map_err(core_err)?.display()
                );
                for b in r.blocked {
                    println!("manual download: {} -> {} ({})", b.name, b.folder, b.url);
                }
            }
        },
        Command::Content { action } => match action {
            ContentAction::Search {
                query,
                r#type,
                mc,
                loader,
            } => {
                use launcher_core::content::modrinth::{self, ProjectType, SearchQuery};
                let project_type = match r#type.as_str() {
                    "modpack" => ProjectType::Modpack,
                    "resourcepack" => ProjectType::Resourcepack,
                    "shader" => ProjectType::Shader,
                    _ => ProjectType::Mod,
                };
                let page = modrinth::search(
                    ctx,
                    &SearchQuery {
                        query,
                        project_type,
                        game_version: mc,
                        loader,
                        sort: Default::default(),
                        offset: 0,
                    },
                )
                .await
                .map_err(core_err)?;
                println!("{} results", page.total_hits);
                for h in page.hits.iter().take(10) {
                    println!("{:<24} {:<32} {:>12}", h.slug, h.title, h.downloads);
                }
            }
            ContentAction::Scan { instance, updates } => {
                let store = launcher_core::instance::InstanceStore::new(ctx.paths.clone());
                let inst = store.get(&instance).map_err(core_err)?;
                let dir = store.dir(&inst.id).map_err(core_err)?;
                let items = launcher_core::content::installed::scan(
                    ctx,
                    &inst,
                    &dir,
                    launcher_core::instance::files::Folder::Mods,
                    updates,
                )
                .await
                .map_err(core_err)?;
                for i in items {
                    println!(
                        "{:<48} {:<28} {:<20} {}",
                        i.file_name,
                        i.title.unwrap_or_else(|| "?".into()),
                        i.version_number.unwrap_or_default(),
                        i.update
                            .map(|u| format!("-> {}", u.version_number))
                            .unwrap_or_default()
                    );
                }
            }
            ContentAction::Install { instance, project } => {
                use launcher_core::content::install::{self, InstallRequest};
                use launcher_core::content::modrinth::ProjectType;
                let store = launcher_core::instance::InstanceStore::new(ctx.paths.clone());
                let inst = store.get(&instance).map_err(core_err)?;
                let dir = store.dir(&inst.id).map_err(core_err)?;
                let r = install::install(
                    ctx,
                    &inst,
                    &dir,
                    &[InstallRequest {
                        project,
                        project_type: ProjectType::Mod,
                        version_id: None,
                    }],
                    &install::quiet_progress(),
                    &cancel,
                )
                .await
                .map_err(core_err)?;
                println!(
                    "installed {:?}, already present {:?}, incompatible {:?}",
                    r.installed, r.already_present, r.incompatible
                );
            }
        },
        Command::Ping { address } => {
            let a = launcher_core::servers::ServerAddress::parse(&address).map_err(core_err)?;
            let status = launcher_core::servers::ping::ping(&a)
                .await
                .map_err(core_err)?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::Loader { action } => match action {
            LoaderAction::List { kind, mc, limit } => {
                let spec = parse_loader(&kind)?;
                let list = loader::list_versions(ctx, spec.kind, &mc)
                    .await
                    .map_err(core_err)?;
                if list.is_empty() {
                    println!("no {kind} versions for {mc}");
                }
                for v in list.iter().take(limit) {
                    println!(
                        "{:<28} {:<20} {}{}",
                        v.id,
                        v.label,
                        if v.stable { "stable" } else { "unstable" },
                        if v.recommended { "  (recommended)" } else { "" }
                    );
                }
            }
            LoaderAction::Shaders { mc, loader, dir } => {
                let spec = parse_loader(&loader)?;
                let inst: launcher_core::instance::Instance =
                    serde_json::from_value(serde_json::json!({
                        "name": "cli", "mcVersion": mc, "loader": spec
                    }))?;
                let r = launcher_core::content::shaders::install(ctx, &inst, &dir, &cancel)
                    .await
                    .map_err(core_err)?;
                println!(
                    "installed: {:?}
already present: {:?}",
                    r.installed, r.already_present
                );
            }
            LoaderAction::ImportOptifine { jar } => {
                let info = loader::optifine::import(&ctx.paths, &jar).map_err(core_err)?;
                println!("imported OptiFine {} {}", info.mc_version, info.edition);
            }
        },
        Command::Launch {
            version,
            loader: loader_arg,
            reinstall,
            offline,
            memory,
            game_dir,
            java,
            verify,
            dry_run,
            exit_when_ready,
            width,
            height,
            server,
        } => {
            let join_server = server
                .as_deref()
                .map(launcher_core::servers::ServerAddress::parse)
                .transpose()
                .map_err(core_err)?;
            let version_id = match version {
                Some(v) => v,
                None => {
                    version::manifest(ctx)
                        .await
                        .map_err(core_err)?
                        .latest
                        .release
                }
            };
            let account = LaunchAccount::offline(&offline).map_err(core_err)?;
            let spec = match &loader_arg {
                Some(l) => parse_loader(l)?,
                None => LoaderSpec::default(),
            };
            let started = std::time::Instant::now();
            let installed =
                loader::ensure_installed(ctx, &version_id, &spec, reinstall, "cli", &cancel)
                    .await
                    .map_err(core_err)?;
            if spec.kind != LoaderKind::Vanilla {
                eprintln!(
                    "loader ready: {} in {:.1}s",
                    installed.version_id,
                    started.elapsed().as_secs_f64()
                );
            }
            let game_dir = game_dir.unwrap_or_else(|| {
                ctx.paths
                    .instances()
                    .join(format!("cli-{}", installed.version_id))
            });
            let version_id = installed.version_id;
            let opts = LaunchOptions {
                version_id: version_id.clone(),
                game_dir,
                account,
                max_memory_mb: memory.unwrap_or(settings.default_memory_mb),
                min_memory_mb: None,
                java_path: java,
                auto_download_java: true,
                extra_jvm_args: vec![],
                extra_game_args: vec![],
                resolution: width.zip(height),
                verify: if verify { Verify::Full } else { Verify::Quick },
                join_server,
            };
            let started = std::time::Instant::now();
            let prepared = launch::prepare(ctx, &opts, &version_id, &cancel)
                .await
                .map_err(core_err)?;
            eprintln!(
                "prepared {} in {:.1}s with Java {} ({})",
                prepared.version_id,
                started.elapsed().as_secs_f64(),
                prepared.java.version,
                prepared.java.executable
            );
            if dry_run {
                println!("{}", prepared.masked_command);
                return Ok(());
            }

            if let Some(secs) = exit_when_ready {
                let (s, c) = (sink.clone(), cancel.clone());
                tokio::spawn(async move {
                    while !s.ready.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                    tokio::time::sleep(Duration::from_secs(secs)).await;
                    eprintln!(">>> stopping game (--exit-when-ready)");
                    c.cancel();
                });
            }
            let mut exit = process::run(&prepared, ctx.events.clone(), &version_id, &cancel)
                .await
                .map_err(core_err)?;
            if loader::early_window::looks_like_early_window_crash(spec.kind, &exit)
                && loader::early_window::disable(&opts.game_dir).map_err(core_err)?
            {
                eprintln!(
                    ">>> early loading window crashed; retrying with earlyWindowControl=false"
                );
                let again = launch::prepare(ctx, &opts, &version_id, &cancel)
                    .await
                    .map_err(core_err)?;
                exit = process::run(&again, ctx.events.clone(), &version_id, &cancel)
                    .await
                    .map_err(core_err)?;
            }
            if exit_when_ready.is_some() {
                if sink.ready.load(Ordering::SeqCst) {
                    eprintln!(">>> SUCCESS: {version_id} reached the main menu");
                } else {
                    return Err(anyhow!("{version_id} exited before reaching the main menu"));
                }
            } else if exit.code != Some(0) && !exit.killed {
                let info = launcher_core::crash::analyze(
                    &version_id,
                    &version_id,
                    launcher_core::crash::Inputs {
                        game_dir: &opts.game_dir,
                        exit_code: exit.code,
                        crash_report: exit.crash_report.as_deref(),
                        output: &exit.output_tail,
                        started: exit.started_at,
                    },
                );
                print_crash(&info);
                return Err(anyhow!("game exited with code {:?}", exit.code));
            }
        }
        Command::Paths | Command::Init | Command::Settings => unreachable!(),
    }
    Ok(())
}
