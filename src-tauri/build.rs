// Every app command must be listed here; tauri-build generates an
// `allow-<command>` permission for each, and only commands granted in
// `capabilities/` are callable from the webview.
const COMMANDS: &[&str] = &["get_bootstrap", "save_settings", "open_data_dir"];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
