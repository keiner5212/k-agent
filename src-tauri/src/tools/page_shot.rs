use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};

use super::{
    context_error, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "page_shot";

const DESCRIPTION: &str = "Capture one viewport of a page that is already being served and return a PNG. One shot per review. Do not retry with another host, a taller window, or a new selector when the image is blank or unchanged. A blank image is a capture miss, not the page. Height is the window (max 1200), not the document. A URL hash scrolls that section into the window. Optional localStorage and sessionStorage are {key, value} lists applied on the page origin before the app script runs. Optional cookies are {name, value} plus optional domain, path, secure, and httpOnly, set before the first request. Default cookie domain is the URL host and must match it. Path defaults to /. Each list is at most 50 items. Each value is at most 8 KB. Start a server with background. It is killed when the turn ends. Do not use bash for that.";

const DEFAULT_WIDTH: i32 = 1280;
const DEFAULT_HEIGHT: i32 = 720;
const MAX_ITEMS: usize = 50;
const MAX_VALUE_BYTES: usize = 8 * 1024;
const READY_POLL_MS: u64 = 250;
const READY_BUDGET_MS: u64 = 10_000;
const BLANK_FRAME: &str = "page_shot captured a blank frame. A blank frame is not the page.";

pub struct PageShotTool;

impl Tool for PageShotTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Page URL. http or https, including localhost." },
                    "width": { "type": "integer", "description": "Viewport width in pixels. Default 1280. Range 320-1600." },
                    "height": { "type": "integer", "description": "Viewport height in pixels. Default 720. Range 240-1200. This is the window, not the page length." },
                    "selector": { "type": "string", "description": "Optional CSS selector. When set, the image is only that element. Prefer one full viewport shot." },
                    "waitMs": { "type": "integer", "description": "Extra delay after the view has painted. Default 300. Max 5000." },
                    "localStorage": {
                        "type": "array",
                        "description": "Optional. Up to 50 {key, value} strings. Applied on the page origin before the app script runs. Each value max 8 KB.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["key", "value"]
                        }
                    },
                    "sessionStorage": {
                        "type": "array",
                        "description": "Optional. Same shape and timing as localStorage, for sessionStorage.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["key", "value"]
                        }
                    },
                    "cookies": {
                        "type": "array",
                        "description": "Optional. Up to 50 cookies, set before the first request. name and value are required. domain defaults to the URL host and must match it. path defaults to /. secure and httpOnly default false. Each value max 8 KB.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "value": { "type": "string" },
                                "domain": { "type": "string" },
                                "path": { "type": "string" },
                                "secure": { "type": "boolean" },
                                "httpOnly": { "type": "boolean" }
                            },
                            "required": ["name", "value"]
                        }
                    }
                },
                "required": ["url"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        if args
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return context_error(None, "page_shot requires a non-empty `url`.");
        }
        context_error(None, "page_shot must run on the async dispatch path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let shot = match parse_shot(&args) {
        Ok(shot) => shot,
        Err(message) => return context_error(None, &message),
    };
    let Some(app) = ctx.app else {
        return context_error(None, "page_shot requires the desktop shell.");
    };
    let png = match capture(app, &shot).await {
        Ok(png) => png,
        Err(message) => return context_error(None, &message),
    };
    let encoded = BASE64.encode(&png);
    let selector_label = shot.selector.clone().unwrap_or_default();
    let local_count = shot.local_storage.len() as i64;
    let session_count = shot.session_storage.len() as i64;
    let cookie_count = shot.cookies.len() as i64;
    ToolOutcome {
        text: toon_doc(&[
            ("url", ToonValue::Str(&shot.url)),
            ("width", ToonValue::Int(shot.width as i64)),
            ("height", ToonValue::Int(shot.height as i64)),
            ("selector", ToonValue::Str(&selector_label)),
            ("localStorage", ToonValue::Int(local_count)),
            ("sessionStorage", ToonValue::Int(session_count)),
            ("cookies", ToonValue::Int(cookie_count)),
            ("bytes", ToonValue::Int(png.len() as i64)),
            ("image", ToonValue::Str("png attached")),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("ok".into()),
            image_data: Some(encoded),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: Some(png),
        file: None,
    }
}

#[derive(Clone, Debug)]
struct StorageItem {
    key: String,
    value: String,
}

#[derive(Clone, Debug)]
struct CookieInput {
    name: String,
    value: String,
    domain: String,
    path: String,
    secure: bool,
    http_only: bool,
}

#[derive(Clone, Debug)]
struct ShotArgs {
    url: String,
    width: i32,
    height: i32,
    selector: Option<String>,
    wait_ms: u64,
    local_storage: Vec<StorageItem>,
    session_storage: Vec<StorageItem>,
    cookies: Vec<CookieInput>,
}

fn parse_shot(args: &Value) -> Result<ShotArgs, String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err("page_shot requires a non-empty `url`.".into());
    }
    let parsed = check_url(&url)?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "page_shot URL has no host.".to_string())?;
    let selector = args
        .get("selector")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(ShotArgs {
        url,
        width: clamp_i32(
            args.get("width").and_then(Value::as_i64),
            DEFAULT_WIDTH,
            320,
            1600,
        ),
        height: clamp_i32(
            args.get("height").and_then(Value::as_i64),
            DEFAULT_HEIGHT,
            240,
            1200,
        ),
        selector,
        wait_ms: args
            .get("waitMs")
            .and_then(Value::as_u64)
            .unwrap_or(300)
            .min(5_000),
        local_storage: parse_pairs(args, "localStorage")?,
        session_storage: parse_pairs(args, "sessionStorage")?,
        cookies: parse_cookies(args, host)?,
    })
}

