//! Linux capture through WebKitGTK.

use std::time::Duration;

const READY_POLL_MS: u64 = 250;
const READY_BUDGET_MS: u64 = 10_000;

use serde_json::Value;

use super::{check_url, origin_allows, storage_script, CookieInput, ShotArgs, BLANK_FRAME};

struct ShotState {
    _window: gtk::Window,
    tx: Option<tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>>,
}

pub async fn capture(app: &tauri::AppHandle, shot: &ShotArgs) -> Result<Vec<u8>, String> {
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

fn surface_blank(surface: cairo::Surface) -> bool {
    use std::convert::TryFrom;

    let Ok(mut image) = cairo::ImageSurface::try_from(surface) else {
        return true;
    };
    surface_is_blank(&mut image)
}

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
    if w < 2 || h < 2 {
        // A one pixel crop is a capture miss, not the page.
        return Err(BLANK_FRAME.into());
    }
    encode_crop(&image, x, y, w, h)
}

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
        if !(12..=246).contains(&l) {
            near_flat += 1;
        }
    }
    let spread = max_l.saturating_sub(min_l);
    near_flat * 100 / pixels.len() as u32 >= 97 && spread < 18
}

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
    use super::*;
    use crate::browser::{ShotArgs, StorageItem};

    #[test]
    fn solid_black_or_white_is_blank() {
        assert!(frame_is_blank(&[(0, 0, 0); 40]));
        assert!(frame_is_blank(&[(255, 255, 255); 40]));
    }

    #[test]
    fn painted_page_is_kept() {
        let mut pixels = vec![(246, 241, 231); 30];
        pixels.push((40, 28, 22));
        pixels.push((120, 72, 48));
        assert!(!frame_is_blank(&pixels));
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
    fn sample_shot(url: &str, token: bool, cookie: bool) -> ShotArgs {
        ShotArgs {
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
    fn grab(shot: ShotArgs) -> Result<Vec<u8>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        shoot(shot, tx)?;
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
