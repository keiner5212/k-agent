use std::collections::HashSet;
use std::ops::Deref;

use scraper::{ElementRef, Html, Node, Selector};

use super::fetch_url::{allows_result_url_with, FetchPolicy, ParsedLink, ParsedPage};

const MAX_TITLE_LENGTH: usize = 300;
const MAX_DESCRIPTION_LENGTH: usize = 500;
const MAX_CONTENT_LENGTH: usize = 16_000;
const MAX_LINK_TEXT_LENGTH: usize = 200;
const MAX_LINKS: usize = 20;
const MIN_MAIN_CHARS: usize = 80;

const SKIP_TAGS: &[&str] = &[
    "script",
    "style",
    "noscript",
    "template",
    "svg",
    "iframe",
    "canvas",
    "form",
    "nav",
    "footer",
    "header",
    "aside",
    "button",
    "input",
    "textarea",
    "select",
    "figure",
    "figcaption",
    "picture",
    "video",
    "audio",
    "object",
];

const MAIN_SELECTORS: &[&str] = &[
    "main",
    "article",
    "[role=\"main\"]",
    "#main-content",
    "#mw-content-text",
    ".article-body",
    ".entry-content",
    ".post-content",
];

pub fn parse_with(html: &str, base_url: &str, policy: FetchPolicy) -> ParsedPage {
    if html.trim().is_empty() {
        return ParsedPage::default();
    }
    let document = Html::parse_document(html);
    let title = extract_title(&document);
    let root = main_root(&document);
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let mut content = String::new();
    append_element(root, base_url, policy, &mut content, &mut links, &mut seen);
    let content = truncate(&normalize_text(&content), MAX_CONTENT_LENGTH);
    let description = extract_description(&document, &content);
    ParsedPage {
        title,
        description,
        content,
        links,
    }
}

fn extract_title(document: &Html) -> String {
    let social = meta_content(document, "property", "og:title");
    if !social.is_empty() {
        return truncate(&social, MAX_TITLE_LENGTH);
    }
    if let Some(title) = select_first(document, "title") {
        let text = plain_text(title);
        if !text.is_empty() {
            return truncate(&text, MAX_TITLE_LENGTH);
        }
    }
    if let Some(heading) = select_first(document, "h1") {
        return truncate(&plain_text(heading), MAX_TITLE_LENGTH);
    }
    String::new()
}

fn extract_description(document: &Html, content: &str) -> String {
    let named = meta_content(document, "name", "description");
    let social = meta_content(document, "property", "og:description");
    let meta = if named.len() >= social.len() {
        named
    } else {
        social
    };
    if meta.chars().count() > 30 {
        return truncate(&meta, MAX_DESCRIPTION_LENGTH);
    }
    for line in content.lines() {
        let line = line.trim().trim_start_matches("- ").trim();
        if line.chars().count() >= 80 {
            return truncate(line, MAX_DESCRIPTION_LENGTH);
        }
    }
    String::new()
}

fn meta_content(document: &Html, attribute: &str, value: &str) -> String {
    let Ok(selector) = Selector::parse("meta") else {
        return String::new();
    };
    for element in document.select(&selector) {
        let found = element.attr(attribute).unwrap_or("");
        if found.eq_ignore_ascii_case(value) {
            return element.attr("content").unwrap_or("").trim().to_string();
        }
    }
    String::new()
}

fn main_root(document: &Html) -> ElementRef<'_> {
    let mut best: Option<(usize, ElementRef<'_>)> = None;
    for query in MAIN_SELECTORS {
        let Ok(selector) = Selector::parse(query) else {
            continue;
        };
        for element in document.select(&selector) {
            let text = plain_text(element);
            let chars = text.chars().count();
            if chars < MIN_MAIN_CHARS {
                continue;
            }
            if best.as_ref().map(|(len, _)| chars > *len).unwrap_or(true) {
                best = Some((chars, element));
            }
        }
    }
    if let Some((_, element)) = best {
        return element;
    }
    select_first(document, "body").unwrap_or_else(|| document.root_element())
}