fn check_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| "page_shot URL is not parseable.".to_string())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("page_shot only accepts http and https URLs.".into());
    }
    Ok(parsed)
}

fn clamp_i32(value: Option<i64>, default_value: i32, min: i32, max: i32) -> i32 {
    let Some(value) = value else {
        return default_value;
    };
    (value as i32).clamp(min, max)
}

fn parse_pairs(args: &Value, field: &str) -> Result<Vec<StorageItem>, String> {
    let Some(value) = args.get(field) else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("page_shot {field} must be an array."))?;
    if items.len() > MAX_ITEMS {
        return Err(format!(
            "page_shot {field} has more than {MAX_ITEMS} entries."
        ));
    }
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let key = item
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("page_shot {field}[{index}] requires a string key."))?;
        let stored = item
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("page_shot {field}[{index}] requires a string value."))?;
        if key.is_empty() {
            return Err(format!("page_shot {field}[{index}] key is empty."));
        }
        if stored.len() > MAX_VALUE_BYTES {
            return Err(format!("page_shot {field}[{index}] value is over 8 KB."));
        }
        out.push(StorageItem {
            key: key.to_string(),
            value: stored.to_string(),
        });
    }
    Ok(out)
}

fn parse_cookies(args: &Value, host: &str) -> Result<Vec<CookieInput>, String> {
    let Some(value) = args.get("cookies") else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| "page_shot cookies must be an array.".to_string())?;
    if items.len() > MAX_ITEMS {
        return Err(format!(
            "page_shot cookies has more than {MAX_ITEMS} entries."
        ));
    }
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("page_shot cookies[{index}] requires a string name."))?;
        let stored = item
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("page_shot cookies[{index}] requires a string value."))?;
        if name.is_empty() {
            return Err(format!("page_shot cookies[{index}] name is empty."));
        }
        if stored.len() > MAX_VALUE_BYTES {
            return Err(format!("page_shot cookies[{index}] value is over 8 KB."));
        }
        let domain = match item.get("domain") {
            None => host.to_string(),
            Some(Value::String(domain)) => {
                let domain = domain.trim();
                if domain.is_empty() {
                    return Err(format!("page_shot cookies[{index}] domain is empty."));
                }
                if !cookie_domain_matches(domain, host) {
                    return Err(format!(
                        "page_shot cookies[{index}] domain does not match the URL host."
                    ));
                }
                cookie_domain_value(domain, host)
            }
            Some(_) => {
                return Err(format!(
                    "page_shot cookies[{index}] domain must be a string."
                ));
            }
        };
        let path = match item.get("path") {
            None => "/".to_string(),
            Some(Value::String(path)) if !path.is_empty() => path.clone(),
            Some(Value::String(_)) => {
                return Err(format!("page_shot cookies[{index}] path is empty."));
            }
            Some(_) => return Err(format!("page_shot cookies[{index}] path must be a string.")),
        };
        out.push(CookieInput {
            name: name.to_string(),
            value: stored.to_string(),
            domain,
            path,
            secure: optional_bool(item, "secure", index)?,
            http_only: optional_bool(item, "httpOnly", index)?,
        });
    }
    Ok(out)
}

