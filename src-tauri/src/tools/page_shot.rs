use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};

use super::{
    context_error, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "page_shot";

const DESCRIPTION: &str = "Capture a hidden WebKit view of one http or https page and return a PNG. Set width and height for the window. Set selector to crop to that element. The page is not shown; only the image is returned.";

const DEFAULT_WIDTH: i32 = 1280;
const DEFAULT_HEIGHT: i32 = 720;

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
                    "height": { "type": "integer", "description": "Viewport height in pixels. Default 720. Range 240-1200." },
                    "selector": { "type": "string", "description": "Optional CSS selector. When set, the image is only that element." },
                    "waitMs": { "type": "integer", "description": "Extra wait after load before capture. Default 300. Max 5000." }
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
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return context_error(None, "page_shot requires a non-empty `url`.");
    }
    if let Err(message) = check_url(&url) {
        return context_error(None, &message);
    }
    let width = clamp_i32(
        args.get("width").and_then(Value::as_i64),
        DEFAULT_WIDTH,
        320,
        1600,
    );
    let height = clamp_i32(
        args.get("height").and_then(Value::as_i64),
        DEFAULT_HEIGHT,
        240,
        1200,
    );
    let selector = args
        .get("selector")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let wait_ms = args
        .get("waitMs")
        .and_then(Value::as_u64)
        .unwrap_or(300)
        .min(5_000);
    let Some(app) = ctx.app else {
        return context_error(None, "page_shot requires the desktop shell.");
    };
    let png = match capture(app, &url, width, height, selector.as_deref(), wait_ms).await {
        Ok(png) => png,
        Err(message) => return context_error(None, &message),
    };
    let encoded = BASE64.encode(&png);
    let selector_label = selector.unwrap_or_default();
    ToolOutcome {
        text: toon_doc(&[
            ("url", ToonValue::Str(&url)),
            ("width", ToonValue::Int(width as i64)),
            ("height", ToonValue::Int(height as i64)),
            ("selector", ToonValue::Str(&selector_label)),
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
    }
}

fn check_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| "page_shot URL is not parseable.".to_string())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("page_shot only accepts http and https URLs.".into());
    }
    Ok(())
}

fn clamp_i32(value: Option<i64>, default_value: i32, min: i32, max: i32) -> i32 {
    let Some(value) = value else {
        return default_value;
    };
    (value as i32).clamp(min, max)
}

async fn capture(
    app: &tauri::AppHandle,
    url: &str,
    width: i32,
    height: i32,
    selector: Option<&str>,
    wait_ms: u64,
) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "linux")]
    {
        capture_linux(app, url, width, height, selector, wait_ms).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, url, width, height, selector, wait_ms);
        Err("page_shot currently requires Linux WebKit.".into())
    }
}

#[cfg(target_os = "linux")]
struct ShotState {
    _window: gtk::Window,
    tx: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>>,
}

#[cfg(target_os = "linux")]
async fn capture_linux(
    app: &tauri::AppHandle,
    url: &str,
    width: i32,
    height: i32,
    selector: Option<&str>,
    wait_ms: u64,
) -> Result<Vec<u8>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let url = url.to_string();
    let selector = selector.map(str::to_string);
    let app = app.clone();
    app.run_on_main_thread(move || {
        let _ = shoot(&url, width, height, selector, wait_ms, tx);
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
    url: &str,
    width: i32,
    height: i32,
    selector: Option<String>,
    wait_ms: u64,
    tx: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
) -> Result<(), String> {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::prelude::*;
    use gtk::prelude::WidgetExtManual;
    use webkit2gtk::WebViewExt;
    use webkit2gtk::{LoadEvent, WebView};

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_accept_focus(false);
    window.set_keep_below(true);
    window.set_type_hint(gdk::WindowTypeHint::Utility);
    window.set_default_size(width, height);
    window.move_(-16000, -16000);
    let view = WebView::new();
    window.add(&view);
    window.show_all();
    view.set_size_request(width, height);
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
    let started = std::cell::Cell::new(false);
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
        started.set(true);
        let view = view.clone();
        let selector = selector.clone();
        let state = state.clone();
        glib::timeout_add_local(Duration::from_millis(wait_ms), move || {
            queue_snapshot(view.clone(), selector.clone(), state.clone());
            glib::ControlFlow::Break
        });
    });
    view.load_uri(url);
    Ok(())
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

    let image = ImageSurface::try_from(surface)
        .map_err(|_| "page_shot snapshot was not an image.".to_string())?;
    let (x, y, w, h) = rect.unwrap_or((0, 0, image.width(), image.height()));
    encode_crop(&image, x, y, w, h)
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
