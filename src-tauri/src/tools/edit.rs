use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use super::{
    line_add_remove, yaml_doc, FileSnapshot, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec,
    YamlValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "edit";

const DESCRIPTION: &str = "Exact string replace in a file. Read first. Fails if oldString is missing or not unique, unless replaceAll is true. Path is absolute or workspace-relative.";

pub struct EditTool;

impl Tool for EditTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path"
                    },
                    "oldString": {
                        "type": "string",
                        "description": "Text to find"
                    },
                    "newString": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "replaceAll": {
                        "type": "boolean",
                        "description": "Replace every match (default false)"
                    }
                },
                "required": ["filePath", "oldString", "newString"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(raw_path) = args.get("filePath").and_then(Value::as_str) else {
            return super::action_error("", "edit tool requires a string `filePath`.");
        };
        let Some(old_string) = args.get("oldString").and_then(Value::as_str) else {
            return super::action_error("", "edit tool requires a string `oldString`.");
        };
        let Some(new_string) = args.get("newString").and_then(Value::as_str) else {
            return super::action_error("", "edit tool requires a string `newString`.");
        };
        let replace_all = args
            .get("replaceAll")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let trimmed_path = raw_path.trim();
        if trimmed_path.is_empty() {
            return super::action_error("", "edit tool `filePath` is empty.");
        }
        if old_string == new_string {
            return super::action_error(
                trimmed_path,
                "No changes to apply: oldString and newString are identical.",
            );
        }
        let resolved = match resolve_path(ctx, trimmed_path) {
            Ok(value) => value,
            Err(message) => return super::action_error(trimmed_path, &message),
        };
        let rel = ctx.relative_path(&resolved);
        let _guard = match file_lock_for(&resolved) {
            Ok(guard) => guard,
            Err(message) => return super::action_error(&rel, &message),
        };

        match fs::read_to_string(&resolved) {
            Ok(content_old) => {
                let ending = detect_line_ending(&content_old);
                let normalized_old =
                    convert_to_line_ending(&normalize_line_endings(old_string), ending);
                let normalized_new =
                    convert_to_line_ending(&normalize_line_endings(new_string), ending);
                let replaced =
                    match replace(&content_old, &normalized_old, &normalized_new, replace_all) {
                        Ok(value) => value,
                        Err(error) => return super::action_error(&rel, &error),
                    };
                if let Err(error) = fs::write(&resolved, &replaced) {
                    return super::action_error(
                        &rel,
                        &format!("Unable to write `{}`: {error}", resolved.display()),
                    );
                }
                let (added, removed) = line_add_remove(&content_old, &replaced);
                ToolOutcome {
                    text: yaml_doc(&[
                        ("path", YamlValue::Str(&rel)),
                        ("status", YamlValue::Str("ok")),
                        ("added", YamlValue::Int(added as i64)),
                        ("removed", YamlValue::Int(removed as i64)),
                    ]),
                    display: ToolDisplay {
                        kind: TOOL_KIND_ACTION.to_string(),
                        path: Some(rel),
                        added: Some(added),
                        removed: Some(removed),
                        status: Some("ok".into()),
                        ..ToolDisplay::default()
                    },
                    snapshot: Some(FileSnapshot {
                        before: content_old,
                        after: replaced,
                    }),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                super::action_error(&rel, &format!("File not found: {}", resolved.display()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                super::action_error(&rel, &format!("Cannot edit binary file: {}", resolved.display()))
            }
            Err(error) => super::action_error(
                &rel,
                &format!("Unable to read `{}`: {error}", resolved.display()),
            ),
        }
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

static FILE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, &'static Mutex<()>>>> = OnceLock::new();

fn file_lock_registry() -> &'static Mutex<HashMap<PathBuf, &'static Mutex<()>>> {
    FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct FileLockGuard {
    _mutex: std::sync::MutexGuard<'static, ()>,
}

fn file_lock_for(path: &std::path::Path) -> Result<FileLockGuard, String> {
    let canonical = crate::pathutil::canonicalize_path(path).unwrap_or_else(|_| path.to_path_buf());
    let mutex: &'static Mutex<()> = {
        let mut map = file_lock_registry()
            .lock()
            .map_err(|_| "edit tool lock registry poisoned.".to_string())?;
        map.entry(canonical)
            .or_insert_with(|| Box::leak(Box::new(Mutex::new(()))))
    };
    let guard = mutex
        .lock()
        .map_err(|_| "edit tool file lock poisoned.".to_string())?;
    Ok(FileLockGuard { _mutex: guard })
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn detect_line_ending(text: &str) -> LineEnding {
    if text.contains("\r\n") {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,
    Crlf,
}

fn convert_to_line_ending(text: &str, ending: LineEnding) -> String {
    match ending {
        LineEnding::Lf => text.to_string(),
        LineEnding::Crlf => text.replace('\n', "\r\n"),
    }
}

pub fn replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String, String> {
    if old_string == new_string {
        return Err("No changes to apply: oldString and newString are identical.".into());
    }
    if old_string.is_empty() {
        return Err(
            "oldString cannot be empty. Use write for an intentional full-file replacement.".into(),
        );
    }

    let replacers: [Replacer; 9] = [
        simple_replacer,
        line_trimmed_replacer,
        block_anchor_replacer,
        whitespace_normalized_replacer,
        indentation_flexible_replacer,
        escape_normalized_replacer,
        trimmed_boundary_replacer,
        context_aware_replacer,
        multi_occurrence_replacer,
    ];

    let mut last_error: Option<String> = None;
    for replacer in replacers {
        for candidate in replacer(content, old_string) {
            if candidate.is_empty() {
                continue;
            }
            let first = match content.find(&candidate) {
                Some(index) => index,
                None => continue,
            };
            if is_disproportionate(&candidate, old_string) {
                return Err(
                    "Refusing replacement because the matched span is much larger than oldString. Re-read the file and provide the full exact oldString for the intended replacement.".into(),
                );
            }
            if replace_all {
                return Ok(content.replace(&candidate, new_string));
            }
            let last = content.rfind(&candidate).unwrap_or(first);
            if first != last {
                last_error = Some(format!(
                    "Found multiple matches for oldString. Provide more surrounding context to make the match unique, or set replaceAll to true."
                ));
                continue;
            }
            let mut output = String::with_capacity(content.len() + new_string.len());
            output.push_str(&content[..first]);
            output.push_str(new_string);
            output.push_str(&content[first + candidate.len()..]);
            return Ok(output);
        }
    }

    Err(last_error.unwrap_or_else(|| {
        "Could not find oldString in the file. It must match exactly, including whitespace, indentation, and line endings.".to_string()
    }))
}

type Replacer = fn(&str, &str) -> Vec<String>;

fn simple_replacer(content: &str, find: &str) -> Vec<String> {
    if content.contains(find) {
        vec![find.to_string()]
    } else {
        Vec::new()
    }
}

fn line_trimmed_replacer(content: &str, find: &str) -> Vec<String> {
    let original_lines: Vec<&str> = content.split('\n').collect();
    let mut find_lines: Vec<&str> = find.split('\n').collect();
    if find_lines.last().copied() == Some("") {
        find_lines.pop();
    }
    if find_lines.is_empty() {
        return Vec::new();
    }
    if original_lines.len() < find_lines.len() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for i in 0..=original_lines.len() - find_lines.len() {
        let mut matches = true;
        for j in 0..find_lines.len() {
            if original_lines[i + j].trim() != find_lines[j].trim() {
                matches = false;
                break;
            }
        }
        if matches {
            let start_byte = line_offset(&original_lines, i);
            let end_byte = line_offset(&original_lines, i + find_lines.len()) - 1;
            if end_byte > start_byte {
                hits.push(content[start_byte..end_byte].to_string());
            } else {
                hits.push(String::new());
            }
        }
    }
    hits
}

fn line_offset(lines: &[&str], line_index: usize) -> usize {
    let mut offset = 0;
    for line in lines.iter().take(line_index) {
        offset += line.len() + 1;
    }
    offset
}

const SINGLE_CANDIDATE_SIMILARITY_THRESHOLD: f64 = 0.65;
const MULTIPLE_CANDIDATES_SIMILARITY_THRESHOLD: f64 = 0.65;

fn block_anchor_replacer(content: &str, find: &str) -> Vec<String> {
    let original_lines: Vec<&str> = content.split('\n').collect();
    let mut find_lines: Vec<&str> = find.split('\n').collect();
    if find_lines.len() < 3 {
        return Vec::new();
    }
    if find_lines.last().copied() == Some("") {
        find_lines.pop();
    }
    if find_lines.len() < 3 {
        return Vec::new();
    }
    let first = find_lines.first().copied().unwrap_or("").trim();
    let last = find_lines.last().copied().unwrap_or("").trim();
    if first.is_empty() || last.is_empty() {
        return Vec::new();
    }
    let block_size = find_lines.len();
    let max_line_delta = ((block_size as f64) * 0.25).floor() as usize;
    let max_line_delta = max_line_delta.max(1);
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for i in 0..original_lines.len() {
        if original_lines[i].trim() != first {
            continue;
        }
        for j in (i + 2)..original_lines.len() {
            if original_lines[j].trim() == last {
                let actual = j - i + 1;
                if actual.abs_diff(block_size) <= max_line_delta {
                    candidates.push((i, j));
                }
                break;
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    if candidates.len() == 1 {
        let (start, end) = candidates[0];
        let similarity = average_similarity(&original_lines, start, end, &find_lines);
        if similarity >= SINGLE_CANDIDATE_SIMILARITY_THRESHOLD {
            let (start_byte, end_byte) = block_byte_range(&original_lines, start, end);
            return vec![content[start_byte..end_byte].to_string()];
        }
        return Vec::new();
    }
    let mut best: Option<(usize, usize, f64)> = None;
    for &(start, end) in &candidates {
        let similarity = average_similarity(&original_lines, start, end, &find_lines);
        match best {
            Some((_, _, current)) if similarity <= current => {}
            _ => best = Some((start, end, similarity)),
        }
    }
    if let Some((start, end, similarity)) = best {
        if similarity >= MULTIPLE_CANDIDATES_SIMILARITY_THRESHOLD {
            let (start_byte, end_byte) = block_byte_range(&original_lines, start, end);
            return vec![content[start_byte..end_byte].to_string()];
        }
    }
    Vec::new()
}

fn average_similarity(original: &[&str], start: usize, end: usize, find_lines: &[&str]) -> f64 {
    let actual_size = end - start + 1;
    let find_size = find_lines.len();
    let middle = (find_size - 2).min(actual_size - 2);
    if middle == 0 {
        return 1.0;
    }
    let mut sum = 0.0;
    for k in 1..(find_size - 1).min(actual_size - 1) {
        let a = original[start + k].trim();
        let b = find_lines[k].trim();
        let max_len = a.len().max(b.len());
        if max_len == 0 {
            continue;
        }
        let distance = levenshtein(a, b);
        sum += 1.0 - (distance as f64) / (max_len as f64);
    }
    sum / (middle as f64)
}

fn block_byte_range(lines: &[&str], start: usize, end: usize) -> (usize, usize) {
    let start_byte = line_offset(lines, start);
    let mut end_byte = start_byte;
    for k in start..=end {
        end_byte += lines[k].len();
        if k < end {
            end_byte += 1;
        }
    }
    (start_byte, end_byte)
}

fn levenshtein(a: &str, b: &str) -> usize {
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr: Vec<usize> = vec![0; b_chars.len() + 1];
    for i in 1..=a_chars.len() {
        curr[0] = i;
        for j in 1..=b_chars.len() {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

fn whitespace_normalized_replacer(content: &str, find: &str) -> Vec<String> {
    let normalized_find = collapse_whitespace(find);
    if normalized_find.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let mut hits = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if collapse_whitespace(line) == normalized_find {
            hits.push(lines[i].to_string());
            continue;
        }
        let collapsed_line = collapse_whitespace(line);
        if collapsed_line.contains(&normalized_find) {
            let trimmed = find.trim();
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            if let Some(pattern) = build_word_pattern(&words) {
                if let Some(matched) = line.find(&pattern) {
                    hits.push(line[matched..matched + pattern.len()].to_string());
                }
            }
        }
    }
    if find.contains('\n') {
        let find_lines: Vec<&str> = find.split('\n').collect();
        if find_lines.len() > 1 && lines.len() >= find_lines.len() {
            for i in 0..=lines.len() - find_lines.len() {
                let block = lines[i..i + find_lines.len()].join("\n");
                if collapse_whitespace(&block) == normalized_find {
                    hits.push(block);
                }
            }
        }
    }
    hits
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn build_word_pattern(words: &[&str]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let escaped: Vec<String> = words.iter().map(|word| regex_escape(word)).collect();
    Some(escaped.join("\\s+"))
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '.' | '\\' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
            | '#' => {
                out.push('\\');
                out.push(ch);
            }
            other => out.push(other),
        }
    }
    out
}

fn indentation_flexible_replacer(content: &str, find: &str) -> Vec<String> {
    let stripped_find = strip_common_indent(find);
    let content_lines: Vec<&str> = content.split('\n').collect();
    let find_lines: Vec<&str> = find.split('\n').collect();
    if content_lines.len() < find_lines.len() || find_lines.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for i in 0..=content_lines.len() - find_lines.len() {
        let block = content_lines[i..i + find_lines.len()].join("\n");
        if strip_common_indent(&block) == stripped_find {
            hits.push(block);
        }
    }
    hits
}

fn strip_common_indent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if non_empty.is_empty() {
        return text.to_string();
    }
    let min_indent = non_empty
        .iter()
        .map(|line| line.chars().take_while(|ch| *ch == ' ').count())
        .min()
        .unwrap_or(0);
    if min_indent == 0 {
        return text.to_string();
    }
    lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                line.chars().skip(min_indent).collect::<String>()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_normalized_replacer(content: &str, find: &str) -> Vec<String> {
    let unescaped = unescape_common(find);
    let mut hits = Vec::new();
    if content.contains(&unescaped) {
        hits.push(unescaped.clone());
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let find_lines: Vec<&str> = unescaped.split('\n').collect();
    if find_lines.len() > 1 && lines.len() >= find_lines.len() {
        for i in 0..=lines.len() - find_lines.len() {
            let block = lines[i..i + find_lines.len()].join("\n");
            if unescape_common(&block) == unescaped {
                hits.push(block);
            }
        }
    }
    hits
}

fn unescape_common(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('r') => {
                    chars.next();
                    out.push('\r');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                Some('\'') => {
                    chars.next();
                    out.push('\'');
                }
                Some('"') => {
                    chars.next();
                    out.push('"');
                }
                Some('`') => {
                    chars.next();
                    out.push('`');
                }
                Some('$') => {
                    chars.next();
                    out.push('$');
                }
                Some('\n') => {
                    chars.next();
                    out.push('\n');
                }
                Some(other) => {
                    out.push('\\');
                    out.push(*other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn trimmed_boundary_replacer(content: &str, find: &str) -> Vec<String> {
    let trimmed = find.trim();
    if trimmed == find {
        return Vec::new();
    }
    let mut hits = Vec::new();
    if content.contains(trimmed) {
        hits.push(trimmed.to_string());
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let find_lines: Vec<&str> = find.split('\n').collect();
    if find_lines.len() > 1 && lines.len() >= find_lines.len() {
        for i in 0..=lines.len() - find_lines.len() {
            let block = lines[i..i + find_lines.len()].join("\n");
            if block.trim() == trimmed {
                hits.push(block);
            }
        }
    }
    hits
}

fn context_aware_replacer(content: &str, find: &str) -> Vec<String> {
    let mut find_lines: Vec<&str> = find.split('\n').collect();
    if find_lines.len() < 3 {
        return Vec::new();
    }
    if find_lines.last().copied() == Some("") {
        find_lines.pop();
    }
    if find_lines.len() < 3 {
        return Vec::new();
    }
    let first = find_lines.first().copied().unwrap_or("").trim();
    let last = find_lines.last().copied().unwrap_or("").trim();
    if first.is_empty() || last.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let mut hits = Vec::new();
    for i in 0..lines.len() {
        if lines[i].trim() != first {
            continue;
        }
        for j in (i + 2)..lines.len() {
            if lines[j].trim() != last {
                continue;
            }
            if j - i + 1 != find_lines.len() {
                break;
            }
            let mut total = 0usize;
            let mut matches = 0usize;
            for k in 1..find_lines.len() - 1 {
                let a = lines[i + k].trim();
                let b = find_lines[k].trim();
                if a.is_empty() && b.is_empty() {
                    continue;
                }
                total += 1;
                if a == b {
                    matches += 1;
                }
            }
            if total == 0 || (matches as f64) / (total as f64) >= 0.5 {
                hits.push(lines[i..=j].join("\n"));
            }
            break;
        }
    }
    hits
}

fn multi_occurrence_replacer(content: &str, find: &str) -> Vec<String> {
    if !content.contains(find) {
        return Vec::new();
    }
    vec![find.to_string()]
}

fn is_disproportionate(candidate: &str, find: &str) -> bool {
    let find_lines = find.lines().count();
    let candidate_lines = candidate.lines().count();
    if candidate_lines >= (find_lines + 3).max(find_lines * 2) {
        return true;
    }
    if find_lines == 1 {
        return false;
    }
    let trimmed_candidate = candidate.trim();
    let trimmed_find = find.trim();
    trimmed_candidate.len() > (trimmed_find.len() + 500).max(trimmed_find.len() * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_exact_match() {
        let out = replace("foo bar baz", "bar", "BAR", false).unwrap();
        assert_eq!(out, "foo BAR baz");
    }

    #[test]
    fn rejects_identical_strings() {
        let err = replace("foo", "foo", "foo", false).unwrap_err();
        assert!(err.contains("identical"));
    }

    #[test]
    fn rejects_empty_old() {
        let err = replace("foo", "", "bar", false).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn multiple_matches_without_replace_all_errors() {
        let err = replace("foo foo foo", "foo", "bar", false).unwrap_err();
        assert!(err.contains("multiple matches"));
    }

    #[test]
    fn replace_all_replaces_every_occurrence() {
        let out = replace("foo foo foo", "foo", "bar", true).unwrap();
        assert_eq!(out, "bar bar bar");
    }

    #[test]
    fn line_trimmed_relaxes_whitespace() {
        let out = replace("   foo   \n  bar  ", "foo\nbar", "BAZ", false).unwrap();
        assert_eq!(out, "BAZ");
    }

    #[test]
    fn indentation_flexible_strips_common_indent() {
        let original = "    foo\n    bar";
        let find = "foo\n        bar";
        let out = replace(original, find, "BAZ", false).unwrap();
        assert_eq!(out, "BAZ");
    }

    #[test]
    fn detect_and_convert_preserves_crlf() {
        let content = "alpha\r\nbeta\r\ngamma";
        let ending = detect_line_ending(content);
        assert_eq!(ending, LineEnding::Crlf);
        let converted = convert_to_line_ending("a\nb", ending);
        assert_eq!(converted, "a\r\nb");
    }

    #[test]
    fn not_found_message_for_missing_substring() {
        let err = replace("hello world", "xyz", "abc", false).unwrap_err();
        assert!(err.contains("Could not find"));
    }
}