fn optional_bool(item: &Value, field: &str, index: usize) -> Result<bool, String> {
    match item.get(field) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!(
            "page_shot cookies[{index}] {field} must be a boolean."
        )),
    }
}

fn cookie_domain_value(domain: &str, host: &str) -> String {
    let bare = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    if bare.eq_ignore_ascii_case(host) {
        bare
    } else {
        format!(".{bare}")
    }
}

fn cookie_domain_matches(domain: &str, host: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || host.is_empty() {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return host == domain;
    }
    host == domain
        || (host.len() > domain.len()
            && host.as_bytes().get(host.len() - domain.len() - 1) == Some(&b'.')
            && host.ends_with(&domain))
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn storage_script(local: &[StorageItem], session: &[StorageItem]) -> String {
    let mut script = String::from("(function(){try{");
    for item in local {
        script.push_str(&format!(
            "localStorage.setItem({},{});",
            js_string(&item.key),
            js_string(&item.value)
        ));
    }
    for item in session {
        script.push_str(&format!(
            "sessionStorage.setItem({},{});",
            js_string(&item.key),
            js_string(&item.value)
        ));
    }
    script.push_str("}catch(e){}})();");
    script
}

fn origin_allows(url: &reqwest::Url) -> Vec<String> {
    let host = url.host_str().unwrap_or("");
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let scheme = url.scheme();
    let mut allows = vec![format!("{scheme}://{host}/*")];
    if let Some(port) = url.port() {
        allows.push(format!("{scheme}://{host}:{port}/*"));
    }
    allows
}

async fn capture(app: &tauri::AppHandle, shot: &ShotArgs) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "linux")]
    {
        capture_linux(app, shot).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, shot);
        Err("page_shot currently requires Linux WebKit.".into())
    }
}

#[cfg(target_os = "linux")]
struct ShotState {
    _window: gtk::Window,
    tx: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>>,
}

#[cfg(target_os = "linux")]
async fn capture_linux(app: &tauri::AppHandle, shot: &ShotArgs) -> Result<Vec<u8>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let shot = shot.clone();
    let app = app.clone();
    app.run_on_main_thread(move || {
        let _ = shoot(shot, tx);
    })
    .map_err(|error| format!("page_shot main thread: {error}"))?;
    match tokio::time::timeout(Duration::from_secs(25), rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("page_shot capture was dropped.".into()),
        Err(_) => Err("page_shot timed out.".into()),
    }
}

#[cfg(target_os = "linux")]
fn shoot(
    shot: ShotArgs,
    tx: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
) -> Result<(), String> {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::prelude::WidgetExtManual;
    use gtk::prelude::*;
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{LoadEvent, WebView};

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    place_hidden(&window, shot.width, shot.height);
    let view = WebView::new();
    window.add(&view);
    view.set_size_request(shot.width, shot.height);
    window.show_all();
    let state = Rc::new(RefCell::new(Some(ShotState {
        _window: window,
        tx: Some(tx),
    })));
    let cleanup_state = state.clone();
    glib::timeout_add_local(Duration::from_secs(25), move || {
        if let Some(shot) = cleanup_state.borrow_mut().take() {
            unsafe { shot._window.destroy() };
        }
        glib::ControlFlow::Break
    });
    if let Err(message) = install_storage(&view, &shot) {
        deliver(&state, Err(message));
        return Ok(());
    }
    let selector = shot.selector.clone();
    let wait_ms = shot.wait_ms;
    let page_url = shot.url.clone();
    let storage_js = if shot.local_storage.is_empty() && shot.session_storage.is_empty() {
        None
    } else {
        Some(storage_script(&shot.local_storage, &shot.session_storage))
    };
    let started = std::cell::Cell::new(false);
    let storage_ready = std::cell::Cell::new(storage_js.is_none());
    let load_state = state.clone();
    view.connect_load_changed(move |view, event| {
        if event != LoadEvent::Finished || started.get() {
            return;
        }
        let current = view
            .uri()
            .map(|value| value.to_string())
            .unwrap_or_default();
        if current.is_empty() || current.starts_with("about:") {
            return;
        }
        if !storage_ready.get() {
            storage_ready.set(true);
            let script = storage_js.clone().unwrap_or_default();
            let reload = page_url.clone();
            let next = view.clone();
            view.evaluate_javascript(
                &script,
                None,
                None,
                None::<&gio::Cancellable>,
                move |_result| {
                    next.load_uri(&reload);
                },
            );
            return;
        }
        started.set(true);
        scroll_hash(view.clone(), selector.clone(), load_state.clone(), wait_ms);
    });
    load_after_cookies(view, shot.cookies, 0, shot.url, state);
    Ok(())
}

