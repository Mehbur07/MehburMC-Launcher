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
use launcher_core::launch::{self, LaunchOptions, process};
use launcher_core::net::download::Verify;
use launcher_core::{Ctx, DataMode, Paths, Settings, java, version};
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
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
    },
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
            CoreEvent::Task { .. } => {}
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
        Command::Launch {
            version,
            offline,
            memory,
            game_dir,
            java,
            verify,
            dry_run,
            exit_when_ready,
            width,
            height,
        } => {
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
            let game_dir =
                game_dir.unwrap_or_else(|| ctx.paths.instances().join(format!("cli-{version_id}")));
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
            let exit = process::run(&prepared, ctx.events.clone(), &version_id, &cancel)
                .await
                .map_err(core_err)?;
            if exit_when_ready.is_some() {
                if sink.ready.load(Ordering::SeqCst) {
                    eprintln!(">>> SUCCESS: {version_id} reached the main menu");
                } else {
                    return Err(anyhow!("{version_id} exited before reaching the main menu"));
                }
            } else if exit.code != Some(0) && !exit.killed {
                return Err(anyhow!("game exited with code {:?}", exit.code));
            }
        }
        Command::Paths | Command::Init | Command::Settings => unreachable!(),
    }
    Ok(())
}
