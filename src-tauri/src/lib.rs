mod agents;
mod agents_md;
mod attachments;
mod catalog;
mod chat;
mod lsp;
mod lsp_client;
mod mcp_client;
mod mcp_servers;
mod pathutil;
mod providers;
mod repo;
mod secret;
mod sessions;
mod shell;
mod skills;
mod tools;
mod workspace_files;

pub const APP_CONFIG_DIR: &str = ".k-agent";
pub const WORKSPACE_AGENTS_DIR: &str = ".agents";

pub(crate) fn serialize_error<E, S>(error: &E, serializer: S) -> Result<S::Ok, S::Error>
where
    E: std::fmt::Display,
    S: serde::Serializer,
{
    serializer.serialize_str(&error.to_string())
}

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    LogicalSize, Manager, Size, State, WindowEvent,
};
use tokio::sync::oneshot;

use attachments::prepare_chat_attachments;
use chat::{generate_app_content, generate_session_title, send_chat_message};
use providers::{
    delete_provider, delete_provider_model, list_providers, refresh_provider_models,
    refresh_single_model, save_provider, set_model_favorite, upsert_provider_model,
};
use sessions::{
    load_sessions, read_session_attachment, read_session_file_revision, save_sessions,
};
use shell::run_shell_command;

static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(false);
static WINDOW_BOUNDS_RESTORED: AtomicBool = AtomicBool::new(false);

const EASTER_EGG_PNG: &[u8] = include_bytes!("../../src/assets/easter-egg.png");
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn require_easter_egg() {
    if EASTER_EGG_PNG.len() < PNG_MAGIC.len() || !EASTER_EGG_PNG.starts_with(PNG_MAGIC) {
        panic!("easter-egg.png missing");
    }
}

#[derive(Default)]
pub(crate) struct LocalWorkspace {
    path: Mutex<Option<PathBuf>>,
}

impl LocalWorkspace {
    pub(crate) fn cloned_path(&self) -> Option<PathBuf> {
        self.path.lock().ok().and_then(|guard| guard.clone())
    }
}

#[derive(Default)]
pub(crate) struct CancelRegistry {
    handles: Mutex<std::collections::HashMap<String, oneshot::Sender<()>>>,
}

impl CancelRegistry {
    pub(crate) fn register(&self, id: String) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let mut handles = self.handles.lock().expect("cancel lock poisoned");
        if let Some(previous) = handles.insert(id, tx) {
            let _ = previous.send(());
        }
        rx
    }

    pub(crate) fn cancel(&self, id: &str) -> bool {
        let mut handles = self.handles.lock().expect("cancel lock poisoned");
        if let Some(tx) = handles.remove(id) {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    pub(crate) fn clear(&self, id: &str) {
        let mut handles = self.handles.lock().expect("cancel lock poisoned");
        handles.remove(id);
    }
}

pub(crate) struct CancelHandle<'a> {
    registry: &'a CancelRegistry,
    id: String,
}

impl<'a> CancelHandle<'a> {
    pub(crate) fn new(registry: &'a CancelRegistry, id: String) -> (Self, oneshot::Receiver<()>) {
        let rx = registry.register(id.clone());
        (Self { registry, id }, rx)
    }
}

impl Drop for CancelHandle<'_> {
    fn drop(&mut self) {
        self.registry.clear(&self.id);
    }
}

