use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::utils::user_agent::user_agent_for_week;

use super::{toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue};

pub const NAME: &str = "fetch_url";

const DESCRIPTION: &str = "Fetch one public HTTPS page and return its title, description, main text, and safe outbound links. Uses a stable Chrome desktop profile (Chrome 153) with realistic pacing, manual redirect handling, host-only cookies, and SSRF guards.";

const FALLBACK_CHROME_MAJOR: &str = "153";
const FALLBACK_CHROME_FULL_VERSION: &str = "153.0.6903.58";

const MAX_BYTES: usize = 1_500_000;
const MAX_TITLE_LENGTH: usize = 300;
const MAX_DESCRIPTION_LENGTH: usize = 500;
const MAX_CONTENT_LENGTH: usize = 16_000;
const MAX_LINK_TEXT_LENGTH: usize = 200;
const MAX_LINKS: usize = 20;
const TIMEOUT_MS: u64 = 12_000;
const MAX_HOPS: usize = 4;
const CACHE_TTL_SECS: u64 = 7 * 86_400;

const ACCESS_DENIED_STATUS: &[u16] = &[401, 402, 403, 407, 451];
const RETRYABLE_STATUS: &[u16] = &[408, 429, 500, 502, 503, 504];

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "localhost.localdomain",
    "metadata.google.internal",
    "metadata.google.com",
    "instance-data",
    "kubernetes.default",
    "kubernetes.default.svc",
];

