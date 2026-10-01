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

const DESCRIPTION: &str = "Exact string replace in one or more files. Pass filePath, oldString, and newString for one edit, or edits for several. All edits are checked before any file is written. Read first. oldString is the file text, not the line-number prefix from read. Match is exact, including whitespace. Fails if oldString is missing or not unique, unless replaceAll is true, and the error says which. Paths outside the workspace wait for the user to allow or deny.";

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
    let mut outcome = EditTool.execute(&args, ctx);
    if outcome.display.status.as_deref() == Some("ok") {
        let mut paths = Vec::new();
        for path in &seen {
            if let Ok(resolved) = resolve_path(ctx, path) {
                paths.push(resolved);
            }
        }
        super::attach_lsp_diagnostics(ctx, &mut outcome, &paths).await;
    }
    outcome
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
            file: None,
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
        file: None,
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

    let mut first = None;
    let mut count = 0usize;
    let mut start = 0usize;
    while let Some(index) = content[start..].find(old_string) {
        let at = start + index;
        count += 1;
        if first.is_none() {
            first = Some(at);
        }
        if !replace_all && count > 1 {
            return Err(
                "Found multiple matches for oldString. Provide more surrounding context to make the match unique, or set replaceAll to true.".into(),
            );
        }
        start = at + old_string.len();
    }
    let Some(first) = first else {
        return Err(
            "Could not find oldString in the file. It must match exactly, including whitespace, indentation, and line endings.".into(),
        );
    };
    if replace_all {
        return Ok(content.replace(old_string, new_string));
    }
    let mut output = String::with_capacity(content.len() - old_string.len() + new_string.len());
    output.push_str(&content[..first]);
    output.push_str(new_string);
    output.push_str(&content[first + old_string.len()..]);
    Ok(output)
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
    fn whitespace_drift_does_not_match() {
        let err = replace("   foo   \n  bar  ", "foo\nbar", "BAZ", false).unwrap_err();
        assert!(err.contains("Could not find"));
    }

    #[test]
    fn indent_drift_does_not_match() {
        let err = replace("    foo\n    bar", "foo\n        bar", "BAZ", false).unwrap_err();
        assert!(err.contains("Could not find"));
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