#[tauri::command]
fn cancel_running_task(state: State<'_, CancelRegistry>, session_id: String) -> bool {
    let cancelled = state.cancel(&session_id);
    tools::ask_user::ask_user_registry().drain();
    cancelled
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

#[tauri::command]
fn webview_log(level: String, message: String) {
    match level.as_str() {
        "error" => eprintln!("[webview] ERROR {message}"),
        "warn" => eprintln!("[webview] WARN {message}"),
        "debug" => println!("[webview] DEBUG {message}"),
        _ => println!("[webview] {message}"),
    }
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

fn apply_bounds_to_window(
    window: &tauri::WebviewWindow,
    bounds: WindowBounds,
) -> Result<(), String> {
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

pub(crate) fn load_ui_settings(app: &tauri::AppHandle) -> Option<serde_json::Value> {
    let dir = app.path().app_data_dir().ok()?;
    let raw = std::fs::read_to_string(dir.join("settings.json")).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let settings = parsed.get("settings")?;
    if let Some(text) = settings.as_str() {
        return serde_json::from_str(text).ok();
    }
    Some(settings.clone())
}

fn apply_saved_window_bounds(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(settings) = load_ui_settings(app) else {
        return;
    };
    let remember = settings
        .get("rememberWindowSize")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if !remember {
        return;
    }
    let Some(bounds) = settings.get("windowBounds") else {
        return;
    };
    let width = bounds
        .get("width")
        .and_then(|value| value.as_f64())
        .unwrap_or(1100.0);
    let height = bounds
        .get("height")
        .and_then(|value| value.as_f64())
        .unwrap_or(720.0);
    let maximized = bounds
        .get("maximized")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let _ = apply_bounds_to_window(
        window,
        WindowBounds {
            width,
            height,
            maximized,
        },
    );
}

fn restore_mapped_window_bounds(window: &tauri::Window) {
    if window.label() != "main" {
        return;
    }
    if WINDOW_BOUNDS_RESTORED.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(webview) = window.app_handle().get_webview_window("main") else {
        WINDOW_BOUNDS_RESTORED.store(false, Ordering::Relaxed);
        return;
    };
    apply_saved_window_bounds(window.app_handle(), &webview);
}

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
    apply_bounds_to_window(&window, bounds)
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
    tokio::task::spawn_blocking(|| {
        SYSTEM_FONTS
            .get_or_init(collect_system_font_families)
            .clone()
    })
    .await
    .map_err(|error| error.to_string())
}

const CHAT_BACKGROUND_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const CHAT_BACKGROUND_MAX_BYTES: u64 = 5 * 1024 * 1024;

fn chat_background_extension(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "png" => Some("png"),
        "jpg" | "jpeg" => Some("jpg"),
        "webp" => Some("webp"),
        _ => None,
    }
}

fn chat_background_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

fn remove_all_chat_backgrounds(dir: &Path) -> Result<(), String> {
    for ext in CHAT_BACKGROUND_EXTENSIONS {
        let candidate = dir.join(format!("chat-background.{ext}"));
        if candidate.exists() {
            std::fs::remove_file(&candidate).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn chat_background_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatBackgroundUpdate {
    filename: String,
    url: String,
}

#[tauri::command]
async fn set_chat_background_image(
    app: tauri::AppHandle,
    source_path: String,
) -> Result<ChatBackgroundUpdate, String> {
    tauri::async_runtime::spawn_blocking(move || set_chat_background_image_sync(app, source_path))
        .await
        .map_err(|error| error.to_string())?
}

fn set_chat_background_image_sync(
    app: tauri::AppHandle,
    source_path: String,
) -> Result<ChatBackgroundUpdate, String> {
    let source = PathBuf::from(source_path.trim());
    if source.as_os_str().is_empty() {
        return Err("empty path".into());
    }
    let extension =
        chat_background_extension(&source).ok_or_else(|| "unsupported image format".to_string())?;
    let metadata = std::fs::metadata(&source).map_err(|error| error.to_string())?;
    if metadata.len() > CHAT_BACKGROUND_MAX_BYTES {
        return Err(format!(
            "image too large (max {} MB)",
            CHAT_BACKGROUND_MAX_BYTES / 1024 / 1024
        ));
    }
    let bytes = std::fs::read(&source).map_err(|error| error.to_string())?;
    let dir = chat_background_dir(&app)?;
    remove_all_chat_backgrounds(&dir)?;
    let filename = format!("chat-background.{extension}");
    let dest = dir.join(&filename);
    std::fs::write(&dest, &bytes).map_err(|error| error.to_string())?;
    let mime = chat_background_mime(extension).ok_or_else(|| "invalid extension".to_string())?;
    let encoded = BASE64_STANDARD.encode(&bytes);
    Ok(ChatBackgroundUpdate {
        filename,
        url: format!("data:{};base64,{}", mime, encoded),
    })
}

#[tauri::command]
async fn clear_chat_background_image(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = chat_background_dir(&app)?;
        remove_all_chat_backgrounds(&dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn get_chat_background_data_url(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || get_chat_background_data_url_sync(app))
        .await
        .map_err(|error| error.to_string())?
}

fn get_chat_background_data_url_sync(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let dir = chat_background_dir(&app)?;
    for extension in CHAT_BACKGROUND_EXTENSIONS {
        let candidate = dir.join(format!("chat-background.{extension}"));
        if !candidate.exists() {
            continue;
        }
        let bytes = std::fs::read(&candidate).map_err(|error| error.to_string())?;
        let mime = match chat_background_mime(extension) {
            Some(value) => value,
            None => continue,
        };
        let encoded = BASE64_STANDARD.encode(&bytes);
        return Ok(Some(format!("data:{};base64,{}", mime, encoded)));
    }
    Ok(None)
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
    const ICON_SVG: &[u8] = include_bytes!("../icons/512x512_icon_inkscape.svg");

    glib::set_prgname(Some(APP_ID));
    glib::set_application_name("k-agent");

    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return;
    };
    let icon_dir = home.join(".local/share/icons/hicolor/scalable/apps");
    let apps_dir = home.join(".local/share/applications");
    if std::fs::create_dir_all(&icon_dir).is_err() || std::fs::create_dir_all(&apps_dir).is_err() {
        return;
    }

    let icon_path = icon_dir.join(format!("{APP_ID}.svg"));
    if std::fs::write(&icon_path, ICON_SVG).is_err() {
        return;
    }
    let _ = std::fs::copy(&icon_path, icon_dir.join("k-agent.svg"));

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
    let canonical =
        crate::pathutil::canonicalize_path(&absolute).map_err(|error| error.to_string())?;
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
    crate::pathutil::canonicalize_path(&expanded)
        .ok()
        .or(Some(expanded))
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
            return args
                .get(index + 1)
                .and_then(|value| resolve_existing_path(value));
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
    require_easter_egg();
    register_linux_identity();
    silence_libayatana_appindicator_warning();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(LocalWorkspace {
            path: Mutex::new(parse_workspace_arg().or_else(|| {
                let home = default_workspace();
                crate::pathutil::canonicalize_path(&home)
                    .ok()
                    .or(Some(home))
            })),
        })
        .manage(lsp_client::LspHub::new())
        .manage(CancelRegistry::default())
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
            set_chat_background_image,
            clear_chat_background_image,
            get_chat_background_data_url,
            get_workspace_path,
            set_workspace_path,
            repo::get_repo_info,
            list_providers,
            save_provider,
            delete_provider,
            mcp_servers::list_mcp_servers,
            mcp_servers::save_mcp_server,
            mcp_servers::delete_mcp_server,
            mcp_servers::refresh_mcp_server_tools,
            mcp_servers::set_mcp_server_enabled,
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
            agents_md::write_agents_md,
            agents_md::delete_agents_md,
            lsp::list_language_servers,
            lsp::install_language_server,
            lsp_client::uninstall_language_server,
            lsp::resolve_language_server,
            lsp_client::lsp_request,
            workspace_files::list_workspace_files,
            load_sessions,
            save_sessions,
            read_session_attachment,
            read_session_file_revision,
            send_chat_message,
            generate_session_title,
            generate_app_content,
            run_shell_command,
            prepare_chat_attachments,
            webview_log,
            cancel_running_task,
            tools::ask_user::submit_ask_user_answer,
            tools::ask_user::cancel_ask_user_answer,
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

            if let Some(window) = app.get_webview_window("main") {
                apply_saved_window_bounds(app.handle(), &window);
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
            }
            if let Some(settings) = load_ui_settings(app.handle()) {
                if let Some(enabled) = settings
                    .get("minimizeToTray")
                    .and_then(|value| value.as_bool())
                {
                    MINIMIZE_TO_TRAY.store(enabled, Ordering::Relaxed);
                }
            }

            let toggle_item = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
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
        .on_window_event(|window, event| match event {
            WindowEvent::Resized(_) => restore_mapped_window_bounds(window),
            WindowEvent::CloseRequested { api, .. } => {
                let enabled = MINIMIZE_TO_TRAY.load(Ordering::Relaxed);
                if enabled {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
