use std::collections::HashSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::fetch_url::{allows_result_url_with, BrowserSession, FetchPolicy};
use super::{toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue};

pub const NAME: &str = "internet_search";

const DESCRIPTION: &str = "Find current public URLs. Returns titles, URLs, and short snippets only. Snippets are not the page. Call fetch_url on a chosen URL to read it. HTTPS by default; public HTTP results appear only when HTTP fetch is enabled in settings. Off-topic engine results are dropped.";

const MAX_PAGES: usize = 2;
const MAX_QUERY_LENGTH: usize = 200;
const MAX_RESULTS_PER_PAGE: usize = 10;
const CACHE_TTL_SECS: u64 = 3_600;
const PAGE_PAUSE_MIN_MS: u64 = 1_500;
const PAGE_PAUSE_SPAN_MS: u64 = 2_000;

const BING_HOME: &str = "https://www.bing.com/";
const DUCK_HOME: &str = "https://html.duckduckgo.com/";

const COMMON_QUERY_WORDS: &[&str] = &[
    "and", "are", "como", "con", "del", "der", "die", "ein", "eine", "for", "from", "how", "las",
    "los", "para", "que", "the", "una", "und", "was", "what", "with",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    query: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    pages: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSearch {
    query: String,
    lang: String,
    pages: u32,
    results: Vec<SearchResult>,
    cached_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResult {
    position: u32,
    title: String,
    url: String,
    site: String,
    snippet: String,
}

pub struct InternetSearchTool;

impl Tool for InternetSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query used only to find current https URLs. Trimmed to 200 characters. Read a result with fetch_url; do not treat snippets as the page."
                    },
                    "lang": {
                        "type": "string",
                        "description": "Optional BCP 47 language tag (defaults to en-US). Sets Accept-Language."
                    },
                    "pages": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 2,
                        "description": "Optional page count. Default 1, hard cap 2."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        match parse_args(args) {
            Ok(_) => {
                super::context_error(None, "internet_search must run on the async dispatch path.")
            }
            Err(message) => super::context_error(None, &message),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args = match parse_args_str(arguments) {
        Ok(args) => args,
        Err(message) => return super::context_error(None, &message),
    };
    match search(&args, ctx).await {
        Ok(result) => ToolOutcome {
            text: result,
            display: ToolDisplay {
                kind: super::TOOL_KIND_CONTEXT.to_string(),
                ..ToolDisplay::default()
            },
            snapshot: None,
        },
        Err(message) => super::context_error(Some(&args.query), &message),
    }
}

async fn search(args: &SearchArgs, ctx: &ToolContext<'_>) -> Result<String, String> {
    let query = clean_query(&args.query);
    if query.is_empty() {
        return Err("internet_search requires a non-empty `query`.".into());
    }
    let lang = clean_lang(args.lang.as_deref().unwrap_or("en-US"));
    if lang.is_empty() {
        return Err("internet_search `lang` is invalid.".into());
    }
    let pages = page_count(args.pages);

    let policy = super::fetch_url::fetch_policy(ctx.app);
    if let Some(cached) = read_cache(ctx.app, &query, &lang, pages) {
        return Ok(render_cached(&query, &lang, pages, &cached));
    }

    let mut session = BrowserSession::new(&lang);
    if session.get(BING_HOME, "none", None).await.is_err() {
        // Warm-up failure should not block the search. Bing sometimes
        // serves the home page from a different host.
    }

    let mut duck_session: Option<BrowserSession> = None;
    let mut all_results: Vec<SearchResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for page in 0..pages {
        if page > 0 {
            let pause = PAGE_PAUSE_MIN_MS + (random_ms() as u64 % PAGE_PAUSE_SPAN_MS);
            tokio::time::sleep(Duration::from_millis(pause)).await;
        }
        let bing = bing_page(&mut session, &query, page, policy).await;
        let batch = match bing {
            Ok(batch) if has_relevant_results(&query, &batch) => batch,
            bing => {
                let duck = duck_session.get_or_insert_with(|| BrowserSession::new(&lang));
                pick_results(&query, bing, duck_page(duck, &query, page, policy).await)?
            }
        };
        if batch.is_empty() {
            break;
        }
        for result in batch {
            if seen.insert(result.url.clone()) {
                all_results.push(SearchResult {
                    position: (all_results.len() as u32) + 1,
                    ..result
                });
            }
        }
    }

    if !all_results.is_empty() {
        let cached = CachedSearch {
            query: query.clone(),
            lang: lang.clone(),
            pages: pages as u32,
            results: all_results.clone(),
            cached_at_secs: now_secs(),
        };
        write_cache(ctx.app, &query, &lang, pages, &cached);
    }

    Ok(render_results(&query, &lang, pages, &all_results))
}

fn pick_results(
    query: &str,
    bing: Result<Vec<SearchResult>, String>,
    duck: Result<Vec<SearchResult>, String>,
) -> Result<Vec<SearchResult>, String> {
    if let Ok(batch) = &duck {
        if has_relevant_results(query, batch) {
            return Ok(batch.clone());
        }
    }
    match (bing, duck) {
        (Err(error), Err(_)) => Err(error),
        (Err(error), Ok(batch)) if batch.is_empty() => Err(error),
        (Ok(_), Err(error)) if is_access_denied_error(&error) => Err(error),
        _ => Ok(Vec::new()),
    }
}

fn render_cached(query: &str, lang: &str, usize_pages: usize, cached: &CachedSearch) -> String {
    let pages = usize_pages as u32;
    let block = if cached.results.is_empty() {
        "(no results)".to_string()
    } else {
        render_items_block(&cached.results)
    };
    toon_doc(&[
        ("query", ToonValue::Str(query)),
        ("lang", ToonValue::Str(lang)),
        ("pages", ToonValue::Int(pages as i64)),
        ("cached", ToonValue::Int(1)),
        ("results", ToonValue::Int(cached.results.len() as i64)),
        ("items", ToonValue::Block(&block)),
    ])
}

fn render_results(query: &str, lang: &str, usize_pages: usize, results: &[SearchResult]) -> String {
    let pages = usize_pages as u32;
    let block = if results.is_empty() {
        "(no results)".to_string()
    } else {
        render_items_block(results)
    };
    toon_doc(&[
        ("query", ToonValue::Str(query)),
        ("lang", ToonValue::Str(lang)),
        ("pages", ToonValue::Int(pages as i64)),
        ("results", ToonValue::Int(results.len() as i64)),
        ("items", ToonValue::Block(&block)),
    ])
}

fn render_items_block(results: &[SearchResult]) -> String {
    let mut out = String::new();
    for (index, result) in results.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "- [{}] {} - {}\n  {}\n  site: {}",
            result.position, result.title, result.url, result.snippet, result.site
        ));
    }
    out
}

