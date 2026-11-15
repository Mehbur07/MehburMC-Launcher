//! Telefonda oyunu başlatma (K104).
//!
//! Rust tarafı başlatma planını bir dosyaya yazar ve Android'deki `GamePlugin`'e
//! (Kotlin) iletir. O da oyun ekranını (`GameActivity`) launcher'dan ayrı bir
//! süreçte açar; oyun kapanınca ya da çökünce launcher etkilenmez. Launcher
//! oyun sürecinin açık olup olmadığını `status` ile sorar, `stop` ile kapatır (A3).

use launcher_core::launch::android::AndroidLaunch;
use launcher_core::launch::process::EngineStatus;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime};

#[cfg(target_os = "android")]
struct Engine<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mcgame")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("com.mehbur.mclauncher", "GamePlugin")?;
                _app.manage(Engine(handle));
            }
            Ok(())
        })
        .build()
}

/// Planı yazar ve oyun ekranını açar.
#[cfg(target_os = "android")]
pub fn start<R: Runtime>(
    app: &AppHandle<R>,
    plan: &AndroidLaunch,
    plan_path: &std::path::Path,
) -> launcher_core::Result<()> {
    use launcher_core::CoreError;
    use tauri::Manager;

    if let Some(dir) = plan_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
    }
    let json = serde_json::to_vec_pretty(plan)
        .map_err(|e| CoreError::EngineStart(format!("plan: {e}")))?;
    std::fs::write(plan_path, json).map_err(|e| CoreError::io(plan_path, e))?;
    let engine = app
        .try_state::<Engine<R>>()
        .ok_or_else(|| CoreError::EngineStart("plugin not registered".into()))?;
    engine
        .0
        .run_mobile_plugin::<serde_json::Value>(
            "launch",
            serde_json::json!({ "planPath": plan_path.display().to_string() }),
        )
        .map_err(|e| CoreError::EngineStart(e.to_string()))?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn start<R: Runtime>(
    _app: &AppHandle<R>,
    _plan: &AndroidLaunch,
    _plan_path: &std::path::Path,
) -> launcher_core::Result<()> {
    Err(launcher_core::CoreError::EngineStart(
        "no game engine on this platform".into(),
    ))
}

/// Oyun süreci açık mı, kapandıysa sistem çökme olarak mı kaydetti (A3).
/// Eklenti yanıt vermezse `None`: launcher oyunu izlemeyi bırakır.
#[cfg(target_os = "android")]
pub fn status<R: Runtime>(
    app: &AppHandle<R>,
    since: std::time::SystemTime,
) -> Option<EngineStatus> {
    use tauri::Manager;

    #[derive(serde::Deserialize)]
    struct Reply {
        running: bool,
        crashed: bool,
    }
    let since_ms = since
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();
    let engine = app.try_state::<Engine<R>>()?;
    match engine
        .0
        .run_mobile_plugin::<Reply>("status", serde_json::json!({ "since": since_ms }))
    {
        Ok(r) => Some(EngineStatus {
            running: r.running,
            crashed: r.crashed,
        }),
        Err(e) => {
            tracing::warn!(error = %e, "cannot read the game engine status");
            None
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn status<R: Runtime>(
    _app: &AppHandle<R>,
    _since: std::time::SystemTime,
) -> Option<EngineStatus> {
    None
}

/// Oyun sürecini kapatır (launcher'daki Durdur düğmesi).
#[cfg(target_os = "android")]
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Manager;
    if let Some(engine) = app.try_state::<Engine<R>>()
        && let Err(e) = engine
            .0
            .run_mobile_plugin::<serde_json::Value>("stop", serde_json::json!({}))
    {
        tracing::warn!(error = %e, "cannot stop the game engine");
    }
}

#[cfg(not(target_os = "android"))]
pub fn stop<R: Runtime>(_app: &AppHandle<R>) {}