fn select_first<'a>(document: &'a Html, query: &str) -> Option<ElementRef<'a>> {
    let selector = Selector::parse(query).ok()?;
    document.select(&selector).next()
}

fn append_element(
    element: ElementRef<'_>,
    base_url: &str,
    policy: FetchPolicy,
    content: &mut String,
    links: &mut Vec<ParsedLink>,
    seen: &mut HashSet<String>,
) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                content.push_str(text.deref());
                content.push(' ');
            }
            Node::Element(_) => {
                let Some(child_element) = ElementRef::wrap(child) else {
                    continue;
                };
                if skip_element(child_element.value()) {
                    continue;
                }
                let name = child_element.value().name();
                let block = is_block(name);
                if block {
                    content.push('\n');
                }
                if name == "li" {
                    content.push_str("- ");
                }
                if name == "a" {
                    if let Some(link) = link_from(child_element, base_url, policy) {
                        push_link(links, seen, link);
                    }
                }
                append_element(child_element, base_url, policy, content, links, seen);
                if block {
                    content.push('\n');
                }
            }
            _ => {}
        }
    }
}

fn link_from(element: ElementRef<'_>, base_url: &str, policy: FetchPolicy) -> Option<ParsedLink> {
    let href = element.attr("href").unwrap_or("").trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let text = plain_text(element);
    if text.chars().count() < 3 || is_chrome_link(&text) {
        return None;
    }
    let resolved = if base_url.is_empty() {
        reqwest::Url::parse(href).ok()
    } else {
        reqwest::Url::parse(base_url)
            .ok()
            .and_then(|base| base.join(href).ok())
    }?;
    let mut parsed = resolved;
    parsed.set_fragment(None);
    if is_image_href(parsed.as_str()) || !allows_result_url_with(parsed.as_str(), policy) {
        return None;
    }
    Some(ParsedLink {
        href: parsed.as_str().to_string(),
        text: truncate(&text, MAX_LINK_TEXT_LENGTH),
    })
}

fn push_link(links: &mut Vec<ParsedLink>, seen: &mut HashSet<String>, link: ParsedLink) {
    if links.len() >= MAX_LINKS || !seen.insert(link.href.clone()) {
        return;
    }
    links.push(link);
}

fn skip_element(element: &scraper::node::Element) -> bool {
    let name = element.name();
    if SKIP_TAGS.contains(&name) {
        return true;
    }
    if element.attr("hidden").is_some() {
        return true;
    }
    let role = element.attr("role").unwrap_or("").to_ascii_lowercase();
    if matches!(role.as_str(), "navigation" | "banner" | "contentinfo") {
        return true;
    }
    is_noise(element.attr("class").unwrap_or("")) || is_noise(element.attr("id").unwrap_or(""))
}

fn is_noise(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "breadcrumb",
        "cookie",
        "newsletter",
        "pagination",
        "embed-image",
        "enlarge",
        "share-bar",
        "social-share",
        "skipnav",
        "summary-box",
    ];
    if NEEDLES.iter().any(|needle| lower.contains(needle)) {
        return true;
    }
    lower
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token,
                "nav"
                    | "menu"
                    | "footer"
                    | "banner"
                    | "sidebar"
                    | "comment"
                    | "comments"
                    | "related"
                    | "promo"
                    | "ads"
                    | "advert"
                    | "advertisement"
                    | "pager"
            )
        })
}

fn is_chrome_link(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "enlarge image"
            | "print"
            | "email"
            | "share"
            | "facebook"
            | "twitter"
            | "linkedin"
            | "instagram"
    )
}

fn is_image_href(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let path = parsed.path().to_ascii_lowercase();
    path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".png")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
        || path.ends_with(".svg")
        || path.ends_with(".avif")
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "section"
            | "article"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "ul"
            | "ol"
            | "blockquote"
            | "br"
            | "tr"
            | "table"
    )
}

fn plain_text(element: ElementRef<'_>) -> String {
    normalize_text(&element.text().collect::<Vec<_>>().join(" "))
}

fn normalize_text(text: &str) -> String {
    let mut lines = Vec::new();
    for raw in text.split('\n') {
        let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output: String = text.chars().take(max_chars).collect();
    output.push_str("...");
    output
}