async fn bing_page(
    session: &mut BrowserSession,
    query: &str,
    page: usize,
    policy: FetchPolicy,
) -> Result<Vec<SearchResult>, String> {
    let first = page * 10 + 1;
    let url = format!(
        "https://www.bing.com/search?q={}&first={}&count=10",
        url_encode(query),
        first
    );
    let fetched = match session
        .get(&url, "same-origin", Some(BING_HOME.to_string()))
        .await
    {
        Ok(fetched) => fetched,
        Err(error) => {
            if is_access_denied_error(&error) {
                return Err(error);
            }
            return Ok(Vec::new());
        }
    };
    if let Some(status) = detect_block_status(&fetched.html) {
        return Err(format!("Search blocked ({status})"));
    }
    Ok(parse_bing_results(&fetched.html, policy))
}

async fn duck_page(
    session: &mut BrowserSession,
    query: &str,
    page: usize,
    policy: FetchPolicy,
) -> Result<Vec<SearchResult>, String> {
    let url = if page == 0 {
        format!("{}html/?q={}", DUCK_HOME, url_encode(query))
    } else {
        format!("{}html/?q={}&s={}", DUCK_HOME, url_encode(query), page * 30)
    };
    let fetched = match session.get(&url, "none", None).await {
        Ok(fetched) => fetched,
        Err(error) => {
            if is_access_denied_error(&error) {
                return Err(error);
            }
            return Ok(Vec::new());
        }
    };
    if duck_blocked(&fetched.html) {
        return Err("Search blocked".to_string());
    }
    Ok(parse_duck_results(&fetched.html, policy))
}

