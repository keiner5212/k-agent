use std::time::Duration;

use serde_json::Value;

const MAX_URL_LEN: usize = 4096;
const MAX_BODY_LEN: usize = 1_000_000;
const MAX_RESPONSE_LEN: usize = 200_000;
const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const MAX_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug)]
pub struct HttpCall {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub struct HttpResult {
    pub final_url: String,
    pub status: u16,
    pub headers: String,
    pub body: String,
    pub truncated: bool,
}

pub fn normalize_method(raw: &str) -> Result<String, String> {
    let method = raw.trim().to_ascii_uppercase();
    if matches!(
        method.as_str(),
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    ) {
        return Ok(method);
    }
    Err(format!("http_request method `{raw}` is not supported."))
}

pub fn pairs_from_object(
    value: Option<&Value>,
    label: &str,
) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(object) = value.as_object() else {
        return Err(format!("http_request `{label}` must be an object."));
    };
    let mut pairs = Vec::new();
    for (key, item) in object {
        let text = match item {
            Value::String(text) => text.clone(),
            Value::Null => continue,
            other => other.to_string(),
        };
        if key.trim().is_empty() {
            return Err(format!("http_request `{label}` has an empty name."));
        }
        pairs.push((key.clone(), text));
    }
    Ok(pairs)
}

pub async fn send(call: HttpCall) -> Result<HttpResult, String> {
    if call.url.trim().is_empty() {
        return Err("http_request requires a non-empty `url`.".into());
    }
    if call.url.len() > MAX_URL_LEN {
        return Err("http_request URL is too long.".into());
    }
    let mut parsed = reqwest::Url::parse(call.url.trim())
        .map_err(|_| "http_request URL is not parseable.".to_string())?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("http_request only accepts http and https URLs.".into());
    }
    if let Some(body) = &call.body {
        if body.len() > MAX_BODY_LEN {
            return Err("http_request body exceeds 1 MB.".into());
        }
    }
    for (name, value) in &call.query {
        parsed.query_pairs_mut().append_pair(name, value);
    }
    let timeout = call.timeout_ms.clamp(1_000, MAX_TIMEOUT_MS);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_millis(timeout))
        .build()
        .map_err(|error| format!("http_request client: {error}"))?;
    let method = reqwest::Method::from_bytes(call.method.as_bytes())
        .map_err(|_| format!("http_request method `{}` is invalid.", call.method))?;
    let mut request = client.request(method, parsed);
    for (name, value) in &call.headers {
        request = request.header(name, value);
    }
    if !matches!(call.method.as_str(), "GET" | "HEAD") {
        if let Some(body) = call.body {
            request = request.body(body);
        }
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("http_request failed: {error}"))?;
    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let mut headers = String::new();
    for (name, value) in response.headers() {
        if !headers.is_empty() {
            headers.push('\n');
        }
        headers.push_str(&format!("{name}: {}", value.to_str().unwrap_or("")));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("http_request read body: {error}"))?;
    let truncated = bytes.len() > MAX_RESPONSE_LEN;
    let slice = if truncated {
        &bytes[..MAX_RESPONSE_LEN]
    } else {
        &bytes
    };
    Ok(HttpResult {
        final_url,
        status,
        headers,
        body: String::from_utf8_lossy(slice).to_string(),
        truncated,
    })
}

pub fn timeout_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1_000, MAX_TIMEOUT_MS)
}
