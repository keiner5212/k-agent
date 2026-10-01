use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    line_add_remove, toon_doc, FileSnapshot, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec,
    ToonValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "apply_patch";

const DESCRIPTION: &str = "\
Apply one patch that adds, updates, or deletes files. All files are checked before any write. \
Use this when several files change together. One exact span in one file still uses edit. \
Envelope: *** Begin Patch, then *** Add File, *** Update File, or *** Delete File, then *** End Patch. \
Update hunks use @@ then lines prefixed with space (context), - (remove), or + (add). \
The context must match the file once. An optional *** Move to renames an update. \
Paths outside the workspace wait for the user.\
";

const MAX_PATCH_CHARS: usize = 200_000;

pub struct ApplyPatchTool;

impl Tool for ApplyPatchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "patchText": {
                        "type": "string",
                        "description": "Full patch, from *** Begin Patch through *** End Patch."
                    }
                },
                "required": ["patchText"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(text) = args.get("patchText").and_then(Value::as_str) else {
            return super::action_error("", "apply_patch requires a string `patchText`.");
        };
        if text.chars().count() > MAX_PATCH_CHARS {
            return super::action_error("", "apply_patch patchText exceeds 200000 chars.");
        }
        let files = match parse_patch(text) {
            Ok(files) => files,
            Err(message) => return super::action_error("", &message),
        };
        apply_files(ctx, &files)
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let text = args.get("patchText").and_then(Value::as_str).unwrap_or("");
    let paths = parse_patch(text).map(|files| {
        let mut out = Vec::new();
        for file in files {
            if let Some(dest) = file.move_to {
                out.push(dest);
            }
            out.push(file.path);
        }
        out
    });
    if let Ok(paths) = &paths {
        let mut seen = Vec::new();
        for path in paths {
            if seen.iter().any(|item: &String| item == path) {
                continue;
            }
            seen.push(path.clone());
            let probe = super::tool_utils::workspace::guard(ctx, path, "Patch", true, || {
                super::ToolOutcome::text("ok")
            })
            .await;
            if probe.text.contains("denied") {
                return probe;
            }
        }
    }
    let mut outcome = ApplyPatchTool.execute(&args, ctx);
    if outcome.display.status.as_deref() == Some("ok") {
        if let Ok(paths) = paths {
            let mut resolved = Vec::new();
            for raw in paths {
                if let Ok(path) =
                    crate::pathutil::resolve_tool_path(&raw, ctx.workspace_path().as_deref())
                {
                    resolved.push(path);
                }
            }
            super::attach_lsp_diagnostics(ctx, &mut outcome, &resolved).await;
        }
    }
    outcome
}

#[derive(Clone)]
enum FileKind {
    Add,
    Delete,
    Update,
}

struct PatchFile {
    kind: FileKind,
    path: String,
    move_to: Option<String>,
    hunks: Vec<Vec<HunkLine>>,
    added: String,
}

#[derive(Clone)]
enum HunkLine {
    Keep(String),
    Del(String),
    Add(String),
}

fn parse_patch(text: &str) -> Result<Vec<PatchFile>, String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == "*** Begin Patch")
        .ok_or_else(|| "apply_patch needs `*** Begin Patch`.".to_string())?;
    let end = lines
        .iter()
        .rposition(|line| line.trim() == "*** End Patch")
        .ok_or_else(|| "apply_patch needs `*** End Patch`.".to_string())?;
    if end <= start {
        return Err("apply_patch patch has no body.".into());
    }
    let mut files = Vec::new();
    let mut index = start + 1;
    while index < end {
        let line = lines[index].trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Add File:") {
            let path = path.trim().to_string();
            index += 1;
            let mut body = String::new();
            while index < end && !lines[index].starts_with("*** ") {
                let row = lines[index];
                let Some(rest) = row.strip_prefix('+') else {
                    return Err(format!("apply_patch add lines must start with +: {path}"));
                };
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(rest);
                index += 1;
            }
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            files.push(PatchFile {
                kind: FileKind::Add,
                path,
                move_to: None,
                hunks: Vec::new(),
                added: body,
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File:") {
            files.push(PatchFile {
                kind: FileKind::Delete,
                path: path.trim().to_string(),
                move_to: None,
                hunks: Vec::new(),
                added: String::new(),
            });
            index += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File:") {
            let path = path.trim().to_string();
            index += 1;
            let mut move_to = None;
            if index < end {
                if let Some(dest) = lines[index].trim().strip_prefix("*** Move to:") {
                    move_to = Some(dest.trim().to_string());
                    index += 1;
                }
            }
            let mut hunks = Vec::new();
            while index < end && !lines[index].starts_with("*** ") {
                if !lines[index].starts_with("@@") {
                    return Err(format!("apply_patch update {path} needs a @@ hunk."));
                }
                index += 1;
                let mut hunk = Vec::new();
                while index < end
                    && !lines[index].starts_with("@@")
                    && !lines[index].starts_with("*** ")
                {
                    let row = lines[index];
                    if row.is_empty() {
                        index += 1;
                        continue;
                    }
                    if let Some(rest) = row.strip_prefix('+') {
                        hunk.push(HunkLine::Add(rest.to_string()));
                    } else if let Some(rest) = row.strip_prefix('-') {
                        hunk.push(HunkLine::Del(rest.to_string()));
                    } else if let Some(rest) = row.strip_prefix(' ') {
                        hunk.push(HunkLine::Keep(rest.to_string()));
                    } else {
                        return Err(format!(
                            "apply_patch hunk line in {path} must start with space, +, or -."
                        ));
                    }
                    index += 1;
                }
                if hunk.is_empty() {
                    return Err(format!("apply_patch update {path} has an empty hunk."));
                }
                hunks.push(hunk);
            }
            if hunks.is_empty() {
                return Err(format!("apply_patch update {path} has no hunks."));
            }
            files.push(PatchFile {
                kind: FileKind::Update,
                path,
                move_to,
                hunks,
                added: String::new(),
            });
            continue;
        }
        return Err(format!("apply_patch does not understand `{line}`."));
    }
    if files.is_empty() {
        return Err("apply_patch patch has no file changes.".into());
    }
    Ok(files)
}