fn duck_blocked(html: &str) -> bool {
    if regex_match(r"(?i)\bresult__a\b", html) {
        return false;
    }
    regex_match(r"(?i)anomaly|bots use duckduckgo", html)
}

fn detect_block_status(html: &str) -> Option<u16> {
    if regex_match(r"(?i)unusual traffic|captcha", html) && !regex_match(r"(?i)\bb_algo\b", html) {
        Some(429)
    } else {
        None
    }
}

fn regex_match(pattern: &str, input: &str) -> bool {
    regex::Regex::new(pattern)
        .map(|re| re.is_match(input))
        .unwrap_or(false)
}

fn is_access_denied_error(error: &str) -> bool {
    error.contains("refused the request")
        || error.contains("rate limited")
        || error.contains("temporarily unavailable")
        || error.starts_with("Search blocked")
}

fn parse_bing_results(html: &str, policy: FetchPolicy) -> Vec<SearchResult> {
    let region = results_region(html);
    let mut results: Vec<SearchResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let block_re = match regex::Regex::new(r"(?is)<li\b[^>]*\bb_algo\b[^>]*>") {
        Ok(re) => re,
        Err(_) => return results,
    };
    let link_re_double = match regex::Regex::new(
        r#"(?is)<h2\b[^>]*>[\s\S]*?<a\b[^>]*\bhref="([^"]+)"[^>]*>([\s\S]*?)</a>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let link_re_single = match regex::Regex::new(
        r#"(?is)<h2\b[^>]*>[\s\S]*?<a\b[^>]*\bhref='([^']+)'[^>]*>([\s\S]*?)</a>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let snippet_re =
        match regex::Regex::new(r#"(?is)<p[^>]*\bb_lineclamp\d?\b[^>]*>([\s\S]*?)</p>"#) {
            Ok(re) => re,
            Err(_) => return results,
        };
    let caption_re = match regex::Regex::new(
        r#"(?is)class=["'][^"']*\bb_caption\b[^"']*[\s\S]*?<p\b[^>]*>([\s\S]*?)</p>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let p_re = match regex::Regex::new(r"(?is)<p\b[^>]*>([\s\S]*?)</p>") {
        Ok(re) => re,
        Err(_) => return results,
    };
    let tptt_re = match regex::Regex::new(r#"(?is)<div[^>]*tptt[^>]*>([\s\S]*?)</div>"#) {
        Ok(re) => re,
        Err(_) => return results,
    };

    let blocks: Vec<&str> = block_re.split(&region).collect();
    for block in blocks.into_iter().skip(1) {
        let Some(caps) = link_re_double
            .captures(block)
            .or_else(|| link_re_single.captures(block))
        else {
            continue;
        };
        let raw_href = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let url = match unwrap_bing_url(raw_href, policy) {
            Some(u) => u,
            None => continue,
        };
        let title = text_of(&decode_entities(
            caps.get(2).map(|m| m.as_str()).unwrap_or(""),
        ))
        .chars()
        .take(200)
        .collect::<String>();
        if title.is_empty() || is_bing(&url) || seen.contains(&url) {
            continue;
        }
        let snippet_html = snippet_re
            .captures(block)
            .or_else(|| caption_re.captures(block))
            .or_else(|| p_re.captures(block))
            .map(|c| c.get(1).map(|m| m.as_str()).unwrap_or("").to_string())
            .unwrap_or_default();
        let site = tptt_re
            .captures(block)
            .map(|c| text_of(&decode_entities(c.get(1).map(|m| m.as_str()).unwrap_or(""))))
            .unwrap_or_default();
        let site = site.chars().take(80).collect::<String>();
        let snippet = text_of(&decode_entities(&snippet_html))
            .chars()
            .take(300)
            .collect::<String>();
        seen.insert(url.clone());
        results.push(SearchResult {
            position: 0,
            title,
            url,
            site,
            snippet,
        });
        if results.len() >= MAX_RESULTS_PER_PAGE {
            break;
        }
    }
    results
}

fn parse_duck_results(html: &str, policy: FetchPolicy) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let block_re = match regex::Regex::new(
        r#"(?is)<div\b[^>]*class=["'][^"']*\bresult\s+results_links\b[^"']*["'][^>]*>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let link_re = match regex::Regex::new(
        r#"(?is)<a\b[^>]*\bclass=["'][^"']*\bresult__a\b[^"']*["'][^>]*\bhref=["']([^"']+)["'][^>]*>([\s\S]*?)</a>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let snippet_re = match regex::Regex::new(
        r#"(?is)<a\b[^>]*class=["'][^"']*\bresult__snippet\b[^"']*["'][^>]*>([\s\S]*?)</a>"#,
    ) {
        Ok(re) => re,
        Err(_) => return results,
    };
    let blocks: Vec<&str> = block_re.split(html).collect();
    for block in blocks.into_iter().skip(1) {
        let Some(caps) = link_re.captures(block) else {
            continue;
        };
        let raw_href = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let url = match unwrap_duck_url(raw_href, policy) {
            Some(u) => u,
            None => continue,
        };
        let title = text_of(&decode_entities(
            caps.get(2).map(|m| m.as_str()).unwrap_or(""),
        ))
        .chars()
        .take(200)
        .collect::<String>();
        if title.is_empty() || seen.contains(&url) {
            continue;
        }
        let snippet = snippet_re
            .captures(block)
            .map(|c| text_of(&decode_entities(c.get(1).map(|m| m.as_str()).unwrap_or(""))))
            .unwrap_or_default();
        let snippet = snippet.chars().take(300).collect::<String>();
        let site = reqwest::Url::parse(&url)
            .map(|u| u.host_str().unwrap_or("").to_string())
            .unwrap_or_default();
        let site = site
            .trim_start_matches("www.")
            .chars()
            .take(80)
            .collect::<String>();
        seen.insert(url.clone());
        results.push(SearchResult {
            position: 0,
            title,
            url,
            site,
            snippet,
        });
        if results.len() >= MAX_RESULTS_PER_PAGE {
            break;
        }
    }
    results
}

