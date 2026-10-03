//! Spawns the game, streams its output as events and reports how it exited.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::PreparedLaunch;
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventSink, LogStream};
use crate::logging::mask_secrets;

#[derive(Debug, Clone)]
pub struct GameExit {
    pub code: Option<i32>,
    /// Newest crash report written during this session, if any.
    pub crash_report: Option<PathBuf>,
    pub duration: Duration,
    /// Exit was triggered by cancellation (launcher-side kill).
    pub killed: bool,
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Runs the prepared game to completion. Cancelling `cancel` kills it.
pub async fn run(
    prepared: &PreparedLaunch,
    events: Arc<dyn EventSink>,
    task: &str,
    cancel: &CancellationToken,
) -> Result<GameExit> {
    let mut cmd = Command::new(&prepared.program);
    cmd.args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    tracing::info!(command = %prepared.masked_command, "starting game");
    let started_wall = SystemTime::now();
    let started = std::time::Instant::now();
    let mut child = cmd.spawn().map_err(|source| CoreError::Spawn {
        program: prepared.program.clone(),
        source,
    })?;

    let secrets = Arc::new(prepared.secrets.clone());
    let out = child.stdout.take().map(|s| {
        tokio::spawn(pump(
            s,
            LogStream::Stdout,
            events.clone(),
            task.to_owned(),
            secrets.clone(),
        ))
    });
    let err = child.stderr.take().map(|s| {
        tokio::spawn(pump(
            s,
            LogStream::Stderr,
            events.clone(),
            task.to_owned(),
            secrets.clone(),
        ))
    });

    let mut killed = false;
    let status = tokio::select! {
        s = child.wait() => s,
        _ = cancel.cancelled() => {
            killed = true;
            let _ = child.kill().await;
            child.wait().await
        }
    }
    .map_err(|e| CoreError::io(&prepared.program, e))?;

    for h in [out, err].into_iter().flatten() {
        let _ = h.await;
    }
    for f in &prepared.temp_files {
        let _ = std::fs::remove_file(f);
    }

    let code = status.code();
    let crash_report = if !killed && code != Some(0) {
        newest_crash_report(&prepared.cwd.join("crash-reports"), started_wall)
    } else {
        None
    };
    let exit = GameExit {
        code,
        crash_report,
        duration: started.elapsed(),
        killed,
    };
    tracing::info!(?exit.code, killed, secs = exit.duration.as_secs(), "game exited");
    events.emit(CoreEvent::GameExited {
        task: task.to_owned(),
        code,
        crash_report: exit.crash_report.as_ref().map(|p| p.display().to_string()),
    });
    Ok(exit)
}

async fn pump(
    stream: impl AsyncRead + Unpin,
    which: LogStream,
    events: Arc<dyn EventSink>,
    task: String,
    secrets: Arc<Vec<String>>,
) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                // The game may not emit UTF-8 (e.g. legacy code pages).
                let mut line = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\r', '\n'])
                    .to_owned();
                for s in secrets.iter().filter(|s| s.len() > 3) {
                    line = line.replace(s.as_str(), "***");
                }
                events.emit(CoreEvent::GameLog {
                    task: task.clone(),
                    stream: which,
                    line: mask_secrets(&line),
                });
            }
        }
    }
}

fn newest_crash_report(dir: &Path, since: SystemTime) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            (modified >= since).then(|| (modified, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Log lines that indicate the client finished loading (main menu reached).
pub fn is_ready_line(line: &str) -> bool {
    line.contains("Sound engine started")
        || line.contains("Starting up SoundSystem")
        || (line.contains("Created: ") && line.contains("atlas"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_patterns() {
        assert!(is_ready_line(
            "[12:00:01] [Render thread/INFO]: Sound engine started"
        ));
        assert!(is_ready_line(
            "[Render thread/INFO]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas"
        ));
        assert!(!is_ready_line("[main/INFO]: Loading Minecraft"));
    }

    #[test]
    fn crash_report_detection() {
        let dir = tempfile::tempdir().unwrap();
        let since = SystemTime::now() - Duration::from_secs(5);
        assert!(newest_crash_report(dir.path(), since).is_none());
        std::fs::write(dir.path().join("crash-2026-10-03_12.00.00-client.txt"), "x").unwrap();
        assert!(newest_crash_report(dir.path(), since).is_some());
    }
}