struct Planned {
    rel: String,
    source: PathBuf,
    dest: PathBuf,
    before: String,
    after: String,
    remove_source: bool,
}

fn apply_files(ctx: &ToolContext<'_>, files: &[PatchFile]) -> ToolOutcome {
    let mut planned = Vec::new();
    for file in files {
        if file.path.is_empty() {
            return super::action_error("", "apply_patch file path is empty.");
        }
        let source = match resolve(ctx, &file.path) {
            Ok(path) => path,
            Err(message) => return super::action_error(&file.path, &message),
        };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &source,
            ctx.workspace_path().as_deref(),
        ) {
            return super::action_error(
                &file.path,
                "apply_patch outside the workspace must run on the async dispatch path.",
            );
        }
        let dest_raw = file.move_to.as_deref().unwrap_or(&file.path);
        let dest = match resolve(ctx, dest_raw) {
            Ok(path) => path,
            Err(message) => return super::action_error(dest_raw, &message),
        };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &dest,
            ctx.workspace_path().as_deref(),
        ) {
            return super::action_error(
                dest_raw,
                "apply_patch outside the workspace must run on the async dispatch path.",
            );
        }
        if !matches!(file.kind, FileKind::Add) {
            if let Err(message) = super::require_prior_read(ctx, &ctx.relative_path(&source)) {
                return super::action_error(&file.path, &message);
            }
        }
        let rel = ctx.relative_path(if matches!(file.kind, FileKind::Delete) {
            &source
        } else {
            &dest
        });
        match file.kind {
            FileKind::Add => {
                if source.exists() {
                    return super::action_error(&rel, &format!("File already exists: {rel}"));
                }
                planned.push(Planned {
                    rel,
                    source: source.clone(),
                    dest: source,
                    before: String::new(),
                    after: file.added.clone(),
                    remove_source: false,
                });
            }
            FileKind::Delete => {
                let before = match fs::read_to_string(&source) {
                    Ok(text) => text,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return super::action_error(&rel, &format!("File not found: {rel}"));
                    }
                    Err(error) => {
                        return super::action_error(
                            &rel,
                            &format!("Unable to read `{rel}`: {error}"),
                        );
                    }
                };
                planned.push(Planned {
                    rel,
                    source,
                    dest,
                    before,
                    after: String::new(),
                    remove_source: true,
                });
            }
            FileKind::Update => {
                let before = match fs::read_to_string(&source) {
                    Ok(text) => text,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return super::action_error(&rel, &format!("File not found: {rel}"));
                    }
                    Err(error) => {
                        return super::action_error(
                            &rel,
                            &format!("Unable to read `{rel}`: {error}"),
                        );
                    }
                };
                let after = match apply_hunks(&before, &file.hunks) {
                    Ok(text) => text,
                    Err(message) => return super::action_error(&file.path, &message),
                };
                planned.push(Planned {
                    rel,
                    source,
                    dest,
                    before,
                    after,
                    remove_source: file.move_to.is_some(),
                });
            }
        }
    }
    let mut written: Vec<(PathBuf, Option<String>)> = Vec::new();
    for item in &planned {
        if item.remove_source && item.after.is_empty() && item.source == item.dest {
            continue;
        }
        if let Some(parent) = item.dest.parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                restore(&written);
                return super::action_error(&item.rel, "Unable to create parent directory.");
            }
        }
        let previous = fs::read_to_string(&item.dest).ok();
        if fs::write(&item.dest, &item.after).is_err() {
            restore(&written);
            return super::action_error(&item.rel, "Unable to write file.");
        }
        written.push((item.dest.clone(), previous));
    }
    for item in &planned {
        if item.remove_source && item.source != item.dest {
            if fs::remove_file(&item.source).is_err() {
                restore(&written);
                return super::action_error(&item.rel, "Unable to remove the old path.");
            }
        } else if item.remove_source && item.after.is_empty() {
            if fs::remove_file(&item.source).is_err() {
                restore(&written);
                return super::action_error(&item.rel, "Unable to delete file.");
            }
        }
    }
    let mut added = 0u32;
    let mut removed = 0u32;
    let mut lines = Vec::new();
    for item in &planned {
        let (add, rem) = line_add_remove(&item.before, &item.after);
        added += add;
        removed += rem;
        lines.push(item.rel.clone());
    }
    let summary = lines.join("\n");
    let count = planned.len() as i64;
    let snapshot = if planned.len() == 1 {
        Some(FileSnapshot {
            before: planned[0].before.clone(),
            after: planned[0].after.clone(),
        })
    } else {
        None
    };
    ToolOutcome {
        text: toon_doc(&[
            ("status", ToonValue::Str("ok")),
            ("added", ToonValue::Int(i64::from(added))),
            ("removed", ToonValue::Int(i64::from(removed))),
            ("count", ToonValue::Int(count)),
            ("files", ToonValue::Block(&summary)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            path: planned.first().map(|item| item.rel.clone()),
            status: Some("ok".into()),
            added: Some(added),
            removed: Some(removed),
            ..ToolDisplay::default()
        },
        snapshot,
        image_png: None,
        file: None,
    }
}

fn restore(written: &[(PathBuf, Option<String>)]) {
    for (path, previous) in written.iter().rev() {
        match previous {
            Some(text) => {
                let _ = fs::write(path, text);
            }
            None => {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn apply_hunks(content: &str, hunks: &[Vec<HunkLine>]) -> Result<String, String> {
    let crlf = content.contains("\r\n");
    let normalized = content.replace("\r\n", "\n");
    let trailing = normalized.ends_with('\n');
    let mut lines: Vec<String> = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized
            .trim_end_matches('\n')
            .split('\n')
            .map(str::to_string)
            .collect()
    };
    for hunk in hunks {
        let old: Vec<&str> = hunk
            .iter()
            .filter_map(|line| match line {
                HunkLine::Keep(text) | HunkLine::Del(text) => Some(text.as_str()),
                HunkLine::Add(_) => None,
            })
            .collect();
        let new: Vec<String> = hunk
            .iter()
            .filter_map(|line| match line {
                HunkLine::Keep(text) | HunkLine::Add(text) => Some(text.clone()),
                HunkLine::Del(_) => None,
            })
            .collect();
        if old.is_empty() {
            if lines.iter().any(|line| !line.is_empty()) {
                return Err("apply_patch hunk has no context. Include unchanged lines.".into());
            }
            lines = new;
            continue;
        }
        let mut hits = Vec::new();
        if lines.len() >= old.len() {
            for index in 0..=lines.len() - old.len() {
                if lines[index..index + old.len()]
                    .iter()
                    .zip(&old)
                    .all(|(have, want)| have == want)
                {
                    hits.push(index);
                }
            }
        }
        if hits.len() != 1 {
            return Err(format!(
                "apply_patch hunk matched {} times. Add surrounding context so it matches once.",
                hits.len()
            ));
        }
        let index = hits[0];
        let mut next = Vec::with_capacity(lines.len() - old.len() + new.len());
        next.extend_from_slice(&lines[..index]);
        next.extend(new);
        next.extend_from_slice(&lines[index + old.len()..]);
        lines = next;
    }
    let mut out = lines.join("\n");
    if trailing || !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    Ok(out)
}

fn resolve(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "k-agent-patch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn adds_updates_and_deletes_in_one_patch() {
        let dir = tempdir();
        fs::write(dir.join("old.txt"), "alpha\nbeta\n").unwrap();
        fs::write(dir.join("gone.txt"), "bye\n").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let patch = "\
*** Begin Patch
*** Add File: new.txt
+hello
*** Update File: old.txt
@@
 alpha
-beta
+BETA
*** Delete File: gone.txt
*** End Patch
";
        let outcome = ApplyPatchTool.execute(&json!({ "patchText": patch }), &ctx);
        assert_eq!(
            outcome.display.status.as_deref(),
            Some("ok"),
            "{}",
            outcome.text
        );
        assert_eq!(fs::read_to_string(dir.join("new.txt")).unwrap(), "hello\n");
        assert_eq!(
            fs::read_to_string(dir.join("old.txt")).unwrap(),
            "alpha\nBETA\n"
        );
        assert!(!dir.join("gone.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_hunk_writes_nothing() {
        let dir = tempdir();
        fs::write(dir.join("a.txt"), "one\n").unwrap();
        fs::write(dir.join("b.txt"), "two\n").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let patch = "\
*** Begin Patch
*** Update File: a.txt
@@
 one
+ONE
*** Update File: b.txt
@@
 missing
+x
*** End Patch
";
        let outcome = ApplyPatchTool.execute(&json!({ "patchText": patch }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