fn results_region(html: &str) -> &str {
    let re =
        match regex::Regex::new(r#"(?is)<ol\b[^>]*\bid=["']b_results["'][^>]*>([\s\S]*?)</ol>"#) {
            Ok(re) => re,
            Err(_) => return html,
        };
    re.captures(html)
        .and_then(|caps| caps.get(1).map(|m| m.as_str()))
        .unwrap_or(html)
}

fn unwrap_bing_url(href: &str, policy: FetchPolicy) -> Option<String> {
    let decoded = decode_entities(href.trim());
    let decoded = collapse_double_ampersands(&decoded);
    let parsed = reqwest::Url::parse(&decoded).ok()?;
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    if (host == "bing.com" || host.ends_with(".bing.com")) && parsed.path().starts_with("/ck/") {
        let pairs: Vec<_> = parsed.query_pairs().collect();
        let token = pairs
            .iter()
            .find(|(name, _)| name == "u" || name == "!u")
            .map(|(_, value)| value.to_string())?;
        let token = token.strip_prefix("a1").unwrap_or(&token);
        let token = token.replace('-', "+").replace('_', "/");
        if token.is_empty() || token.len() > 4096 {
            return None;
        }
        let candidate = base64_decode(&token)?;
        if allows_result_url_with(&candidate, policy) {
            return Some(clean_result_url(&candidate));
        }
        return None;
    }
    if allows_result_url_with(parsed.as_str(), policy) {
        Some(clean_result_url(parsed.as_str()))
    } else {
        None
    }
}

