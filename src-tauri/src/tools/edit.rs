use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use super::{
    line_add_remove, toon_doc, FileSnapshot, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec,
    ToonValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "edit";

const DESCRIPTION: &str = "Exact string replace in one or more files. Pass filePath/oldString/newString for one edit, or edits for several. All edits are checked before any file is written. Read first. Fails if oldString is missing or not unique, unless replaceAll is true. Paths outside the workspace wait for the user to allow or deny.";

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
                    },
                    "edits": {
                        "type": "array",
                        "description": "Several edits applied together. Each item has filePath, oldString, newString, and optional replaceAll. Do not also set the top-level fields.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "filePath": { "type": "string" },
                                "oldString": { "type": "string" },
                                "newString": { "type": "string" },
                                "replaceAll": { "type": "boolean" }
                            },
                            "required": ["filePath", "oldString", "newString"]
                        }
                    }
                },
                "anyOf": [
                    { "required": ["filePath", "oldString", "newString"] },
                    { "required": ["edits"] }
                ]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let ops = match parse_edits(args) {
            Ok(ops) => ops,
            Err(message) => return super::action_error("", &message),
        };
        apply_edits(ctx, &ops)
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let ops = match parse_edits(&args) {
        Ok(ops) => ops,
        Err(message) => return super::action_error("", &message),
    };
    let mut seen = Vec::new();
    for op in &ops {
        if seen.iter().any(|path: &String| path == &op.file_path) {
            continue;
        }
        seen.push(op.file_path.clone());
        let probe = super::tool_utils::workspace::guard(ctx, &op.file_path, "Edit", true, || {
            super::ToolOutcome::text("ok")
        })
        .await;
        if probe.text.contains("denied") {
            return probe;
        }
    }
    EditTool.execute(&args, ctx)
}

struct EditRequest {
    file_path: String,
    old_string: String,
    new_string: String,
    replace_all: bool,
}

fn parse_edits(args: &Value) -> Result<Vec<EditRequest>, String> {
    let has_single = args.get("filePath").is_some()
        || args.get("oldString").is_some()
        || args.get("newString").is_some();
    if let Some(edits) = args.get("edits") {
        if has_single {
            return Err(
                "edit accepts either filePath/oldString/newString or `edits`, not both.".into(),
            );
        }
        let items = edits.as_array().ok_or("edit `edits` must be an array.")?;
        if items.is_empty() {
            return Err("edit `edits` is empty.".into());
        }
        let mut ops = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            ops.push(parse_one(item).map_err(|error| format!("edit edits[{index}]: {error}"))?);
        }
        return Ok(ops);
    }
    Ok(vec![parse_one(args)?])
}

fn parse_one(args: &Value) -> Result<EditRequest, String> {
    let file_path = args
        .get("filePath")
        .and_then(Value::as_str)
        .ok_or("edit requires a string `filePath`.")?
        .trim()
        .to_string();
    if file_path.is_empty() {
        return Err("edit `filePath` is empty.".into());
    }
    let old_string = args
        .get("oldString")
        .and_then(Value::as_str)
        .ok_or("edit requires a string `oldString`.")?
        .to_string();
    let new_string = args
        .get("newString")
        .and_then(Value::as_str)
        .ok_or("edit requires a string `newString`.")?
        .to_string();
    if old_string == new_string {
        return Err("No changes to apply: oldString and newString are identical.".into());
    }
    let replace_all = args
        .get("replaceAll")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(EditRequest {
        file_path,
        old_string,
        new_string,
        replace_all,
    })
}

struct StagedEdit {
    resolved: PathBuf,
    rel: String,
    before: String,
    after: String,
}