const BLOCKED_SUFFIXES: &[&str] = &[
    ".localhost",
    ".local",
    ".internal",
    ".localdomain",
    ".intranet",
    ".lan",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchArgs {
    url: String,
    #[serde(default)]
    lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResponse {
    title: String,
    description: String,
    content: String,
    links: Vec<CachedLink>,
    cached_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedLink {
    href: String,
    text: String,
}

pub struct FetchUrlTool;

impl Tool for FetchUrlTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Public HTTPS URL to fetch. Only port 443 is accepted. Credentials, IP literals, and private or metadata hostnames are refused."
                    },
                    "lang": {
                        "type": "string",
                        "description": "Optional BCP 47 language tag (for example en-US, es-ES). Defaults to en-US. Sets the Accept-Language header."
                    }
                },
                "required": ["url"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        match parse_args(args) {
            Ok(_) => super::context_error(
                None,
                "fetch_url must run on the async dispatch path.",
            ),
            Err(message) => super::context_error(None, &message),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args = match parse_args_str(arguments) {
        Ok(args) => args,
        Err(message) => return super::context_error(None, &message),
    };
    match fetch(&args, ctx).await {
        Ok(result) => ToolOutcome {
            text: result,
            display: ToolDisplay {
                kind: super::TOOL_KIND_CONTEXT.to_string(),
                ..ToolDisplay::default()
            },
            snapshot: None,
        },
        Err(message) => super::context_error(Some(&args.url), &message),
    }
}

async fn fetch(args: &FetchArgs, _ctx: &ToolContext<'_>) -> Result<String, String> {
    let canonical = canonicalize_url(&args.url)?;
    if let Some(cached) = read_cache(&canonical) {
        return Ok(render(&canonical, &cached));
    }
    let session = BrowserSession::new(args.lang.as_deref().unwrap_or("en-US"));
    let fetched = session.get(&canonical, "none", None).await?;
    let parsed = ContentParser::parse(&fetched.html, &fetched.final_url);
    let cached = CachedResponse {
        title: parsed.title.clone(),
        description: parsed.description.clone(),
        content: parsed.content.clone(),
        links: parsed
            .links
            .iter()
            .map(|link| CachedLink {
                href: link.href.clone(),
                text: link.text.clone(),
            })
            .collect(),
        cached_at_secs: now_secs(),
    };
    write_cache(&canonical, &cached);
    Ok(render(&canonical, &cached))
}

fn render(url: &str, body: &CachedResponse) -> String {
    let mut fields: Vec<(&str, ToonValue<'_>)> = Vec::new();
    fields.push(("url", ToonValue::Str(url)));
    fields.push(("title", ToonValue::Str(&body.title)));
    if !body.description.is_empty() {
        fields.push(("description", ToonValue::Str(&body.description)));
    }
    fields.push(("content", ToonValue::Block(&body.content)));
    let mut links = String::new();
    for (index, link) in body.links.iter().enumerate() {
        if index > 0 {
            links.push('\n');
        }
        links.push_str(&format!("- {} ({})", link.text, link.href));
    }
    fields.push(("links", ToonValue::Block(&links)));
    toon_doc(&fields)
}

pub fn canonicalize_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(r"fetch_url requires a non-empty `url`.".into());
    }
    if trimmed.len() > 2048 {
        return Err("fetch_url URL exceeds 2048 chars.".into());
    }
    if trimmed != raw {
        return Err("fetch_url URL has leading or trailing whitespace.".into());
    }
    if raw.chars().any(|c| c.is_control()) || raw.contains('\\') {
        return Err("fetch_url URL contains control characters.".into());
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| "fetch_url URL is not parseable.".to_string())?;
    if parsed.scheme() != "https" {
        return Err("fetch_url only accepts HTTPS URLs.".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("fetch_url URL must not carry credentials.".into());
    }
    if parsed.port().is_some() {
        return Err("fetch_url only accepts the default HTTPS port (443).".into());
    }
    let host = bare_host(parsed.host_str().unwrap_or(""));
    if host.is_empty() || host.contains('%') {
        return Err("fetch_url URL has an invalid hostname.".into());
    }
    if is_blocked_host(&host) {
        return Err("fetch_url refused: internal or private hostname.".into());
    }
    if is_ip_literal(&host) {
        return Err("fetch_url refused: IP literals are not allowed.".into());
    }
    Ok(parsed.as_str().to_string())
}

fn bare_host(hostname: &str) -> String {
    let lower = hostname.to_lowercase();
    let trimmed = lower.trim_end_matches('.');
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_blocked_host(host: &str) -> bool {
    if BLOCKED_HOSTS.iter().any(|blocked| *blocked == host) {
        return true;
    }
    BLOCKED_SUFFIXES.iter().any(|suffix| {
        host == suffix.trim_start_matches('.') || host.ends_with(suffix)
    })
}

fn is_ip_literal(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    if host.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if let Some(rest) = host.strip_prefix("0x").or_else(|| host.strip_prefix("0X")) {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
    }
    if !host.is_empty() && host.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return true;
    }
    false
}

#[allow(dead_code)]
fn is_private_address(address: &str) -> bool {
    let cleaned = address.split('%').next().unwrap_or(address);
    match cleaned.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => is_private_v4(v4),
        Ok(IpAddr::V6(v6)) => is_private_v6(v6),
        Err(_) => true,
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let [a, b, c] = [octets[0], octets[1], octets[2]];
    if a == 0 || a == 10 || a == 127 {
        return true;
    }
    if a == 169 && b == 254 {
        return true;
    }
    if a == 172 && (16..=31).contains(&b) {
        return true;
    }
    if a == 192 && b == 168 {
        return true;
    }
    if a == 192 && b == 0 && (c == 0 || c == 2) {
        return true;
    }
    if a == 100 && (64..=127).contains(&b) {
        return true;
    }
    if a == 198 && (b == 18 || b == 19) {
        return true;
    }
    if a == 198 && b == 51 && c == 100 {
        return true;
    }
    if a == 203 && b == 0 && c == 113 {
        return true;
    }
    if a >= 224 {
        return true;
    }
    false
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    let segments = ip.segments();
    let head = segments[0];
    if (0xfc00..=0xfdff).contains(&head) {
        return true;
    }
    if (0xfe80..=0xfeff).contains(&head) {
        return true;
    }
    if head >= 0xff00 {
        return true;
    }
    if head == 0x2001 && segments[1] == 0x0db8 {
        return true;
    }
    false
}

pub fn allows_result_url(raw: &str) -> bool {
    canonicalize_url(raw).is_ok()
}

#[derive(Debug)]
pub struct FetchedPage {
    pub html: String,
    pub final_url: String,
}

pub struct BrowserSession {
    client: reqwest::Client,
    lang: String,
}

impl BrowserSession {
    pub fn new(lang: &str) -> Self {
        let user_agent = user_agent_for_week(std::time::SystemTime::now());
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_millis(TIMEOUT_MS))
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .build()
            .expect("reqwest client must build");
        Self {
            client,
            lang: lang.to_string(),
        }
    }

    pub async fn get(&self, url: &str, site: &'static str, referer: Option<String>) -> Result<FetchedPage, String> {
        let canonical = canonicalize_url(url)?;
        let mut current = canonical;
        let mut current_site: &'static str = site;
        let mut current_referer = referer;
        let mut retries_left: u8 = 1;
        let mut hops = 0usize;
        loop {
            if hops > MAX_HOPS {
                return Err("fetch_url exceeded the redirect hop limit.".into());
            }
            hops += 1;
            let delay_ms: u64 = 50 + (random_ms() % 200) as u64;
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let request = self
                .client
                .get(&current)
                .headers(navigation_headers(current_site, current_referer.as_deref(), &self.lang));
            let response = match request.send().await {
                Ok(resp) => resp,
                Err(error) => {
                    if error.is_timeout() {
                        if retries_left > 0 {
                            retries_left -= 1;
                            let backoff_ms: u64 = 1500 + (random_ms() % 1500) as u64;
                            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                            continue;
                        }
                        return Err("fetch_url request timed out.".into());
                    }
                    return Err(format!("fetch_url request failed: {error}"));
                }
            };
            let status = response.status().as_u16();
            if RETRYABLE_STATUS.contains(&status) {
                if retries_left > 0 {
                    retries_left -= 1;
                    let backoff_ms: u64 = 1500 + (random_ms() % 1500) as u64;
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    continue;
                }
                if status == 429 {
                    return Err("fetch_url rate limited.".into());
                }
                return Err("fetch_url page temporarily unavailable.".into());
            }
            if (300..400).contains(&status) {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let Some(location) = location else {
                    return Err("fetch_url redirect had no Location.".into());
                };
                let next = reqwest::Url::parse(&location)
                    .or_else(|_| reqwest::Url::parse(&current).and_then(|base| base.join(&location)))
                    .map_err(|_| "fetch_url redirect target was unparseable.")?;
                let next_canonical = canonicalize_url(next.as_str())?;
                let next_parsed = reqwest::Url::parse(&next_canonical)
                    .map_err(|_| "fetch_url redirect target URL.".to_string())?;
                let host = bare_host(
                    reqwest::Url::parse(&current)
                        .map_err(|_| "fetch_url redirect base URL.".to_string())?
                        .host_str()
                        .unwrap_or(""),
                );
                let next_host = bare_host(next_parsed.host_str().unwrap_or(""));
                let same_host = next_host == host;
                let referer = if same_host {
                    Some(current.clone())
                } else {
                    let origin = reqwest::Url::parse(&current)
                        .ok()
                        .map(|u| {
                            let scheme = u.scheme();
                            let host = u.host_str().unwrap_or("");
                            format!("{scheme}://{host}")
                        })
                        .filter(|s| !s.is_empty());
                    origin.map(|origin| format!("{origin}/"))
                };
                current = next_canonical;
                current_site = if same_host { "same-origin" } else { "cross-site" };
                current_referer = referer;
                continue;
            }
            if ACCESS_DENIED_STATUS.contains(&status) {
                return Err("fetch_url page refused the request.".into());
            }
            if !(200..300).contains(&status) {
                return Err(format!("fetch_url HTTP {status}."));
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_lowercase();
            if !content_type.contains("text/html") && !content_type.contains("application/xhtml+xml") {
                return Err("fetch_url page is not HTML.".into());
            }
            let bytes = response
                .bytes()
                .await
                .map_err(|error| format!("fetch_url read body: {error}"))?;
            if bytes.len() > MAX_BYTES {
                return Err("fetch_url response exceeded 1.5 MB.".into());
            }
            let html = String::from_utf8_lossy(&bytes).to_string();
            if html.trim().len() < 20 {
                return Err("fetch_url response body was empty.".into());
            }
            return Ok(FetchedPage {
                html,
                final_url: current,
            });
        }
    }
}

fn random_ms() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

fn navigation_headers(
    site: &'static str,
    referer: Option<&str>,
    lang: &str,
) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let user_agent = user_agent_for_week(std::time::SystemTime::now());
    let (chrome_major, chrome_full) = ua_versions(&user_agent);
    let sec_ch_ua = format!(
        "\"Google Chrome\";v=\"{}\", \"Chromium\";v=\"{}\", \"Not.A/Brand\";v=\"24\"",
        chrome_major, chrome_major
    );
    let sec_ch_ua_full = format!(
        "\"Google Chrome\";v=\"{}\", \"Chromium\";v=\"{}\", \"Not.A/Brand\";v=\"24.0.0.0\"",
        chrome_full, chrome_full
    );
    let sec_ch_ua_platform = platform_token(&user_agent);
    let accept_language = if lang == "en" {
        "en,en;q=0.9".to_string()
    } else {
        let primary = lang.split('-').next().unwrap_or(lang);
        if primary == "en" {
            format!("{lang},en;q=0.9")
        } else {
            format!("{lang},{primary};q=0.9,en;q=0.8")
        }
    };
    insert(&mut headers, "sec-ch-ua", &sec_ch_ua);
    insert(&mut headers, "sec-ch-ua-mobile", "?0");
    insert(&mut headers, "sec-ch-ua-platform", &sec_ch_ua_platform);
    insert(&mut headers, "sec-ch-ua-platform-version", "\"15.0.0\"");
    insert(&mut headers, "sec-ch-ua-arch", "\"x86\"");
    insert(&mut headers, "sec-ch-ua-bitness", "\"64\"");
    insert(&mut headers, "sec-ch-ua-model", "");
    insert(&mut headers, "sec-ch-ua-full-version-list", &sec_ch_ua_full);
    insert(&mut headers, "sec-gpc", "1");
    insert(&mut headers, "dnt", "1");
    insert(&mut headers, "upgrade-insecure-requests", "1");
    insert(&mut headers, "user-agent", &user_agent);
    insert(
        &mut headers,
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
    );
    insert(&mut headers, "sec-fetch-site", site);
    insert(&mut headers, "sec-fetch-mode", "navigate");
    insert(&mut headers, "sec-fetch-user", "?1");
    insert(&mut headers, "sec-fetch-dest", "document");
    insert(&mut headers, "accept-language", &accept_language);
    insert(&mut headers, "accept-encoding", "gzip, deflate, br, zstd");
    insert(&mut headers, "priority", "u=0, i");
    insert(&mut headers, "cache-control", "no-cache");
    insert(&mut headers, "pragma", "no-cache");
    if let Some(referer) = referer {
        insert(&mut headers, "referer", referer);
    }
    headers
}

/// Extract the major Chrome version and full Chrome version from a UA
/// string of the shape `... Chrome/X.Y.Z.W Safari/...`. Returns the hard
/// fallback if Chrome is not present (rare - all generated UAs include it).
fn ua_versions(user_agent: &str) -> (String, String) {
    let after_chrome = match user_agent.split_once("Chrome/") {
        Some((_, rest)) => rest,
        None => return (FALLBACK_CHROME_MAJOR.to_string(), FALLBACK_CHROME_FULL_VERSION.to_string()),
    };
    let version = after_chrome.split_whitespace().next().unwrap_or("");
    let major = version
        .split('.')
        .next()
        .unwrap_or(FALLBACK_CHROME_MAJOR)
        .to_string();
    if major.is_empty() {
        (FALLBACK_CHROME_MAJOR.to_string(), FALLBACK_CHROME_FULL_VERSION.to_string())
    } else {
        (major.clone(), version.to_string())
    }
}

fn platform_token(user_agent: &str) -> String {
    if user_agent.contains("Windows NT") {
        "\"Windows\"".to_string()
    } else if user_agent.contains("Macintosh") {
        "\"macOS\"".to_string()
    } else if user_agent.contains("Linux") {
        "\"Linux\"".to_string()
    } else if user_agent.contains("X11") {
        "\"Linux\"".to_string()
    } else {
        "\"Windows\"".to_string()
    }
}

fn insert(headers: &mut reqwest::header::HeaderMap, key: &'static str, value: &str) {
    let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) else {
        return;
    };
    let Ok(value) = reqwest::header::HeaderValue::from_str(value) else {
        return;
    };
    headers.insert(name, value);
}

