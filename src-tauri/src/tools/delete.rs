use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    line_count, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_ACTION,
};

pub const NAME: &str = "delete";

const DESCRIPTION: &str = "Delete a file or empty directory. Out-of-workspace paths require an interactive user confirmation. Removes file lines from the context counter when a file is deleted.";

pub struct DeleteTool;

impl Tool for DeleteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path to delete"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        match parse_target(args, ctx) {
            Err(outcome) => outcome,
            Ok(target)
                if super::tool_utils::workspace::reject_if_unconfirmed(
                    &target.resolved,
                    ctx.workspace_path().as_deref(),
                ) =>
            {
                super::action_error(
                    &target.rel,
                    "delete outside the workspace must run on the async dispatch path.",
                )
            }
            Ok(target) => apply_delete(&target),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let raw = args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    super::tool_utils::workspace::guard(ctx, raw, "Delete", true, || DeleteTool.execute(&args, ctx))
        .await
}

struct DeleteTarget {
    resolved: PathBuf,
    rel: String,
}

fn parse_target(args: &Value, ctx: &ToolContext<'_>) -> Result<DeleteTarget, ToolOutcome> {
    let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
        return Err(super::action_error(
            "",
            "delete tool requires a string `path`.",
        ));
    };
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(super::action_error("", "delete tool `path` is empty."));
    }
    let resolved = match resolve_path(ctx, trimmed) {
        Ok(value) => value,
        Err(message) => return Err(super::action_error(trimmed, &message)),
    };
    let rel = ctx.relative_path(&resolved);
    Ok(DeleteTarget { resolved, rel })
}

fn apply_delete(target: &DeleteTarget) -> ToolOutcome {
    let resolved = &target.resolved;
    let rel = &target.rel;
    let metadata = match fs::symlink_metadata(resolved) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return super::action_error(rel, &format!("Path not found: {}", resolved.display()));
        }
        Err(error) => {
            return super::action_error(
                rel,
                &format!("Unable to stat `{}`: {error}", resolved.display()),
            );
        }
    };

    if metadata.file_type().is_dir() {
        if let Err(error) = fs::remove_dir(resolved) {
            if error.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                return super::action_error(
                    rel,
                    &format!(
                        "Directory is not empty: {} (recursive deletion is not supported)",
                        resolved.display()
                    ),
                );
            }
            return super::action_error(
                rel,
                &format!("Unable to delete `{}`: {error}", resolved.display()),
            );
        }
        return ToolOutcome {
            text: toon_doc(&[
                ("path", ToonValue::Str(rel)),
                ("status", ToonValue::Str("deleted")),
                ("kind", ToonValue::Str("directory")),
            ]),
            display: ToolDisplay {
                kind: TOOL_KIND_ACTION.to_string(),
                path: Some(rel.clone()),
                status: Some("ok".into()),
                ..ToolDisplay::default()
            },
            snapshot: None,
            image_png: None,
            file: None,
        };
    }

    let lines = line_count(&fs::read_to_string(resolved).unwrap_or_default());
    if let Err(error) = fs::remove_file(resolved) {
        return super::action_error(
            rel,
            &format!("Unable to delete `{}`: {error}", resolved.display()),
        );
    }

    ToolOutcome {
        text: toon_doc(&[
            ("path", ToonValue::Str(rel)),
            ("status", ToonValue::Str("deleted")),
            ("kind", ToonValue::Str("file")),
            ("linesRemoved", ToonValue::Int(lines as i64)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            path: Some(rel.clone()),
            status: Some("ok".into()),
            lines_removed: Some(lines),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "k-agent-delete-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn deletes_workspace_file() {
        let dir = tempdir();
        let file = dir.join("gone.txt");
        fs::write(&file, "x").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = execute_async(r#"{"path":"gone.txt"}"#, &ctx).await;
        assert_eq!(outcome.display.status.as_deref(), Some("ok"));
        assert!(!file.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_missing_path() {
        let dir = tempdir();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = DeleteTool.execute(&json!({}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        let _ = fs::remove_dir_all(&dir);
    }
}