fn collapse_double_ampersands(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            output.push('&');
            while chars.peek() == Some(&'&') {
                chars.next();
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn unwrap_duck_url(href: &str, policy: FetchPolicy) -> Option<String> {
    let decoded = decode_entities(href.trim());
    let decoded = if let Some(rest) = decoded.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        decoded
    };
    let parsed = reqwest::Url::parse(&decoded).ok().or_else(|| {
        reqwest::Url::parse(DUCK_HOME)
            .ok()
            .and_then(|base| base.join(&decoded).ok())
    })?;
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    if (host == "duckduckgo.com" || host.ends_with(".duckduckgo.com")) && parsed.path() == "/l/" {
        let target = parsed
            .query_pairs()
            .find(|(name, _)| name == "uddg")
            .map(|(_, value)| value.into_owned())?;
        if allows_result_url_with(&target, policy) {
            return Some(clean_result_url(&target));
        }
        return None;
    }
    if allows_result_url_with(parsed.as_str(), policy) {
        Some(clean_result_url(parsed.as_str()))
    } else {
        None
    }
}

fn clean_result_url(raw: &str) -> String {
    let Ok(mut parsed) = reqwest::Url::parse(raw) else {
        return raw.to_string();
    };
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(name, _)| !is_tracker_param(name))
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(&kept);
    }
    parsed.set_fragment(None);
    parsed.to_string()
}

fn is_tracker_param(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("utm_") || matches!(name.as_str(), "fbclid" | "gclid" | "mc_eid" | "yclid")
}

fn is_bing(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| {
            let host = u.host_str().unwrap_or("").to_lowercase();
            host == "bing.com" || host.ends_with(".bing.com")
        })
        .unwrap_or(true)
}

fn has_relevant_results(query: &str, results: &[SearchResult]) -> bool {
    if results.is_empty() {
        return false;
    }
    let terms = search_terms(query);
    if terms.is_empty() {
        return true;
    }
    let mut covered: HashSet<String> = HashSet::new();
    let terms_per_result = terms.len().min(2);
    let mut strong_results = 0;
    for result in results {
        let haystack = format!(
            "{} {} {} {}",
            result.title, result.site, result.snippet, result.url
        );
        let words: HashSet<String> = normalized_words(&haystack).into_iter().collect();
        let matched: Vec<&String> = terms.iter().filter(|term| words.contains(*term)).collect();
        for term in &matched {
            covered.insert((*term).clone());
        }
        if matched.len() >= terms_per_result {
            strong_results += 1;
        }
    }
    let required_coverage = ((terms.len() as f64 * 0.6).ceil() as usize).max(1);
    let required_results = if results.len() >= 5 { 2 } else { 1 };
    covered.len() >= required_coverage && strong_results >= required_results
}

fn search_terms(query: &str) -> Vec<String> {
    let tokens = normalized_words(query);
    let useful: Vec<String> = tokens
        .iter()
        .filter(|token| token.chars().count() >= 3 && !COMMON_QUERY_WORDS.contains(&token.as_str()))
        .cloned()
        .collect();
    if useful.is_empty() {
        tokens.into_iter().take(8).collect()
    } else {
        let mut seen: HashSet<String> = HashSet::new();
        useful
            .into_iter()
            .filter(|t| seen.insert(t.clone()))
            .take(8)
            .collect()
    }
}