// Opacity 0 leaves the WebKit frame blank. Keep the window mapped off-screen.
#[cfg(target_os = "linux")]
fn place_hidden(window: &gtk::Window, width: i32, height: i32) {
    use gtk::prelude::*;

    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_accept_focus(false);
    window.set_keep_below(true);
    window.set_type_hint(gdk::WindowTypeHint::Utility);
    window.set_default_size(width, height);
    window.set_size_request(width, height);
    window.set_opacity(1.0);
    window.move_(0, offscreen_y(window));
}

#[cfg(target_os = "linux")]
fn offscreen_y(window: &gtk::Window) -> i32 {
    use gdk::prelude::MonitorExt;
    use gtk::prelude::WidgetExt;

    let display = window.display();
    let monitor = display.primary_monitor().or_else(|| display.monitor(0));
    monitor
        .map(|monitor| {
            let geo = monitor.geometry();
            geo.y().saturating_add(geo.height()).saturating_add(32)
        })
        .unwrap_or(16_000)
}

#[cfg(target_os = "linux")]
fn install_storage(view: &webkit2gtk::WebView, shot: &ShotArgs) -> Result<(), String> {
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{
        UserContentInjectedFrames, UserContentManagerExt, UserScript, UserScriptInjectionTime,
    };

    if shot.local_storage.is_empty() && shot.session_storage.is_empty() {
        return Ok(());
    }
    let Some(manager) = view.user_content_manager() else {
        return Err("page_shot could not install page storage.".into());
    };
    let page = check_url(&shot.url)?;
    let allows = origin_allows(&page);
    let allow_refs: Vec<&str> = allows.iter().map(String::as_str).collect();
    let script = UserScript::new(
        &storage_script(&shot.local_storage, &shot.session_storage),
        UserContentInjectedFrames::TopFrame,
        UserScriptInjectionTime::Start,
        &allow_refs,
        &["about:blank"],
    );
    manager.add_script(&script);
    Ok(())
}

#[cfg(target_os = "linux")]
fn load_after_cookies(
    view: webkit2gtk::WebView,
    cookies: Vec<CookieInput>,
    index: usize,
    url: String,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
) {
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{CookieManagerExt, WebContextExt, WebsiteDataManagerExt};

    if index >= cookies.len() {
        view.load_uri(&url);
        return;
    }
    let Some(manager) = view
        .website_data_manager()
        .and_then(|data| data.cookie_manager())
        .or_else(|| view.context().and_then(|context| context.cookie_manager()))
    else {
        deliver(&state, Err("page_shot cookie manager is missing.".into()));
        return;
    };
    let spec = &cookies[index];
    let mut cookie = soup::Cookie::new(&spec.name, &spec.value, &spec.domain, &spec.path, -1);
    cookie.set_secure(spec.secure);
    cookie.set_http_only(spec.http_only);
    let next_view = view.clone();
    let next_state = state.clone();
    let next_url = url.clone();
    let next_cookies = cookies.clone();
    manager.add_cookie(
        &mut cookie,
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(()) => load_after_cookies(next_view, next_cookies, index + 1, next_url, next_state),
            Err(error) => deliver(&next_state, Err(format!("page_shot cookie: {error}"))),
        },
    );
}

