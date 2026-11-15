//! Spawns the game, streams its output as events and reports how it exited.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::PreparedLaunch;
use super::log4j::{Log4jParser, ParsedLine};
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
    /// Last lines the game printed (for crash analysis).
    pub output_tail: String,
    pub started_at: SystemTime,
}

/// Lines of game output kept in memory for crash analysis.
const TAIL_LINES: usize = 400;

/// Optional callbacks while the game runs.
#[derive(Default)]
pub struct RunHooks {
    /// Called once when the client reaches the main menu ([`is_ready_line`]).
    pub on_ready: Option<Box<dyn FnOnce() + Send>>,
    /// Send stdout + stderr to this file instead of pipes and follow it.
    /// Used when the launcher quits while the game keeps running: a pipe
    /// whose reader is gone can block the game's logging thread.
    pub output_file: Option<PathBuf>,
}

/// How often a followed output file is polled for new lines.
const FOLLOW_INTERVAL: Duration = Duration::from_millis(100);

type Shared<T> = Arc<Mutex<T>>;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Runs the prepared game to completion. Cancelling `cancel` kills it.
pub async fn run(
    prepared: &PreparedLaunch,
    events: Arc<dyn EventSink>,
    task: &str,
    cancel: &CancellationToken,
) -> Result<GameExit> {
    run_with(prepared, events, task, cancel, RunHooks::default()).await
}

pub async fn run_with(
    prepared: &PreparedLaunch,
    events: Arc<dyn EventSink>,
    task: &str,
    cancel: &CancellationToken,
    hooks: RunHooks,
) -> Result<GameExit> {
    let mut cmd = Command::new(&prepared.program);
    cmd.args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdin(Stdio::null());
    match &hooks.output_file {
        Some(path) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
            }
            let f = std::fs::File::create(path).map_err(|e| CoreError::io(path, e))?;
            let f2 = f.try_clone().map_err(|e| CoreError::io(path, e))?;
            // The game must outlive the launcher here.
            cmd.stdout(Stdio::from(f))
                .stderr(Stdio::from(f2))
                .kill_on_drop(false);
        }
        None => {
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
        }
    }
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
    let tail: Shared<VecDeque<String>> = Arc::new(Mutex::new(VecDeque::with_capacity(TAIL_LINES)));
    let on_ready = Arc::new(Mutex::new(hooks.on_ready));
    let exited = Arc::new(AtomicBool::new(false));
    let spawn_pump = |stream, which, follow: Option<Arc<AtomicBool>>| {
        tokio::spawn(pump(
            stream,
            Pump {
                which,
                events: events.clone(),
                task: task.to_owned(),
                secrets: secrets.clone(),
                tail: tail.clone(),
                on_ready: on_ready.clone(),
            },
            follow,
        ))
    };
    let (out, err) = match &hooks.output_file {
        Some(path) => {
            let f = tokio::fs::File::open(path)
                .await
                .map_err(|e| CoreError::io(path, e))?;
            let out = spawn_pump(
                Box::new(f) as Box<dyn AsyncRead + Unpin + Send>,
                LogStream::Stdout,
                Some(exited.clone()),
            );
            (Some(out), None)
        }
        None => (
            child.stdout.take().map(|s| {
                spawn_pump(
                    Box::new(s) as Box<dyn AsyncRead + Unpin + Send>,
                    LogStream::Stdout,
                    None,
                )
            }),
            child.stderr.take().map(|s| {
                spawn_pump(
                    Box::new(s) as Box<dyn AsyncRead + Unpin + Send>,
                    LogStream::Stderr,
                    None,
                )
            }),
        ),
    };

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
    exited.store(true, Ordering::SeqCst);

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
    let output_tail = tail
        .lock()
        .map(|t| {
            t.iter().map(String::as_str).collect::<Vec<_>>().join(
                "
",
            )
        })
        .unwrap_or_default();
    let exit = GameExit {
        code,
        crash_report,
        duration: started.elapsed(),
        killed,
        output_tail,
        started_at: started_wall,
    };
    tracing::info!(?exit.code, killed, secs = exit.duration.as_secs(), "game exited");
    events.emit(CoreEvent::GameExited {
        task: task.to_owned(),
        code,
        crash_report: exit.crash_report.as_ref().map(|p| p.display().to_string()),
    });
    Ok(exit)
}

struct Pump {
    which: LogStream,
    events: Arc<dyn EventSink>,
    task: String,
    secrets: Arc<Vec<String>>,
    tail: Shared<VecDeque<String>>,
    on_ready: Shared<Option<Box<dyn FnOnce() + Send>>>,
}