fn normalized_words(value: &str) -> Vec<String> {
    let normalized = value
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in normalized.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn text_of(html: &str) -> String {
    let re = match regex::Regex::new(r"<[^>]+>") {
        Ok(re) => re,
        Err(_) => return html.to_string(),
    };
    let stripped = re.replace_all(html, " ");
    let decoded = decode_entities(&stripped);
    let collapsed = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim().to_string()
}

fn decode_entities(text: &str) -> String {
    let mut output = text.to_string();
    if let Ok(re) = regex::Regex::new(r"(?i)&#x([0-9a-f]+);") {
        output = re
            .replace_all(
                &output,
                |caps: &regex::Captures| match u32::from_str_radix(&caps[1], 16) {
                    Ok(code) => char_from_code(code),
                    Err(_) => String::new(),
                },
            )
            .to_string();
    }
    if let Ok(re) = regex::Regex::new(r"&#(\d+);") {
        output = re
            .replace_all(&output, |caps: &regex::Captures| {
                match caps[1].parse::<u32>() {
                    Ok(code) => char_from_code(code),
                    Err(_) => String::new(),
                }
            })
            .to_string();
    }
    output = output.replace("&amp;", "&");
    output = output.replace("&lt;", "<");
    output = output.replace("&gt;", ">");
    output = output.replace("&quot;", "\"");
    output = output.replace("&#39;", "'");
    output = output.replace("&apos;", "'");
    output = output.replace("&nbsp;", " ");
    output
}

fn char_from_code(code: u32) -> String {
    if !(0..=0x10ffff).contains(&code) || (0xd800..=0xdfff).contains(&code) {
        return String::new();
    }
    char::from_u32(code)
        .map(|c| c.to_string())
        .unwrap_or_default()
}

fn url_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(*byte as char);
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{:02X}", byte)),
        }
    }
    output
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::engine::general_purpose::URL_SAFE;
    use base64::Engine;
    let pad = (4 - input.len() % 4) % 4;
    let padded: String = if pad == 0 {
        input.to_string()
    } else {
        format!("{}{}", input, "=".repeat(pad))
    };
    URL_SAFE.decode(padded).ok().and_then(|bytes| {
        let trimmed: Vec<u8> = bytes.into_iter().filter(|byte| *byte != 0).collect();
        String::from_utf8(trimmed).ok()
    })
}

fn clean_query(query: &str) -> String {
    let trimmed: String = query
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    trimmed
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_QUERY_LENGTH)
        .collect()
}

fn clean_lang(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.len() > 35
        || trimmed
            .chars()
            .any(|c| c.is_control() || matches!(c, ',' | ';' | '\\'))
    {
        return String::new();
    }
    let primary = trimmed.split('-').next().unwrap_or(trimmed);
    if primary.is_empty() {
        return String::new();
    }
    trimmed.to_string()
}

fn page_count(value: Option<u32>) -> usize {
    match value {
        Some(n) if n >= 1 => (n as usize).min(MAX_PAGES),
        _ => 1,
    }
}

fn random_ms() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

fn parse_args(args: &Value) -> Result<(), String> {
    let query = args.get("query").and_then(Value::as_str).unwrap_or("");
    if query.is_empty() {
        return Err("internet_search requires a string `query`.".into());
    }
    if let Some(lang) = args.get("lang").and_then(Value::as_str) {
        if clean_lang(lang).is_empty() {
            return Err("internet_search `lang` is invalid.".into());
        }
    }
    if let Some(pages) = args.get("pages").and_then(Value::as_u64) {
        if !(1..=2).contains(&pages) {
            return Err("internet_search `pages` must be 1 or 2.".into());
        }
    }
    Ok(())
}

fn parse_args_str(arguments: &str) -> Result<SearchArgs, String> {
    let value: Value = serde_json::from_str(arguments)
        .map_err(|_| "internet_search arguments must be a JSON object.".to_string())?;
    parse_args(&value)?;
    let query = value
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "internet_search requires a string `query`.".to_string())?
        .to_string();
    let lang = value
        .get("lang")
        .and_then(Value::as_str)
        .map(str::to_string);
    let pages = value.get("pages").and_then(Value::as_u64).map(|n| n as u32);
    Ok(SearchArgs { query, lang, pages })
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_root(app: Option<&tauri::AppHandle>) -> Option<std::path::PathBuf> {
    super::tool_cache_dir(app, "internet-search")
}

fn cache_key(query: &str, lang: &str, pages: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    lang.hash(&mut hasher);
    pages.hash(&mut hasher);
    format!("{:016x}.json", hasher.finish())
}

fn read_cache(
    app: Option<&tauri::AppHandle>,
    query: &str,
    lang: &str,
    pages: usize,
) -> Option<CachedSearch> {
    let root = cache_root(app)?;
    let path = root.join(cache_key(query, lang, pages));
    let raw = std::fs::read_to_string(&path).ok()?;
    let cached: CachedSearch = serde_json::from_str(&raw).ok()?;
    if now_secs().saturating_sub(cached.cached_at_secs) > CACHE_TTL_SECS {
        return None;
    }
    Some(cached)
}

