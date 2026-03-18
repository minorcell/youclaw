mod backend;

use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;

use backend::BackendState;
use tauri::{
    menu::{MenuBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use tauri_plugin_notification::NotificationExt;

const MAIN_WINDOW_LABEL: &str = "main";
const MENU_BAR_TRAY_ID: &str = "menu-bar";
const MENU_BAR_SHOW_WINDOW_ID: &str = "menu-bar-show-window";

#[derive(Clone)]
struct DesktopState {
    backend: BackendState,
    ws_endpoint: String,
}

#[tauri::command]
fn get_ws_endpoint(state: tauri::State<'_, DesktopState>) -> String {
    state.ws_endpoint.clone()
}

#[tauri::command]
fn get_menu_bar_enabled(state: tauri::State<'_, DesktopState>) -> Result<bool, String> {
    state
        .backend
        .storage
        .get_menu_bar_enabled()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn set_menu_bar_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    enabled: bool,
) -> Result<bool, String> {
    let previous = state
        .backend
        .storage
        .get_menu_bar_enabled()
        .map_err(|err| err.to_string())?;

    if previous == enabled {
        return Ok(enabled);
    }

    apply_menu_bar_state(&app, enabled).map_err(|err| err.to_string())?;
    if let Err(err) = state.backend.storage.set_menu_bar_enabled(enabled) {
        let _ = apply_menu_bar_state(&app, previous);
        return Err(err.to_string());
    }

    Ok(enabled)
}

#[tauri::command]
fn send_test_notification(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if cfg!(debug_assertions) {
        return send_macos_dev_test_notification();
    }

    app.notification()
        .builder()
        .title("YouClaw")
        .body("通知权限已就绪，后续可以在这里接收系统提醒。")
        .show()
        .map_err(|err| err.to_string())
}

#[cfg(target_os = "macos")]
fn send_macos_dev_test_notification() -> Result<(), String> {
    let script = format!(
        "display notification \"{}\" with title \"{}\" subtitle \"{}\"",
        apple_script_escape("通知权限已就绪，后续可以在这里接收系统提醒。"),
        apple_script_escape("YouClaw"),
        apple_script_escape("开发模式测试通知")
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|err| err.to_string())?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err("osascript failed to deliver notification".to_string())
    } else {
        Err(stderr)
    }
}

#[cfg(target_os = "macos")]
fn apple_script_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
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
            let menu_bar_enabled = backend
                .storage
                .get_menu_bar_enabled()
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            app.manage(DesktopState {
                backend,
                ws_endpoint,
            });
            if menu_bar_enabled {
                apply_menu_bar_state(&app.handle(), true)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_ws_endpoint,
            get_menu_bar_enabled,
            set_menu_bar_enabled,
            send_test_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn apply_menu_bar_state<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> std::io::Result<()> {
    if enabled {
        ensure_menu_bar_tray(app)?;
        if let Some(tray) = app.tray_by_id(MENU_BAR_TRAY_ID) {
            tray.set_visible(true)
                .map_err(|err| std::io::Error::other(err.to_string()))?;
        }
        return Ok(());
    }

    if let Some(tray) = app.tray_by_id(MENU_BAR_TRAY_ID) {
        tray.set_visible(false)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
    }

    Ok(())
}

fn ensure_menu_bar_tray<R: Runtime>(app: &AppHandle<R>) -> std::io::Result<()> {
    if app.tray_by_id(MENU_BAR_TRAY_ID).is_some() {
        return Ok(());
    }

    let quit_item = PredefinedMenuItem::quit(app, Some("退出 YouClaw"))
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    let menu = MenuBuilder::new(app)
        .text(MENU_BAR_SHOW_WINDOW_ID, "显示主窗口")
        .separator()
        .item(&quit_item)
        .build()
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    let mut builder = TrayIconBuilder::with_id(MENU_BAR_TRAY_ID)
        .menu(&menu)
        .tooltip("YouClaw")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == MENU_BAR_SHOW_WINDOW_ID {
                let _ = reveal_main_window(app);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = reveal_main_window(&tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder
        .build(app)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(())
}

fn reveal_main_window<R: Runtime>(app: &AppHandle<R>) -> std::io::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };

    window
        .unminimize()
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    window
        .show()
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    window
        .set_focus()
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    Ok(())
}

fn fallback_data_dir(app_name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(format!(".{}-data", app_name))
}