/// Reads lines until EOF. With `follow`, EOF only ends the loop once the
/// game has exited (tail -f over the output file).
async fn pump(stream: Box<dyn AsyncRead + Unpin + Send>, p: Pump, follow: Option<Arc<AtomicBool>>) {
    let Pump {
        which,
        events,
        task,
        secrets,
        tail,
        on_ready,
    } = p;
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    let mut parser = Log4jParser::default();
    let emit = |parsed: ParsedLine| {
        let mut text = parsed.text;
        for s in secrets.iter().filter(|s| s.len() > 3) {
            text = text.replace(s.as_str(), "***");
        }
        let line = mask_secrets(&text);
        if is_ready_line(&line)
            && let Some(f) = on_ready.lock().ok().and_then(|mut g| g.take())
        {
            f();
        }
        if let Ok(mut t) = tail.lock() {
            if t.len() == TAIL_LINES {
                t.pop_front();
            }
            t.push_back(line.clone());
        }
        events.emit(CoreEvent::GameLog {
            task: task.clone(),
            stream: which,
            line,
            level: parsed.level,
            time_ms: parsed.time_ms,
            thread: parsed.thread,
        });
    };
    while let Ok(n) = reader.read_until(b'\n', &mut buf).await {
        let following = follow
            .as_ref()
            .is_some_and(|exited| !exited.load(Ordering::SeqCst));
        if n == 0 && buf.is_empty() {
            if following {
                tokio::time::sleep(FOLLOW_INTERVAL).await;
                continue;
            }
            break;
        }
        // A followed file may end mid-line while the game is still writing:
        // keep the partial line and wait for the rest.
        if following && !buf.ends_with(b"\n") {
            tokio::time::sleep(FOLLOW_INTERVAL).await;
            continue;
        }
        // The game may not emit UTF-8 (e.g. legacy code pages).
        let line = String::from_utf8_lossy(&buf).into_owned();
        buf.clear();
        if let Some(parsed) = parser.feed(line.trim_end_matches(['\r', '\n'])) {
            emit(parsed);
        }
    }
    if let Some(parsed) = parser.finish() {
        emit(parsed);
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

/// Telefon (A3): oyun motoru sürecinin durumu (`GamePlugin.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineStatus {
    pub running: bool,
    /// Süreç kapandı ve sistem bunu çökme olarak kaydetti (Android 11+).
    pub crashed: bool,
}

/// Motor süreci hiç görünmezse bu kadar beklenir (yavaş telefonda açılış).
const ENGINE_START_GRACE: Duration = Duration::from_secs(30);
/// Motor durumunun sorulma aralığı.
pub const ENGINE_POLL: Duration = Duration::from_secs(2);

/// Telefon (A3): ayrı süreçteki oyunu kapanana kadar izler ve masaüstündeki
/// gibi bir [`GameExit`] üretir. `status` `None` dönerse izlenemiyordur
/// (motor yok); o zaman `None` döner. İptal (Durdur) `stop`'u çağırır.
///
/// Çıkış kodu motorun yazdığı dosyadan gelir. Dosya yoksa: sistem çökme
/// kaydettiyse ya da yeni bir çökme raporu varsa çökme, değilse oyun son
/// uygulamalardan kapatılmıştır (normal bitiş, `killed`).
pub async fn watch_engine<S, SF, K, KF>(
    status: S,
    stop: K,
    game_dir: &Path,
    events: Arc<dyn EventSink>,
    task: &str,
    cancel: &CancellationToken,
    poll: Duration,
) -> Option<GameExit>
where
    S: Fn(SystemTime) -> SF,
    SF: std::future::Future<Output = Option<EngineStatus>>,
    K: FnOnce() -> KF,
    KF: std::future::Future<Output = ()>,
{
    let started = std::time::Instant::now();
    let started_wall = SystemTime::now();
    let mut seen = false;
    let mut killed = false;
    let mut last = EngineStatus::default();
    loop {
        tokio::select! {
            () = cancel.cancelled() => {
                stop().await;
                killed = true;
                break;
            }
            () = tokio::time::sleep(poll) => {}
        }
        let s = status(started_wall).await?;
        if s.running {
            seen = true;
            continue;
        }
        if seen || s.crashed || started.elapsed() >= ENGINE_START_GRACE {
            last = s;
            break;
        }
    }

    let file_code = super::android::take_exit_code(game_dir);
    let code = if killed { None } else { file_code };
    let crash_report = if !killed && code != Some(0) {
        newest_crash_report(&game_dir.join("crash-reports"), started_wall)
    } else {
        None
    };
    let closed_outside = !killed && code.is_none() && !last.crashed && crash_report.is_none();
    let exit = GameExit {
        code,
        crash_report,
        duration: started.elapsed(),
        killed: killed || closed_outside,
        output_tail: file_tail(&game_dir.join("logs").join("android-latest.log")),
        started_at: started_wall,
    };
    tracing::info!(?exit.code, killed, closed_outside, crashed = last.crashed, secs = exit.duration.as_secs(), "game engine exited");
    events.emit(CoreEvent::GameExited {
        task: task.to_owned(),
        code: exit.code,
        crash_report: exit.crash_report.as_ref().map(|p| p.display().to_string()),
    });
    Some(exit)
}

/// Dosyanın son `TAIL_LINES` satırı (çökme analizi için).
fn file_tail(path: &Path) -> String {
    let Ok(raw) = std::fs::read(path) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&raw);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
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

    struct Quiet;
    impl EventSink for Quiet {
        fn emit(&self, _e: CoreEvent) {}
    }

    /// Sırayla verilen durumları döndüren sahte motor.
    fn scripted(
        steps: Vec<Option<EngineStatus>>,
    ) -> impl Fn(SystemTime) -> std::future::Ready<Option<EngineStatus>> {
        let steps = Mutex::new(VecDeque::from(steps));
        move |_| {
            let next = steps
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Some(EngineStatus::default()));
            std::future::ready(next)
        }
    }

    const RUN: Option<EngineStatus> = Some(EngineStatus {
        running: true,
        crashed: false,
    });
    const GONE: Option<EngineStatus> = Some(EngineStatus {
        running: false,
        crashed: false,
    });
    const CRASHED: Option<EngineStatus> = Some(EngineStatus {
        running: false,
        crashed: true,
    });

    async fn watch(
        dir: &Path,
        steps: Vec<Option<EngineStatus>>,
        cancel: &CancellationToken,
    ) -> Option<GameExit> {
        watch_engine(
            scripted(steps),
            || std::future::ready(()),
            dir,
            Arc::new(Quiet),
            "t",
            cancel,
            Duration::from_millis(1),
        )
        .await
    }

    #[tokio::test]
    async fn engine_exit_cases() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let exit_file = super::super::android::exit_file(dir.path());
        std::fs::create_dir_all(exit_file.parent().unwrap()).unwrap();

        // Oyundan "Oyundan çık": motor 0 yazar.
        std::fs::write(&exit_file, "0").unwrap();
        let e = watch(dir.path(), vec![GONE, RUN, RUN, GONE], &cancel)
            .await
            .unwrap();
        assert_eq!((e.code, e.killed), (Some(0), false));
        assert!(!exit_file.exists());

        // Java hata koduyla bitti: çökme analizi yapılmalı.
        std::fs::write(&exit_file, "1").unwrap();
        let e = watch(dir.path(), vec![RUN, GONE], &cancel).await.unwrap();
        assert_eq!((e.code, e.killed), (Some(1), false));

        // Son uygulamalardan kaydırıldı: dosya yok, çökme kaydı yok → normal bitiş.
        let e = watch(dir.path(), vec![RUN, GONE], &cancel).await.unwrap();
        assert_eq!((e.code, e.killed), (None, true));

        // Sistem çökme kaydetti (yerel çökme): analiz yapılmalı.
        std::fs::write(
            dir.path().join("logs").join("android-latest.log"),
            "a\nb\nboom",
        )
        .unwrap();
        let e = watch(dir.path(), vec![RUN, CRASHED], &cancel)
            .await
            .unwrap();
        assert_eq!((e.code, e.killed), (None, false));
        assert!(e.output_tail.ends_with("boom"));

        // İzleme yok (masaüstü / eklenti yok).
        assert!(watch(dir.path(), vec![None], &cancel).await.is_none());

        // Durdur düğmesi.
        cancel.cancel();
        let e = watch(dir.path(), vec![RUN], &cancel).await.unwrap();
        assert!(e.killed);
    }

    struct Collect(Mutex<Vec<String>>);
    impl EventSink for Collect {
        fn emit(&self, e: CoreEvent) {
            if let CoreEvent::GameLog { line, .. } = e {
                self.0.lock().unwrap().push(line);
            }
        }
    }

    #[tokio::test]
    async fn follows_a_growing_file_until_exit() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.log");
        let mut w = std::fs::File::create(&path).unwrap();
        let sink = Arc::new(Collect(Mutex::new(vec![])));
        let exited = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let r = ready.clone();
        let p = Pump {
            which: LogStream::Stdout,
            events: sink.clone(),
            task: "t".into(),
            secrets: Arc::new(vec![]),
            tail: Arc::new(Mutex::new(VecDeque::new())),
            on_ready: Arc::new(Mutex::new(Some(Box::new(move || {
                r.store(true, Ordering::SeqCst)
            })))),
        };
        let f = tokio::fs::File::open(&path).await.unwrap();
        let h = tokio::spawn(pump(Box::new(f), p, Some(exited.clone())));
        write!(
            w,
            "first
half"
        )
        .unwrap();
        w.flush().unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        write!(
            w,
            " line
[Render thread/INFO]: Sound engine started
last"
        )
        .unwrap();
        w.flush().unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(ready.load(Ordering::SeqCst));
        exited.store(true, Ordering::SeqCst);
        h.await.unwrap();
        assert_eq!(
            *sink.0.lock().unwrap(),
            [
                "first",
                "half line",
                "[Render thread/INFO]: Sound engine started",
                "last"
            ]
        );
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
