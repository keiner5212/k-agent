mod agents;
mod agents_md;
mod catalog;
mod providers;
mod repo;
mod secret;
mod skills;

pub const APP_CONFIG_DIR: &str = ".k-agent";
pub const WORKSPACE_AGENTS_DIR: &str = ".agents";

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    LogicalSize, Manager, Size, State, WindowEvent,
};

use providers::{
    delete_provider, delete_provider_model, list_providers, refresh_provider_models,
    refresh_single_model, save_provider, set_model_favorite, upsert_provider_model,
};

static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct LocalWorkspace {
    path: Mutex<Option<PathBuf>>,
}

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

fn parse_resize_edge(edge: &str) -> Result<tauri_runtime::ResizeDirection, String> {
    match edge {
        "north" => Ok(tauri_runtime::ResizeDirection::North),
        "south" => Ok(tauri_runtime::ResizeDirection::South),
        "east" => Ok(tauri_runtime::ResizeDirection::East),
        "west" => Ok(tauri_runtime::ResizeDirection::West),
        "north-east" => Ok(tauri_runtime::ResizeDirection::NorthEast),
        "north-west" => Ok(tauri_runtime::ResizeDirection::NorthWest),
        "south-east" => Ok(tauri_runtime::ResizeDirection::SouthEast),
        "south-west" => Ok(tauri_runtime::ResizeDirection::SouthWest),
        _ => Err("invalid resize edge".into()),
    }
}

#[tauri::command]
fn window_start_resize(window: tauri::Window, edge: String) -> Result<(), String> {
    let direction = parse_resize_edge(&edge)?;
    window
        .start_resize_dragging(direction)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn window_start_drag(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

static SYSTEM_FONTS: OnceLock<Vec<String>> = OnceLock::new();

fn collect_system_font_families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .filter_map(|face| {
            let (name, _) = face.families.first()?;
            let trimmed = name.trim();
            if trimmed.is_empty() || trimmed.starts_with('.') {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

#[tauri::command]
async fn list_system_fonts() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| SYSTEM_FONTS.get_or_init(collect_system_font_families).clone())
        .await
        .map_err(|error| error.to_string())
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

#[tauri::command]
fn get_workspace_path(state: State<'_, LocalWorkspace>) -> Option<String> {
    state
        .path
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|path| path.display().to_string()))
}

#[tauri::command]
fn set_workspace_path(state: State<'_, LocalWorkspace>, path: String) -> Result<(), String> {
    let expanded = expand_user_path(&path).ok_or_else(|| format!("invalid path: {path}"))?;
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(expanded)
    };
    if !absolute.exists() {
        std::fs::create_dir_all(&absolute).map_err(|error| error.to_string())?;
    }
    let canonical = std::fs::canonicalize(&absolute).map_err(|error| error.to_string())?;
    let mut guard = state.path.lock().map_err(|error| error.to_string())?;
    *guard = Some(canonical);
    Ok(())
}

fn expand_user_path(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "~" {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    if let Some(suffix) = trimmed.strip_prefix("~/") {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        return Some(home.join(suffix));
    }
    Some(PathBuf::from(trimmed))
}

fn resolve_existing_path(input: &str) -> Option<PathBuf> {
    let expanded = expand_user_path(input)?;
    std::fs::canonicalize(&expanded).ok().or(Some(expanded))
}

fn parse_workspace_arg() -> Option<PathBuf> {
    if let Ok(from_env) = std::env::var("K_AGENT_WORKSPACE") {
        if let Some(path) = resolve_existing_path(&from_env) {
            return Some(path);
        }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = arg.strip_prefix("--workspace=") {
            return resolve_existing_path(value);
        }
        if arg == "--workspace" || arg == "-w" {
            return args.get(index + 1).and_then(|value| resolve_existing_path(value));
        }
        if !arg.starts_with('-') {
            return resolve_existing_path(arg);
        }
        index += 1;
    }
    None
}

fn default_workspace() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    register_linux_identity();
    silence_libayatana_appindicator_warning();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(LocalWorkspace {
            path: Mutex::new(parse_workspace_arg().or_else(|| {
                let home = default_workspace();
                std::fs::canonicalize(&home).ok().or(Some(home))
            })),
        })
        .invoke_handler(tauri::generate_handler![
            set_minimize_to_tray,
            get_minimize_to_tray,
            window_minimize,
            window_toggle_maximize,
            window_is_maximized,
            window_close,
            window_get_bounds,
            window_apply_bounds,
            window_start_resize,
            window_start_drag,
            window_open_devtools,
            list_system_fonts,
            get_workspace_path,
            set_workspace_path,
            repo::get_repo_info,
            list_providers,
            save_provider,
            delete_provider,
            refresh_provider_models,
            refresh_single_model,
            upsert_provider_model,
            delete_provider_model,
            set_model_favorite,
            skills::list_skills,
            skills::read_skill_meta,
            skills::read_skill_file,
            skills::create_skill,
            skills::update_skill,
            skills::update_skill_content,
            skills::delete_skill,
            agents::list_agents,
            agents::read_agent_meta,
            agents::create_agent,
            agents::update_agent,
            agents::delete_agent,
            agents_md::list_agents_md,
        ])
        .setup(|app| {
            if let Ok(home) = app.path().home_dir() {
                let config_dir = home.join(APP_CONFIG_DIR);
                let _ = std::fs::create_dir_all(&config_dir);
            }
            if let Ok(secrets_dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&secrets_dir);
                if let Err(error) = crate::secret::ensure_master_key(&secrets_dir) {
                    eprintln!("k-agent master key: {error}");
                }
            }

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
