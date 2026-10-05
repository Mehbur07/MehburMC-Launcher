mod bridge;
mod commands;
mod state;

use tauri::window::Color;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::{
    accounts, content, data, dialogs, files, friends, home, instances, loaders, play, skins, update,
};
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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

            if let Some(rx) = app_state.events_rx.lock().expect("events lock").take() {
                tauri::async_runtime::spawn(bridge::forward(app.handle().clone(), rx));
            }
            if let Some(launcher) = app_state.launcher.clone() {
                tauri::async_runtime::spawn(friends::heartbeat_loop(launcher));
            }
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
            commands::open_data_dir,
            commands::list_versions,
            commands::list_java,
            commands::save_text_file,
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::copy_instance,
            instances::reorder_instances,
            instances::select_instance,
            instances::export_instance,
            instances::import_instance,
            instances::open_instance_folder,
            play::launch_instance,
            play::repair_instance,
            play::stop_instance,
            play::list_tasks,
            play::cancel_task,
            play::clear_tasks,
            accounts::list_accounts,
            accounts::add_offline_account,
            accounts::remove_account,
            accounts::rename_account,
            accounts::select_account,
            accounts::account_avatars,
            accounts::set_account_avatar,
            accounts::clear_account_avatar,
            files::list_instance_files,
            files::instance_folder_path,
            files::toggle_instance_file,
            files::delete_instance_file,
            files::read_instance_log,
            loaders::list_loader_versions,
            loaders::import_optifine,
            loaders::install_shader_support,
            content::search_modrinth,
            content::project_versions,
            content::install_content,
            content::scan_content,
            content::update_content,
            content::import_modpack,
            content::install_modrinth_modpack,
            content::content_icon,
            content::open_external,
            friends::friends_status,
            friends::friends_online,
            friends::friends_enable,
            friends::friends_disable,
            friends::friends_list,
            friends::friend_request,
            friends::friend_respond,
            friends::friend_remove,
            friends::friend_block,
            friends::chat_messages,
            friends::chat_send,
            friends::chat_mark_read,
            friends::my_shares,
            friends::share_instance,
            friends::unshare_instance,
            friends::friend_shared_lists,
            friends::install_friend_mods,
            skins::list_skins,
            skins::import_skin_file,
            skins::add_skin_bytes,
            skins::list_default_skins,
            skins::update_skin,
            skins::delete_skin,
            skins::assign_skin,
            skins::export_skin,
            home::list_news,
            home::analyze_crash_report,
            home::open_data_file,
            data::move_data_folder,
            data::default_data_folder,
            data::restart_app,
            update::check_app_update,
            update::install_app_update,
            dialogs::pick_path,
        ])
        .build(tauri::generate_context!())
        .expect("error while building MehburMC Launcher")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                friends::go_offline_on_exit(app);
            }
        });
}