#[cfg(target_os = "linux")]
fn scroll_hash(
    view: webkit2gtk::WebView,
    selector: Option<String>,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
    wait_ms: u64,
) {
    use webkit2gtk::WebViewExt;

    let script = "(function(){var h=location.hash;if(!h||h.length<2)return 'ok';var id=decodeURIComponent(h.slice(1));var el=document.getElementById(id);if(!el){try{el=document.querySelector('#'+CSS.escape(id))}catch(e){el=null}}if(el&&el.scrollIntoView)el.scrollIntoView({block:'start'});return 'ok'})()";
    let shot_view = view.clone();
    view.evaluate_javascript(
        script,
        None,
        None,
        None::<&gio::Cancellable>,
        move |_result| {
            poll_ready(
                shot_view,
                selector,
                state,
                wait_ms,
                std::time::Instant::now(),
            );
        },
    );
}

#[cfg(target_os = "linux")]
fn poll_ready(
    view: webkit2gtk::WebView,
    selector: Option<String>,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
    wait_ms: u64,
    started: std::time::Instant,
) {
    use gtk::prelude::*;
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{SnapshotOptions, SnapshotRegion};

    glib::timeout_add_local(Duration::from_millis(READY_POLL_MS), move || {
        if state.borrow().is_none() {
            return glib::ControlFlow::Break;
        }
        let expired = started.elapsed() >= Duration::from_millis(READY_BUDGET_MS);
        let alloc = view.allocation();
        if !expired && (alloc.width() < 1 || alloc.height() < 1) {
            return glib::ControlFlow::Continue;
        }
        if expired {
            finish_after_wait(view.clone(), selector.clone(), state.clone(), wait_ms);
            return glib::ControlFlow::Break;
        }
        let probe_view = view.clone();
        let next_view = view.clone();
        let next_selector = selector.clone();
        let next_state = state.clone();
        view.snapshot(
            SnapshotRegion::Visible,
            SnapshotOptions::NONE,
            None::<&gio::Cancellable>,
            move |result| {
                let painted = result.ok().is_some_and(|surface| !surface_blank(surface));
                if painted || started.elapsed() >= Duration::from_millis(READY_BUDGET_MS) {
                    finish_after_wait(probe_view, next_selector, next_state, wait_ms);
                    return;
                }
                poll_ready(next_view, next_selector, next_state, wait_ms, started);
            },
        );
        glib::ControlFlow::Break
    });
}

#[cfg(target_os = "linux")]
fn finish_after_wait(
    view: webkit2gtk::WebView,
    selector: Option<String>,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
    wait_ms: u64,
) {
    glib::timeout_add_local(Duration::from_millis(wait_ms), move || {
        if state.borrow().is_some() {
            queue_snapshot(view.clone(), selector.clone(), state.clone());
        }
        glib::ControlFlow::Break
    });
}

#[cfg(target_os = "linux")]
fn surface_blank(surface: cairo::Surface) -> bool {
    use std::convert::TryFrom;

    let Ok(mut image) = cairo::ImageSurface::try_from(surface) else {
        return true;
    };
    surface_is_blank(&mut image)
}

#[cfg(target_os = "linux")]
fn queue_snapshot(
    view: webkit2gtk::WebView,
    selector: Option<String>,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
) {
    use webkit2gtk::WebViewExt;

    let Some(selector) = selector else {
        snap(view, None, state);
        return;
    };
    let script = format!(
        "(function(){{var el=document.querySelector({});if(!el)return 'missing';var r=el.getBoundingClientRect();return JSON.stringify({{x:r.x,y:r.y,w:r.width,h:r.height}});}})()",
        serde_json::to_string(&selector).unwrap_or_else(|_| "\"\"".into())
    );
    let view_for_snap = view.clone();
    view.evaluate_javascript(
        &script,
        None,
        None,
        None::<&gio::Cancellable>,
        move |result| {
            let raw = match result {
                Ok(value) => value.to_string(),
                Err(error) => {
                    deliver(&state, Err(format!("page_shot selector: {error}")));
                    return;
                }
            };
            match parse_rect(&raw) {
                Ok(rect) => snap(view_for_snap.clone(), Some(rect), state.clone()),
                Err(message) => deliver(&state, Err(message)),
            }
        },
    );
}

#[cfg(target_os = "linux")]
fn snap(
    view: webkit2gtk::WebView,
    rect: Option<(i32, i32, i32, i32)>,
    state: std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
) {
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{SnapshotOptions, SnapshotRegion};

    view.snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&gio::Cancellable>,
        move |result| {
            let png = match result {
                Ok(surface) => encode_surface(surface, rect),
                Err(error) => Err(format!("page_shot snapshot: {error}")),
            };
            deliver(&state, png);
        },
    );
}