fn parse_args(args: &Value) -> Result<(), String> {
    let url = args.get("url").and_then(Value::as_str).unwrap_or("");
    if url.is_empty() {
        return Err(r"fetch_url requires a string `url`.".into());
    }
    if let Some(lang) = args.get("lang").and_then(Value::as_str) {
        if lang.len() > 35 {
            return Err(r"fetch_url `lang` exceeds 35 chars.".into());
        }
        if lang.chars().any(|c| c.is_control()) {
            return Err(r"fetch_url `lang` contains control characters.".into());
        }
    }
    Ok(())
}

fn parse_args_str(arguments: &str) -> Result<FetchArgs, String> {
    let value: Value =
        serde_json::from_str(arguments).map_err(|_| "fetch_url arguments must be a JSON object.".to_string())?;
    parse_args(&value)?;
    let url = value
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| r"fetch_url requires a string `url`.".to_string())?
        .to_string();
    let lang = value.get("lang").and_then(Value::as_str).map(str::to_string);
    Ok(FetchArgs { url, lang })
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_root() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let path = home.join(".k-agent").join("cache").join("fetch-url");
    let _ = std::fs::create_dir_all(&path);
    Some(path)
}

fn cache_key(canonical: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:016x}.json", hasher.finish())
}

fn read_cache(canonical: &str) -> Option<CachedResponse> {
    let root = cache_root()?;
    let path = root.join(cache_key(canonical));
    let raw = std::fs::read_to_string(&path).ok()?;
    let cached: CachedResponse = serde_json::from_str(&raw).ok()?;
    if now_secs().saturating_sub(cached.cached_at_secs) > CACHE_TTL_SECS {
        return None;
    }
    Some(cached)
}

