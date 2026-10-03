//! `mehbur-cli` — exercises launcher-core without the UI.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use launcher_core::{DataMode, Paths, Settings};

#[derive(Parser)]
#[command(name = "mehbur-cli", version, about = "MehburMC Launcher core CLI")]
struct Cli {
    /// Use this MehburMC root instead of the platform data directory.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = match &cli.root {
        Some(root) => Paths::from_root(root.clone(), DataMode::Standard),
        None => Paths::resolve(None),
    }
    .map_err(|e| anyhow::anyhow!(e.detail()))?;

    match cli.command {
        Command::Paths => println!("{}", serde_json::to_string_pretty(&paths.info())?),
        Command::Init => {
            paths
                .ensure_layout()
                .map_err(|e| anyhow::anyhow!(e.detail()))
                .context("creating data folders")?;
            println!("layout ready at {}", paths.mc().display());
        }
        Command::Settings => {
            let s = Settings::load(&paths).map_err(|e| anyhow::anyhow!(e.detail()))?;
            println!("{}", serde_json::to_string_pretty(&s)?);
        }
    }
    Ok(())
}