#[cfg(target_os = "linux")]
fn deliver(
    state: &std::rc::Rc<std::cell::RefCell<Option<ShotState>>>,
    result: Result<Vec<u8>, String>,
) {
    use gtk::prelude::WidgetExtManual;
    if let Some(mut shot) = state.borrow_mut().take() {
        unsafe { shot._window.destroy() };
        if let Some(tx) = shot.tx.take() {
            let _ = tx.send(result);
        }
    }
}

#[cfg(target_os = "linux")]
fn parse_rect(raw: &str) -> Result<(i32, i32, i32, i32), String> {
    let raw = raw.trim().trim_matches('"');
    if raw == "missing" || raw.is_empty() {
        return Err("page_shot selector matched no element.".into());
    }
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|_| "page_shot could not read the element box.".to_string())?;
    let x = parsed.get("x").and_then(Value::as_f64).unwrap_or(0.0) as i32;
    let y = parsed.get("y").and_then(Value::as_f64).unwrap_or(0.0) as i32;
    let w = parsed.get("w").and_then(Value::as_f64).unwrap_or(0.0) as i32;
    let h = parsed.get("h").and_then(Value::as_f64).unwrap_or(0.0) as i32;
    if w < 1 || h < 1 {
        return Err("page_shot selector matched an empty element.".into());
    }
    Ok((x, y, w, h))
}

#[cfg(target_os = "linux")]
fn encode_surface(
    surface: cairo::Surface,
    rect: Option<(i32, i32, i32, i32)>,
) -> Result<Vec<u8>, String> {
    use std::convert::TryFrom;

    use cairo::ImageSurface;

    let mut image = ImageSurface::try_from(surface)
        .map_err(|_| "page_shot snapshot was not an image.".to_string())?;
    if surface_is_blank(&mut image) {
        return Err(BLANK_FRAME.into());
    }
    let (x, y, w, h) = rect.unwrap_or((0, 0, image.width(), image.height()));
    encode_crop(&image, x, y, w, h)
}

#[cfg(target_os = "linux")]
fn surface_is_blank(image: &mut cairo::ImageSurface) -> bool {
    let width = image.width();
    let height = image.height();
    if width < 1 || height < 1 {
        return true;
    }
    let stride = image.stride() as usize;
    let Ok(data) = image.data() else {
        return false;
    };
    let step_x = (width / 48).max(1) as usize;
    let step_y = (height / 48).max(1) as usize;
    let mut pixels = Vec::new();
    for y in (0..height as usize).step_by(step_y) {
        for x in (0..width as usize).step_by(step_x) {
            let offset = y * stride + x * 4;
            if offset + 2 >= data.len() {
                continue;
            }
            // ARgb32 is native-endian, so little-endian bytes are B, G, R.
            let b = data[offset];
            let g = data[offset + 1];
            let r = data[offset + 2];
            pixels.push((r, g, b));
        }
    }
    frame_is_blank(&pixels)
}

fn frame_is_blank(pixels: &[(u8, u8, u8)]) -> bool {
    if pixels.is_empty() {
        return true;
    }
    let mut near_flat = 0u32;
    let mut min_l = 255u16;
    let mut max_l = 0u16;
    for (r, g, b) in pixels {
        let l = (u16::from(*r) + u16::from(*g) + u16::from(*b)) / 3;
        min_l = min_l.min(l);
        max_l = max_l.max(l);
        if l < 12 || l > 246 {
            near_flat += 1;
        }
    }
    let spread = max_l.saturating_sub(min_l);
    near_flat * 100 / pixels.len() as u32 >= 97 && spread < 18
}