fn write_cache(canonical: &str, cached: &CachedResponse) {
    let Some(root) = cache_root() else { return };
    let path = root.join(cache_key(canonical));
    if let Ok(text) = serde_json::to_string(cached) {
        let _ = std::fs::write(&path, text);
    }
}

pub struct ContentParser;

#[derive(Debug, Default)]
pub struct ParsedPage {
    pub title: String,
    pub description: String,
    pub content: String,
    pub links: Vec<ParsedLink>,
}

#[derive(Debug, Clone)]
pub struct ParsedLink {
    pub href: String,
    pub text: String,
}

impl ContentParser {
    pub fn parse(html: &str, base_url: &str) -> ParsedPage {
        if html.trim().is_empty() {
            return ParsedPage::default();
        }
        let title = Self::extract_title(html);
        let description = Self::extract_description(html);
        let cleaned = Self::remove_noise_elements(html);
        let content = Self::extract_main_content(&cleaned);
        let links = Self::extract_links(&cleaned, base_url);
        ParsedPage {
            title,
            description,
            content,
            links,
        }
    }

    fn extract_title(html: &str) -> String {
        let social = meta_content(html, "property", "og:title");
        if !social.is_empty() {
            return truncate(&decode_text(&social), MAX_TITLE_LENGTH);
        }
        let from_tag = capture(html, "<title[^>]*>", "</title>")
            .or_else(|| capture(html, "<h1[^>]*>", "</h1>"))
            .unwrap_or_default();
        truncate(&html_to_text(&from_tag), MAX_TITLE_LENGTH)
    }

