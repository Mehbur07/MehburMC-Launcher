//! Telefonda oyunu başlatma (K104).
//!
//! Rust tarafı başlatma planını bir dosyaya yazar ve Android'deki `GamePlugin`'e
//! (Kotlin) iletir. O da oyun ekranını (`GameActivity`) launcher'dan ayrı bir
//! süreçte açar; oyun kapanınca ya da çökünce launcher etkilenmez.

use launcher_core::launch::android::AndroidLaunch;
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