fn write_cache(
    app: Option<&tauri::AppHandle>,
    query: &str,
    lang: &str,
    pages: usize,
    cached: &CachedSearch,
) {
    let Some(root) = cache_root(app) else { return };
    let path = root.join(cache_key(query, lang, pages));
    if let Ok(text) = serde_json::to_string(cached) {
        let _ = std::fs::write(&path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bing_block_with_ck_redirect() {
        let html = r#"<ol id="b_results"><li class="b_algo">
        <h2><a href="https://www.bing.com/ck/a?!u=aHR0cHM6Ly9leGFtcGxlLmNvbS9hcnRpY2xl">Example Article</a></h2>
        <p class="b_lineclamp2 b_algoSlug">Snippet text describing the article.</p>
        <div class="b_tptt">example.com</div>
        </li></ol>"#;
        let results = parse_bing_results(html, FetchPolicy::public_https());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Article");
        assert_eq!(results[0].url, "https://example.com/article");
        assert_eq!(results[0].site, "example.com");
        assert!(results[0].snippet.contains("Snippet text"));
    }

    #[test]
    fn detects_bing_block_response() {
        let html = "<html>unusual traffic from your computer</html>";
        assert_eq!(detect_block_status(html), Some(429));
    }

    #[test]
    fn ignores_bing_block_response_with_results_present() {
        let html = "<html>b_algo and captcha still rendered results</html>";
        assert_eq!(detect_block_status(html), None);
    }

    #[test]
    fn unwrap_bing_url_passes_through_non_redirect() {
        let url =
            unwrap_bing_url("https://example.com/article", FetchPolicy::public_https()).unwrap();
        assert_eq!(url, "https://example.com/article");
    }

    #[test]
    fn unwrap_bing_url_rejects_private_host() {
        assert!(unwrap_bing_url("https://localhost/path", FetchPolicy::public_https()).is_none());
    }

    #[test]
    fn relevance_check_requires_term_coverage() {
        let results = vec![SearchResult {
            position: 1,
            title: "Rust tutorial".into(),
            url: "https://example.com/rust".into(),
            site: "example.com".into(),
            snippet: "Learn the Rust programming language.".into(),
        }];
        assert!(has_relevant_results("rust tutorial", &results));
    }

    #[test]
    fn relevance_check_rejects_off_topic_results() {
        let results = vec![SearchResult {
            position: 1,
            title: "Cooking recipes".into(),
            url: "https://example.com/recipes".into(),
            site: "example.com".into(),
            snippet: "Pasta and pizza and bread".into(),
        }];
        assert!(!has_relevant_results("rust async await", &results));
    }

    #[test]
    fn clean_query_strips_control_and_truncates() {
        let input = "rust\n\tasync\tawait\0rest";
        let cleaned = clean_query(input);
        assert_eq!(cleaned, "rust async await rest");
        let long = "a".repeat(500);
        let trimmed = clean_query(&long);
        assert_eq!(trimmed.chars().count(), MAX_QUERY_LENGTH);
    }

    #[test]
    fn clean_lang_rejects_invalid_inputs() {
        assert!(clean_lang("").is_empty());
        assert!(clean_lang(",").is_empty());
        assert!(clean_lang("en,US").is_empty());
        assert_eq!(clean_lang("en-US"), "en-US");
        assert_eq!(clean_lang("es"), "es");
    }

    #[test]
    fn page_count_clamps_to_max() {
        assert_eq!(page_count(None), 1);
        assert_eq!(page_count(Some(1)), 1);
        assert_eq!(page_count(Some(2)), 2);
        assert_eq!(page_count(Some(5)), MAX_PAGES);
    }

    #[test]
    fn url_encode_handles_reserved_chars() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("rust & async"), "rust+%26+async");
    }

    #[test]
    fn base64_decode_round_trip() {
        let encoded = "aHR0cHM6Ly9leGFtcGxlLmNvbS9hcnRpY2xl";
        let decoded = base64_decode(encoded).unwrap();
        assert_eq!(decoded, "https://example.com/article");
    }

    #[test]
    fn pick_results_drops_bing_decoy_when_duck_matches() {
        let decoy = vec![SearchResult {
            position: 1,
            title: "Reebok Official Site".into(),
            url: "https://www.reebok.com/".into(),
            site: "reebok.com".into(),
            snippet: "Athletic shoes and apparel.".into(),
        }];
        let real = vec![SearchResult {
            position: 1,
            title: "What Is Cancer? - NCI".into(),
            url: "https://www.cancer.gov/about-cancer/understanding/what-is-cancer".into(),
            site: "cancer.gov".into(),
            snippet: "Cancer is a disease in which some of the body's cells grow uncontrollably."
                .into(),
        }];
        let picked = pick_results(
            "cancer NCI NIH overview disease cells",
            Ok(decoy),
            Ok(real.clone()),
        )
        .unwrap();
        assert_eq!(picked[0].url, real[0].url);
    }

    #[test]
    fn pick_results_returns_empty_when_both_are_off_topic() {
        let decoy = vec![SearchResult {
            position: 1,
            title: "OpenCode Zen".into(),
            url: "https://opencode.ai/docs/zen/".into(),
            site: "opencode.ai".into(),
            snippet: "Tested models for coding agents.".into(),
        }];
        let picked = pick_results(
            "cancer NCI NIH overview disease cells",
            Ok(decoy.clone()),
            Ok(decoy),
        )
        .unwrap();
        assert!(picked.is_empty());
    }

    #[test]
    fn unwrap_bing_ck_with_empty_query_key() {
        let href = "https://www.bing.com/ck/a?!&&p=abc&u=a1aHR0cHM6Ly93d3cuY2FuY2VyLmdvdi8&ntb=1";
        let url = unwrap_bing_url(href, FetchPolicy::public_https()).unwrap();
        assert_eq!(url, "https://www.cancer.gov/");
    }

    #[test]
    fn parses_duck_result_without_lookahead() {
        let html = r#"<div class="result results_links results_links_deep web-result ">
            <h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.cancer.gov%2Fabout-cancer%2Funderstanding%2Fwhat-is-cancer&amp;rut=abc">What Is Cancer? - NCI</a></h2>
            <a class="result__snippet" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.cancer.gov%2Fabout-cancer">Cancer is a disease in which cells grow.</a>
        </div>"#;
        let results = parse_duck_results(html, FetchPolicy::public_https());
        assert_eq!(results.len(), 1, "{results:?}");
        assert_eq!(
            results[0].url,
            "https://www.cancer.gov/about-cancer/understanding/what-is-cancer"
        );
        assert!(results[0].snippet.contains("disease"));
        assert!(!duck_blocked(html));
        assert!(!duck_blocked(
            "health-challenges in a url and result__a still present <a class=\"result__a\"></a>"
        ));
    }

    #[test]
    fn unwrap_duck_protocol_relative_redirect() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.cancer.gov%2Fabout-cancer%2Funderstanding%2Fwhat-is-cancer&rut=abc";
        let url = unwrap_duck_url(href, FetchPolicy::public_https()).unwrap();
        assert_eq!(
            url,
            "https://www.cancer.gov/about-cancer/understanding/what-is-cancer"
        );
    }

    #[test]
    fn base64_decode_pads_short_input() {
        let encoded = "aHR0cHM6Ly9ydXN0LWxhbmcub3JnLw";
        let decoded = base64_decode(encoded).unwrap();
        assert_eq!(decoded, "https://rust-lang.org/");
    }

    #[tokio::test]
    #[ignore]
    async fn live_cancer_query_prefers_relevant_urls() {
        let ctx = super::ToolContext::for_test(std::env::temp_dir(), 1);
        let outcome = execute_async(
            r#"{"query":"cancer NCI NIH overview disease cells","lang":"en-US","pages":1}"#,
            &ctx,
        )
        .await;
        assert!(outcome.text.contains("cancer.gov"), "{}", outcome.text);
        assert!(!outcome.text.to_ascii_lowercase().contains("opencode"));
        assert!(!outcome.text.to_ascii_lowercase().contains("reebok"));
    }
}