    fn extract_description(html: &str) -> String {
        let meta_name = meta_content(html, "name", "description");
        let meta_property = meta_content(html, "property", "og:description");
        let meta = if meta_name.len() > meta_property.len() { meta_name } else { meta_property };
        if meta.len() > 30 {
            return truncate(&meta, MAX_DESCRIPTION_LENGTH);
        }
        let cleaned = Self::remove_noise_elements(html);
        for paragraph in capture_all(&cleaned, "<p[^>]*>", "</p>") {
            let text = html_to_text(&paragraph).trim().to_string();
            if text.chars().count() >= 80 {
                return truncate(&text, MAX_DESCRIPTION_LENGTH);
            }
        }
        String::new()
    }

    fn find_main_content_block(html: &str) -> String {
        let id_patterns = [
            "main-content", "mw-content-text", "content", "article-content",
            "post-content", "story-content", "page-content", "article",
        ];
        for pattern in id_patterns {
            let regex = format!(r#"(?is)<[a-z]+[^>]+id=["']{pattern}["'][^>]*>(.*?)</[a-z]+>"#);
            if let Ok(re) = regex_compile(&regex) {
                if let Some(caps) = re.captures(html) {
                    if caps.get(1).map(|m| m.as_str().len()).unwrap_or(0) > 500 {
                        return caps[1].to_string();
                    }
                }
            }
        }
        let class_patterns = [
            "article-body", "article-content", "post-content", "story-content",
            "main-content", "entry-content", "page-content", "page-body",
            "content-body", "article", "post", "story",
        ];
        for pattern in class_patterns {
            let regex = format!(r#"(?is)<[a-z]+[^>]+class=["'][^"']*\b{pattern}\b[^"']*["'][^>]*>(.*?)</[a-z]+>"#);
            if let Ok(re) = regex_compile(&regex) {
                if let Some(caps) = re.captures(html) {
                    if let Some(body) = caps.get(1) {
                        if body.as_str().len() > 500 {
                            return body.as_str().to_string();
                        }
                    }
                }
            }
        }
        capture(html, "<body[^>]*>", "</body>").unwrap_or_else(|| html.to_string())
    }

    fn extract_main_content(html: &str) -> String {
        let mut content = Self::find_main_content_block(html);
        for pattern in [
            r"(?is)<table[^>]*>.*?</table>",
            r"(?is)<figure[^>]*>.*?</figure>",
            r#"(?is)<div[^>]+class=["'][^"']*(?:hatnote|thumb|navbox|metadata|toc|rellink|gallery|ambox|refbegin|portal|infobox)[^"']*["'][^>]*>.*?</div>"#,
            r"(?is)<aside[^>]*>.*?</aside>",
            r#"(?is)<section[^>]+class=["'][^"']*(?:related|recommended|sidebar|comments)[^"']*["'][^>]*>.*?</section>"#,
        ] {
            if let Ok(re) = regex_compile(pattern) {
                content = re.replace_all(&content, "").to_string();
            }
        }
        truncate(&html_to_text(&content), MAX_CONTENT_LENGTH)
    }

    fn remove_noise_elements(html: &str) -> String {
        let mut output = html.to_string();
        let patterns: &[&str] = &[
            r"(?s)<!--.*?-->",
            r"(?is)<script[^>]*>.*?</script>",
            r"(?is)<style[^>]*>.*?</style>",
            r"(?is)<noscript[^>]*>.*?</noscript>",
            r"(?is)<template[^>]*>.*?</template>",
            r"(?is)<nav[^>]*>.*?</nav>",
            r"(?is)<footer[^>]*>.*?</footer>",
            r"(?is)<header[^>]*>.*?</header>",
            r"(?is)<iframe[^>]*>.*?</iframe>",
            r#"(?is)<(div|section)[^>]+class=["'][^"']*(?:ad|ads|advert|advertisement|banner|popup|modal)[^"']*["'][^>]*>.*?</\1>"#,
            r"<[^>]+\bdata-[a-z-]+[^>]*>",
        ];
        for pattern in patterns {
            if let Ok(re) = regex_compile(pattern) {
                output = re.replace_all(&output, "").to_string();
            }
        }
        output
    }

    fn extract_links(html: &str, base_url: &str) -> Vec<ParsedLink> {
        // Rust's `regex` crate does not support backreferences, so we run
        // two passes: one for `href="..."` and one for `href='...'`.
        let re_double = match regex_compile(
            r#"(?is)<a\b[^>]*?\bhref="([^"]*)"[^>]*>([\s\S]*?)</a>"#,
        ) {
            Ok(re) => re,
            Err(_) => return Vec::new(),
        };
        let re_single = match regex_compile(
            r#"(?is)<a\b[^>]*?\bhref='([^']*)'[^>]*>([\s\S]*?)</a>"#,
        ) {
            Ok(re) => re,
            Err(_) => return Vec::new(),
        };
        let mut seen: HashMap<String, ParsedLink> = HashMap::new();
        for caps in re_double.captures_iter(html).chain(re_single.captures_iter(html)) {
            let href = decode_text(caps.get(1).map(|m| m.as_str()).unwrap_or(""))
                .trim()
                .to_string();
            let text = html_to_text(caps.get(2).map(|m| m.as_str()).unwrap_or(""));
            if href.is_empty() || href.starts_with('#') {
                continue;
            }
            if text.chars().count() < 3 {
                continue;
            }
            let resolved = if base_url.is_empty() {
                reqwest::Url::parse(&href).ok()
            } else {
                reqwest::Url::parse(base_url)
                    .ok()
                    .and_then(|base| base.join(&href).ok())
            };
            let Some(mut parsed) = resolved else { continue };
            parsed.set_fragment(None);
            if !allows_result_url(parsed.as_str()) {
                continue;
            }
            let entry = seen
                .entry(parsed.as_str().to_string())
                .or_insert_with(|| ParsedLink {
                    href: parsed.as_str().to_string(),
                    text: truncate(&text, MAX_LINK_TEXT_LENGTH),
                });
            if entry.text.chars().count() < text.chars().count() {
                entry.text = truncate(&text, MAX_LINK_TEXT_LENGTH);
            }
            if seen.len() >= MAX_LINKS {
                break;
            }
        }
        seen.into_values().collect()
    }
}

fn regex_compile(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(pattern)
}

fn meta_content(html: &str, attribute: &str, value: &str) -> String {
    let needle = value.to_lowercase();
    let tag_regex = match regex_compile(r"(?i)<meta\b[^>]*>") {
        Ok(re) => re,
        Err(_) => return String::new(),
    };
    for tag in tag_regex.find_iter(html) {
        let tag_str = tag.as_str();
        if tag_attribute(tag_str, attribute).to_lowercase() == needle {
            return tag_attribute(tag_str, "content");
        }
    }
    String::new()
}

fn tag_attribute(tag: &str, name: &str) -> String {
    let pattern = format!(r#"(?i)\b{name}\s*=\s*(["'])(.*?)\1"#);
    let re = match regex_compile(&pattern) {
        Ok(re) => re,
        Err(_) => return String::new(),
    };
    re.captures(tag)
        .and_then(|caps| caps.get(2))
        .map(|m| decode_text(m.as_str()).trim().to_string())
        .unwrap_or_default()
}

fn capture(html: &str, open: &str, close: &str) -> Option<String> {
    let pattern = format!("(?is){open}(.*?){close}");
    let re = regex_compile(&pattern).ok()?;
    re.captures(html)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

fn capture_all(html: &str, open: &str, close: &str) -> Vec<String> {
    let pattern = format!("(?is){open}(.*?){close}");
    let re = match regex_compile(&pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(html)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn decode_text(text: &str) -> String {
    let mut output = text.to_string();
    output = decode_numeric_entities(&output);
    output = output.replace("&nbsp;", " ");
    output = output.replace("&lt;", "<");
    output = output.replace("&gt;", ">");
    output = output.replace("&quot;", "\"");
    output = output.replace("&ldquo;", "\"");
    output = output.replace("&rdquo;", "\"");
    output = output.replace("&#39;", "'");
    output = output.replace("&apos;", "'");
    output = output.replace("&lsquo;", "'");
    output = output.replace("&rsquo;", "'");
    output = output.replace("&ndash;", "-");
    output = output.replace("&mdash;", "-");
    output = output.replace("&amp;", "&");
    output.retain(|c| !(c.is_control() && c != '\n' && c != '\t'));
    output
}

fn decode_numeric_entities(input: &str) -> String {
    let Ok(hex_re) = regex_compile(r"(?i)&#x([0-9a-f]+);") else {
        return input.to_string();
    };
    let Ok(dec_re) = regex_compile(r"&#(\d+);") else {
        return input.to_string();
    };
    let mut output = hex_re
        .replace_all(input, |caps: &regex::Captures| {
            match u32::from_str_radix(&caps[1], 16) {
                Ok(code) => char_from_code(code),
                Err(_) => String::new(),
            }
        })
        .to_string();
    output = dec_re
        .replace_all(&output, |caps: &regex::Captures| {
            match caps[1].parse::<u32>() {
                Ok(code) => char_from_code(code),
                Err(_) => String::new(),
            }
        })
        .to_string();
    output
}

fn char_from_code(code: u32) -> String {
    if !(0..=0x10ffff).contains(&code) || (0xd800..=0xdfff).contains(&code) {
        return String::new();
    }
    char::from_u32(code).map(|c| c.to_string()).unwrap_or_default()
}

fn html_to_text(html: &str) -> String {
    let mut output = html.to_string();
    if let Ok(re) = regex_compile(r"(?i)<(h[1-6]|p|div|article|section|li|blockquote|hr|br)[^>]*>") {
        output = re.replace_all(&output, "\n").to_string();
    }
    if let Ok(re) = regex_compile(r"(?i)</(h[1-6]|p|div|article|section|blockquote)>") {
        output = re.replace_all(&output, "\n").to_string();
    }
    if let Ok(re) = regex_compile(r"(?i)<li[^>]*>") {
        output = re.replace_all(&output, "\n- ").to_string();
    }
    if let Ok(re) = regex_compile(r"(?i)</li>") {
        output = re.replace_all(&output, "").to_string();
    }
    if let Ok(re) = regex_compile(r"(?i)</[^>]*>") {
        output = re.replace_all(&output, "").to_string();
    }
    if let Ok(re) = regex_compile(r"(?i)<[^>]*>") {
        output = re.replace_all(&output, "").to_string();
    }
    let decoded = decode_text(&output);
    collapse_whitespace(&decoded).trim().to_string()
}

fn collapse_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut prev_newline = false;
    let mut prev_space = false;
    let mut consecutive_newlines = 0;
    for ch in text.chars() {
        match ch {
            '\n' => {
                output.push('\n');
                prev_newline = true;
                prev_space = false;
                consecutive_newlines += 1;
            }
            c if c.is_whitespace() => {
                if !prev_space {
                    output.push(' ');
                }
                prev_space = true;
                prev_newline = false;
                consecutive_newlines = 0;
            }
            c => {
                if prev_newline && consecutive_newlines >= 4 {
                    output.push('\n');
                }
                output.push(c);
                prev_newline = false;
                prev_space = false;
                consecutive_newlines = 0;
            }
        }
    }
    output
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output: String = text.chars().take(max_chars).collect();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_url_rejects_non_https() {
        assert!(canonicalize_url("http://example.com").is_err());
        assert!(canonicalize_url("ftp://example.com").is_err());
    }

    #[test]
    fn canonicalize_url_rejects_credentials() {
        assert!(canonicalize_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn canonicalize_url_rejects_non_default_port() {
        assert!(canonicalize_url("https://example.com:8443").is_err());
    }

    #[test]
    fn canonicalize_url_rejects_blocked_hosts() {
        assert!(canonicalize_url("https://localhost/path").is_err());
        assert!(canonicalize_url("https://metadata.google.internal").is_err());
        assert!(canonicalize_url("https://service.lan/path").is_err());
    }

    #[test]
    fn canonicalize_url_rejects_ip_literals() {
        assert!(canonicalize_url("https://127.0.0.1").is_err());
        assert!(canonicalize_url("https://[::1]").is_err());
        assert!(canonicalize_url("https://0x7f000001").is_err());
    }

    #[test]
    fn canonicalize_url_accepts_normal_https() {
        let canonical = canonicalize_url("https://example.com/path").unwrap();
        assert_eq!(canonical, "https://example.com/path");
    }

    #[test]
    fn is_private_v4_catches_rfc1918_and_metadata() {
        assert!(is_private_v4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(is_private_v4(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(is_private_v4(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(is_private_v4(Ipv4Addr::new(169, 254, 169, 254)));
        assert!(is_private_v4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(!is_private_v4(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!is_private_v4(Ipv4Addr::new(1, 1, 1, 1)));
    }

    #[test]
    fn parses_full_html_document() {
        let html = r#"<!DOCTYPE html><html><head>
            <title>Example Domain</title>
            <meta name="description" content="This domain is for use in illustrative examples.">
        </head><body>
        <article>
        <p>This domain is for use in illustrative examples in documents. You may use this
        domain in literature without prior coordination or asking for permission.</p>
        <a href="https://example.com/more">More detail</a>
        <a href="http://example.com/insecure">Insecure</a>
        <a href="javascript:alert(1)">Bad</a>
        </article></body></html>"#;
        let parsed = ContentParser::parse(html, "https://example.com/");
        assert_eq!(parsed.title, "Example Domain");
        assert!(parsed.description.contains("illustrative examples"));
        assert!(parsed.content.contains("illustrative examples"));
        let hrefs: Vec<&str> = parsed.links.iter().map(|l| l.href.as_str()).collect();
        assert!(
            hrefs.contains(&"https://example.com/more"),
            "expected `https://example.com/more` in {hrefs:?}"
        );
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].href, "https://example.com/more");
    }

    #[test]
    fn html_to_text_decodes_entities_and_strips_tags() {
        let html = "<p>Hello &amp; goodbye &lt;world&gt;</p>";
        let text = html_to_text(html);
        assert_eq!(text, "Hello & goodbye <world>");
    }

    #[test]
    fn decode_text_handles_numeric_and_named_entities() {
        assert_eq!(decode_text("&#x2014;"), "\u{2014}");
        assert_eq!(decode_text("&#8212;"), "\u{2014}");
        assert_eq!(decode_text("&amp;"), "&");
        assert_eq!(decode_text("&nbsp;"), " ");
    }

    #[test]
    fn navigation_headers_have_full_chrome_set() {
        let headers = navigation_headers("none", None, "en-US");
        assert!(headers.contains_key("sec-ch-ua"));
        assert!(headers.contains_key("sec-ch-ua-full-version-list"));
        assert!(headers.contains_key("sec-gpc"));
        assert!(headers.contains_key("dnt"));
        assert!(headers.contains_key("accept-encoding"));
        assert!(headers.contains_key("priority"));
        assert!(headers.contains_key("user-agent"));
    }
}