fn apply_edits(ctx: &ToolContext<'_>, ops: &[EditRequest]) -> ToolOutcome {
    let mut order: Vec<PathBuf> = Vec::new();
    let mut grouped: HashMap<PathBuf, Vec<&EditRequest>> = HashMap::new();
    for op in ops {
        let resolved = match resolve_path(ctx, &op.file_path) {
            Ok(path) => path,
            Err(message) => return super::action_error(&op.file_path, &message),
        };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &resolved,
            ctx.workspace_path().as_deref(),
        ) {
            return super::action_error(
                &op.file_path,
                "edit outside the workspace must run on the async dispatch path.",
            );
        }
        if !grouped.contains_key(&resolved) {
            order.push(resolved.clone());
        }
        grouped.entry(resolved).or_default().push(op);
    }
    order.sort();
    let mut guards = Vec::new();
    for path in &order {
        match file_lock_for(path) {
            Ok(guard) => guards.push(guard),
            Err(message) => return super::action_error(&ctx.relative_path(path), &message),
        }
    }
    let mut staged = Vec::new();
    for path in &order {
        let rel = ctx.relative_path(path);
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return super::action_error(&rel, &format!("File not found: {}", path.display()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                return super::action_error(
                    &rel,
                    &format!("Cannot edit binary file: {}", path.display()),
                );
            }
            Err(error) => {
                return super::action_error(
                    &rel,
                    &format!("Unable to read `{}`: {error}", path.display()),
                );
            }
        };
        let mut next = content.clone();
        for op in grouped.get(path).into_iter().flatten() {
            let ending = detect_line_ending(&next);
            let old = convert_to_line_ending(&normalize_line_endings(&op.old_string), ending);
            let new = convert_to_line_ending(&normalize_line_endings(&op.new_string), ending);
            next = match replace(&next, &old, &new, op.replace_all) {
                Ok(value) => value,
                Err(error) => return super::action_error(&rel, &error),
            };
        }
        staged.push(StagedEdit {
            resolved: path.clone(),
            rel,
            before: content,
            after: next,
        });
    }
    for (index, file) in staged.iter().enumerate() {
        if let Err(error) = fs::write(&file.resolved, &file.after) {
            for written in staged.iter().take(index) {
                let _ = fs::write(&written.resolved, &written.before);
            }
            return super::action_error(
                &file.rel,
                &format!("Unable to write `{}`: {error}", file.resolved.display()),
            );
        }
    }
    drop(guards);
    let mut added_total = 0u32;
    let mut removed_total = 0u32;
    let mut lines = Vec::new();
    for file in &staged {
        let (added, removed) = line_add_remove(&file.before, &file.after);
        added_total += added;
        removed_total += removed;
        lines.push(format!("{} +{added} -{removed}", file.rel));
    }
    let files = lines.join("\n");
    let first = staged
        .first()
        .map(|file| file.rel.clone())
        .unwrap_or_default();
    let snapshot = pack_snapshot(&staged);
    if staged.len() == 1 {
        return ToolOutcome {
            text: toon_doc(&[
                ("path", ToonValue::Str(&first)),
                ("status", ToonValue::Str("ok")),
                ("added", ToonValue::Int(added_total as i64)),
                ("removed", ToonValue::Int(removed_total as i64)),
            ]),
            display: ToolDisplay {
                kind: TOOL_KIND_ACTION.to_string(),
                path: Some(first),
                added: Some(added_total),
                removed: Some(removed_total),
                status: Some("ok".into()),
                ..ToolDisplay::default()
            },
            snapshot: Some(snapshot),
            image_png: None,
        };
    }
    let count = staged.len() as i64;
    ToolOutcome {
        text: toon_doc(&[
            ("status", ToonValue::Str("ok")),
            ("count", ToonValue::Int(count)),
            ("added", ToonValue::Int(added_total as i64)),
            ("removed", ToonValue::Int(removed_total as i64)),
            ("files", ToonValue::Block(&files)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            path: Some(first),
            added: Some(added_total),
            removed: Some(removed_total),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: Some(snapshot),
        image_png: None,
    }
}

fn pack_snapshot(staged: &[StagedEdit]) -> FileSnapshot {
    if staged.len() == 1 {
        return FileSnapshot {
            before: staged[0].before.clone(),
            after: staged[0].after.clone(),
        };
    }
    let mut before = String::new();
    let mut after = String::new();
    for file in staged {
        before.push_str(&format!("===== {}\n", file.rel));
        before.push_str(&file.before);
        if !file.before.ends_with('\n') {
            before.push('\n');
        }
        after.push_str(&format!("===== {}\n", file.rel));
        after.push_str(&file.after);
        if !file.after.ends_with('\n') {
            after.push('\n');
        }
    }
    FileSnapshot { before, after }
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
    use serde_json::json;

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

    #[test]
    fn execute_requires_file_path() {
        let dir = std::env::temp_dir().join(format!("k-agent-edit-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = EditTool.execute(&json!({}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
