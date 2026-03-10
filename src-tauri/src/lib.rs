mod backend;

use std::path::PathBuf;

use backend::BackendState;
use tauri::Manager;

#[derive(Clone)]
struct DesktopState {
    #[allow(dead_code)]
    backend: BackendState,
    ws_endpoint: String,
}

#[tauri::command]
fn get_ws_endpoint(state: tauri::State<'_, DesktopState>) -> String {
    state.ws_endpoint.clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| fallback_data_dir(app.package_info().name.as_str()));
            let backend = BackendState::new(data_dir)
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            let ws_endpoint =
                tauri::async_runtime::block_on(backend::ws::start_ws_server(backend.clone()))
                    .map_err(|err| std::io::Error::other(err.to_string()))?;
            app.manage(DesktopState {
                backend,
                ws_endpoint,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_ws_endpoint])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn fallback_data_dir(app_name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(format!(".{}-data", app_name))
}
