mod providers;

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    LogicalSize, Manager, Size, WindowEvent,
};

use providers::{
    delete_provider, list_providers, refresh_provider_models, refresh_single_model, save_provider,
};

static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn set_minimize_to_tray(enabled: bool) {
    MINIMIZE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn get_minimize_to_tray() -> bool {
    MINIMIZE_TO_TRAY.load(Ordering::Relaxed)
}

#[tauri::command]
fn window_minimize(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::WebviewWindow) -> Result<(), String> {
    let maximized = window.is_maximized().unwrap_or(false);
    if maximized {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_is_maximized(window: tauri::WebviewWindow) -> bool {
    window.is_maximized().unwrap_or(false)
}

#[tauri::command]
fn window_close(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowBounds {
    width: f64,
    height: f64,
    maximized: bool,
}

const MIN_WINDOW_WIDTH: f64 = 720.0;
const MIN_WINDOW_HEIGHT: f64 = 480.0;

#[tauri::command]
fn window_get_bounds(window: tauri::WebviewWindow) -> WindowBounds {
    let maximized = window.is_maximized().unwrap_or(false);
    let (width, height) = match (window.inner_size(), window.scale_factor()) {
        (Ok(size), Ok(scale)) => {
            let logical = size.to_logical::<f64>(scale);
            (logical.width, logical.height)
        }
        _ => (1100.0, 720.0),
    };
    WindowBounds {
        width,
        height,
        maximized,
    }
}

#[tauri::command]
fn window_apply_bounds(window: tauri::WebviewWindow, bounds: WindowBounds) -> Result<(), String> {
    let width = bounds.width.max(MIN_WINDOW_WIDTH);
    let height = bounds.height.max(MIN_WINDOW_HEIGHT);
    let _ = window.unmaximize();
    window
        .set_size(Size::Logical(LogicalSize { width, height }))
        .map_err(|e| e.to_string())?;
    if bounds.maximized {
        window.maximize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn silence_libayatana_appindicator_warning() {
    use glib::{log_set_handler, LogLevels};

    log_set_handler(
        Some("libayatana-appindicator"),
        LogLevels::all(),
        false,
        false,
        |_domain, _level, _message| {},
    );
}

#[cfg(not(target_os = "linux"))]
fn silence_libayatana_appindicator_warning() {}

#[cfg(target_os = "linux")]
fn register_linux_identity() {
    const APP_ID: &str = "dev.kagent.app";
    const ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");

    glib::set_prgname(Some(APP_ID));
    glib::set_application_name("k-agent");

    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return;
    };
    let icon_dir = home.join(".local/share/icons/hicolor/128x128/apps");
    let apps_dir = home.join(".local/share/applications");
    if std::fs::create_dir_all(&icon_dir).is_err() || std::fs::create_dir_all(&apps_dir).is_err() {
        return;
    }

    let icon_path = icon_dir.join(format!("{APP_ID}.png"));
    if std::fs::write(&icon_path, ICON_PNG).is_err() {
        return;
    }
    let _ = std::fs::copy(&icon_path, icon_dir.join("k-agent.png"));

    let exec = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "k-agent".to_string());
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=k-agent\nComment=AI coding model orchestrator\nExec={exec}\nIcon={APP_ID}\nTerminal=false\nCategories=Development;\nStartupWMClass={APP_ID}\n"
    );
    let _ = std::fs::write(apps_dir.join(format!("{APP_ID}.desktop")), &desktop);
    let _ = std::fs::write(apps_dir.join("k-agent.desktop"), desktop);
}

#[cfg(not(target_os = "linux"))]
fn register_linux_identity() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    register_linux_identity();
    silence_libayatana_appindicator_warning();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            set_minimize_to_tray,
            get_minimize_to_tray,
            window_minimize,
            window_toggle_maximize,
            window_is_maximized,
            window_close,
            window_get_bounds,
            window_apply_bounds,
            list_providers,
            save_provider,
            delete_provider,
            refresh_provider_models,
            refresh_single_model,
        ])
        .setup(|app| {
            if let (Some(window), Some(icon)) =
                (app.get_webview_window("main"), app.default_window_icon())
            {
                let _ = window.set_icon(icon.clone());
            }

            let toggle_item =
                MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("k-agent")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => toggle_window_visibility(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window_visibility(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let enabled = MINIMIZE_TO_TRAY.load(Ordering::Relaxed);
                if enabled {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