#[cfg(target_os = "linux")]
fn encode_crop(
    image: &cairo::ImageSurface,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<Vec<u8>, String> {
    let x = x.max(0);
    let y = y.max(0);
    let width = width.min(image.width().saturating_sub(x)).max(1);
    let height = height.min(image.height().saturating_sub(y)).max(1);
    let cropped = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .map_err(|error| format!("page_shot crop: {error}"))?;
    let context =
        cairo::Context::new(&cropped).map_err(|error| format!("page_shot crop: {error}"))?;
    context
        .set_source_surface(image, -x as f64, -y as f64)
        .map_err(|error| format!("page_shot crop: {error}"))?;
    context
        .paint()
        .map_err(|error| format!("page_shot crop: {error}"))?;
    let mut png = Vec::new();
    cropped
        .write_to_png(&mut png)
        .map_err(|error| format!("page_shot encode: {error}"))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::{cookie_domain_matches, frame_is_blank, parse_shot, storage_script, StorageItem};
    use serde_json::json;

    #[test]
    fn solid_black_or_white_is_blank() {
        assert!(frame_is_blank(&vec![(0, 0, 0); 40]));
        assert!(frame_is_blank(&vec![(255, 255, 255); 40]));
    }

    #[test]
    fn painted_page_is_kept() {
        let mut pixels = vec![(246, 241, 231); 30];
        pixels.push((40, 28, 22));
        pixels.push((120, 72, 48));
        assert!(!frame_is_blank(&pixels));
    }

    #[test]
    fn cookie_domain_must_match_host() {
        assert!(cookie_domain_matches("example.com", "example.com"));
        assert!(cookie_domain_matches(".example.com", "app.example.com"));
        assert!(!cookie_domain_matches("evil.com", "app.example.com"));
        assert!(!cookie_domain_matches("example.com", "127.0.0.1"));
        assert!(cookie_domain_matches("127.0.0.1", "127.0.0.1"));

        let foreign = parse_shot(&json!({
            "url": "https://app.example.com/home",
            "cookies": [{ "name": "sid", "value": "abc", "domain": "evil.com" }]
        }));
        assert!(foreign.unwrap_err().contains("domain does not match"));

        let parent = parse_shot(&json!({
            "url": "https://app.example.com/home",
            "cookies": [{ "name": "sid", "value": "abc", "domain": ".example.com", "httpOnly": true }]
        }))
        .expect("parent domain");
        assert_eq!(parent.cookies[0].domain, ".example.com");
        assert!(parent.cookies[0].http_only);
        assert!(!parent.cookies[0].secure);
        assert_eq!(parent.cookies[0].path, "/");

        let omitted = parse_shot(&json!({
            "url": "http://127.0.0.1:9/path",
            "cookies": [{ "name": "sid", "value": "abc" }]
        }))
        .expect("default domain");
        assert_eq!(omitted.cookies[0].domain, "127.0.0.1");
    }

    #[test]
    fn storage_lists_reject_over_limit_before_capture() {
        let value = "y".repeat(super::MAX_VALUE_BYTES);
        let ok = parse_shot(&json!({
            "url": "http://127.0.0.1/",
            "localStorage": [{ "key": "token", "value": value }]
        }))
        .expect("8 KB value");
        assert_eq!(ok.local_storage.len(), 1);
        assert_eq!(ok.local_storage[0].key, "token");
        assert!(ok.session_storage.is_empty());
        assert!(ok.cookies.is_empty());

        let over = "y".repeat(super::MAX_VALUE_BYTES + 1);
        let too_big = parse_shot(&json!({
            "url": "http://127.0.0.1/",
            "localStorage": [{ "key": "token", "value": over }]
        }));
        assert!(too_big.unwrap_err().contains("over 8 KB"));

        let items: Vec<_> = (0..51)
            .map(|index| json!({ "key": format!("k{index}"), "value": "v" }))
            .collect();
        let too_many = parse_shot(&json!({
            "url": "http://127.0.0.1/",
            "sessionStorage": items
        }));
        assert!(too_many.unwrap_err().contains("more than 50"));

        let scheme = parse_shot(&json!({ "url": "file:///tmp/a.html" }));
        assert!(scheme.unwrap_err().contains("http and https"));
    }

    #[test]
    fn storage_script_sets_items_before_page_script() {
        let script = storage_script(
            &[StorageItem {
                key: "a\"b".into(),
                value: "x".into(),
            }],
            &[StorageItem {
                key: "sid".into(),
                value: "1".into(),
            }],
        );
        assert!(script.starts_with("(function(){try{"));
        assert!(script.contains("localStorage.setItem("));
        assert!(script.contains("sessionStorage.setItem("));
        assert!(script.contains("\\\""));
        assert!(!script.contains("document.cookie"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a Linux desktop session"]
    fn slow_paint_storage_and_cookie() {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            panic!("no display");
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            for _ in 0..12 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 4096];
                let n = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let color = if req.contains("sid=in") {
                    "#28a05a"
                } else {
                    "#c82828"
                };
                let body = format!(
                    "<!doctype html><html><body style=\"margin:0;background:#ffffff\"><script>document.addEventListener(\"DOMContentLoaded\",function(){{setTimeout(function(){{var token=\"\";try{{token=localStorage.getItem(\"token\")||\"\"}}catch(e){{}}document.body.style.background=token===\"in\"?\"#1464c8\":\"{color}\";}},800);}});</script></body></html>"
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
            }
        });
        gtk::init().expect("gtk");
        let url = format!("http://{addr}/");
        let logged_out = center_rgb(&grab(sample_shot(&url, false, false)).expect("logged out"));
        let cookie = center_rgb(&grab(sample_shot(&url, false, true)).expect("cookie"));
        let token = center_rgb(&grab(sample_shot(&url, true, false)).expect("token"));
        assert!(near(logged_out, (200, 40, 40)), "logged out {logged_out:?}");
        assert!(near(cookie, (40, 160, 90)), "cookie {cookie:?}");
        assert!(near(token, (20, 100, 200)), "token {token:?}");
    }

    #[cfg(target_os = "linux")]
    fn sample_shot(url: &str, token: bool, cookie: bool) -> super::ShotArgs {
        super::ShotArgs {
            url: url.to_string(),
            width: 640,
            height: 480,
            selector: None,
            wait_ms: 200,
            local_storage: if token {
                vec![StorageItem {
                    key: "token".into(),
                    value: "in".into(),
                }]
            } else {
                Vec::new()
            },
            session_storage: Vec::new(),
            cookies: if cookie {
                vec![super::CookieInput {
                    name: "sid".into(),
                    value: "in".into(),
                    domain: "127.0.0.1".into(),
                    path: "/".into(),
                    secure: false,
                    http_only: true,
                }]
            } else {
                Vec::new()
            },
        }
    }

    #[cfg(target_os = "linux")]
    fn grab(shot: super::ShotArgs) -> Result<Vec<u8>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        super::shoot(shot, tx)?;
        let slot = std::rc::Rc::new(std::cell::RefCell::new(Some(rx)));
        let result = std::rc::Rc::new(std::cell::RefCell::new(None));
        let slot_tick = slot.clone();
        let result_tick = result.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let mut guard = slot_tick.borrow_mut();
            let Some(rx) = guard.as_mut() else {
                return glib::ControlFlow::Break;
            };
            match rx.try_recv() {
                Ok(value) => {
                    *result_tick.borrow_mut() = Some(value);
                    gtk::main_quit();
                    glib::ControlFlow::Break
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    glib::ControlFlow::Continue
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    *result_tick.borrow_mut() = Some(Err("page_shot capture was dropped.".into()));
                    gtk::main_quit();
                    glib::ControlFlow::Break
                }
            }
        });
        glib::timeout_add_local(std::time::Duration::from_secs(20), || {
            gtk::main_quit();
            glib::ControlFlow::Break
        });
        gtk::main();
        let value = result.borrow_mut().take();
        value.unwrap_or_else(|| Err("page_shot live check timed out.".into()))
    }

    #[cfg(target_os = "linux")]
    fn center_rgb(png: &[u8]) -> (u8, u8, u8) {
        let mut image = cairo::ImageSurface::create_from_png(&mut &png[..]).expect("png");
        let width = image.width().max(1) as usize;
        let height = image.height().max(1) as usize;
        let stride = image.stride() as usize;
        let data = image.data().expect("pixels");
        let offset = (height / 2) * stride + (width / 2) * 4;
        (data[offset + 2], data[offset + 1], data[offset])
    }

    #[cfg(target_os = "linux")]
    fn near(pixel: (u8, u8, u8), expect: (u8, u8, u8)) -> bool {
        pixel.0.abs_diff(expect.0) < 30
            && pixel.1.abs_diff(expect.1) < 30
            && pixel.2.abs_diff(expect.2) < 30
    }
}
