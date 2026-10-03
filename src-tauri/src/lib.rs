mod commands;
mod state;

use tauri::window::Color;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

/// Matches `--mc-bg` so the window never flashes white before the UI paints.
const BACKGROUND: Color = Color(5, 7, 10, 255);

pub fn run() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let app_state = AppState::init(exe_dir.as_deref());

    tauri::Builder::default()
        // Must be registered first: a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(launcher_core::LAUNCHER_NAME)
                .inner_size(1180.0, 720.0)
                .min_inner_size(960.0, 600.0)
                .decorations(false)
                .shadow(true)
                .resizable(true)
                .center()
                .background_color(BACKGROUND);
            // Portable mode keeps WebView2 data inside the data folder too.
            if let Some(paths) = &app_state.paths
                && paths.mode() == launcher_core::DataMode::Portable
            {
                builder = builder.data_directory(paths.webview_data());
            }
            builder.build()?;
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
            commands::open_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MehburMC Launcher");
}
