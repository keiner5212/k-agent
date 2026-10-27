//! One page capture per platform.
//!
//! Linux uses WebKit through GTK, Windows uses WebView2. The tool layer only
//! knows the shot description and the PNG bytes that come back.

use serde_json::Value;

const DEFAULT_WIDTH: i32 = 1280;
const DEFAULT_HEIGHT: i32 = 720;
const MAX_ITEMS: usize = 50;
const MAX_VALUE_BYTES: usize = 8 * 1024;

/// A capture that shows no pixels is a miss, not the page.
pub const BLANK_FRAME: &str = "page_shot captured a blank frame. A blank frame is not the page.";

#[derive(Clone, Debug)]
pub struct StorageItem {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct CookieInput {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
}

#[derive(Clone, Debug)]
pub struct ShotArgs {
    pub url: String,
    pub width: i32,
    pub height: i32,
    pub selector: Option<String>,
    pub wait_ms: u64,
    pub local_storage: Vec<StorageItem>,
    pub session_storage: Vec<StorageItem>,
    pub cookies: Vec<CookieInput>,
}

pub fn parse_shot(args: &Value) -> Result<ShotArgs, String> {
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

pub fn check_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| "page_shot URL is not parseable.".to_string())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("page_shot only accepts http and https URLs.".into());
    }
    Ok(parsed)
}

pub fn clamp_i32(value: Option<i64>, default_value: i32, min: i32, max: i32) -> i32 {
    let Some(value) = value else {
        return default_value;
    };
    (value as i32).clamp(min, max)
}

pub fn parse_pairs(args: &Value, field: &str) -> Result<Vec<StorageItem>, String> {
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

pub fn parse_cookies(args: &Value, host: &str) -> Result<Vec<CookieInput>, String> {
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

pub fn optional_bool(item: &Value, field: &str, index: usize) -> Result<bool, String> {
    match item.get(field) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!(
            "page_shot cookies[{index}] {field} must be a boolean."
        )),
    }
}

pub fn cookie_domain_value(domain: &str, host: &str) -> String {
    let bare = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    if bare.eq_ignore_ascii_case(host) {
        bare
    } else {
        format!(".{bare}")
    }
}

pub fn cookie_domain_matches(domain: &str, host: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || host.is_empty() {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return host == domain;
    }
    if domain != host && !domain.contains('.') {
        return false;
    }
    host == domain
        || (host.len() > domain.len()
            && host.as_bytes().get(host.len() - domain.len() - 1) == Some(&b'.')
            && host.ends_with(&domain))
}

pub fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

pub fn storage_script(local: &[StorageItem], session: &[StorageItem]) -> String {
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

pub fn origin_allows(url: &reqwest::Url) -> Vec<String> {
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

/// Capture one viewport of a page and return the PNG bytes.
pub async fn capture(app: &tauri::AppHandle, shot: &ShotArgs) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "linux")]
    {
        webkit::capture(app, shot).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, shot);
        Err(format!(
            "page_shot needs WebKit and runs on Linux only. This host is {}.",
            crate::compat::os_name()
        ))
    }
}

#[cfg(target_os = "linux")]
mod webkit;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cookie_domain_must_match_host() {
        assert!(cookie_domain_matches("example.com", "example.com"));
        assert!(cookie_domain_matches(".example.com", "app.example.com"));
        assert!(!cookie_domain_matches("evil.com", "app.example.com"));
        assert!(!cookie_domain_matches("com", "app.example.com"));
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
        let value = "y".repeat(MAX_VALUE_BYTES);
        let ok = parse_shot(&json!({
            "url": "http://127.0.0.1/",
            "localStorage": [{ "key": "token", "value": value }]
        }))
        .expect("8 KB value");
        assert_eq!(ok.local_storage.len(), 1);
        assert_eq!(ok.local_storage[0].key, "token");
        assert!(ok.session_storage.is_empty());
        assert!(ok.cookies.is_empty());

        let over = "y".repeat(MAX_VALUE_BYTES + 1);
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
}
