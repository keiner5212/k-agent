use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{
    ask_user::{ask_user_wait, AskUserAnswer, AskUserOption, AskUserQuestion},
    line_count, yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue,
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
            Ok(target) if target.outside_workspace => super::action_error(
                &target.rel,
                "delete outside the workspace must run on the async dispatch path.",
            ),
            Ok(target) => apply_delete(&target),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let target = match parse_target(&args, ctx) {
        Ok(value) => value,
        Err(outcome) => return outcome,
    };
    if target.outside_workspace {
        let confirmed = confirm_destructive(ctx, &target.resolved, &target.rel).await;
        if !confirmed {
            return super::action_error(&target.rel, "User denied the destructive action.");
        }
    }
    apply_delete(&target)
}

struct DeleteTarget {
    resolved: PathBuf,
    rel: String,
    outside_workspace: bool,
}

fn parse_target(args: &Value, ctx: &ToolContext<'_>) -> Result<DeleteTarget, ToolOutcome> {
    let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
        return Err(super::action_error("", "delete tool requires a string `path`."));
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
    let workspace = ctx.workspace_path();
    let outside_workspace = match workspace.as_ref() {
        Some(root) => !resolved.starts_with(root),
        None => true,
    };
    Ok(DeleteTarget {
        resolved,
        rel,
        outside_workspace,
    })
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
            text: yaml_doc(&[
                ("path", YamlValue::Str(rel)),
                ("status", YamlValue::Str("deleted")),
                ("kind", YamlValue::Str("directory")),
            ]),
            display: ToolDisplay {
                kind: TOOL_KIND_ACTION.to_string(),
                path: Some(rel.clone()),
                status: Some("ok".into()),
                ..ToolDisplay::default()
            },
            snapshot: None,
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
        text: yaml_doc(&[
            ("path", YamlValue::Str(rel)),
            ("status", YamlValue::Str("deleted")),
            ("kind", YamlValue::Str("file")),
            ("linesRemoved", YamlValue::Int(lines as i64)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            path: Some(rel.clone()),
            status: Some("ok".into()),
            lines_removed: Some(lines),
            ..ToolDisplay::default()
        },
        snapshot: None,
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

async fn confirm_destructive(ctx: &ToolContext<'_>, resolved: &Path, rel: &str) -> bool {
    let kind_label = if resolved.is_dir() { "directory" } else { "file" };
    let questions = vec![AskUserQuestion {
        id: "delete_confirm".to_string(),
        header: "Confirm deletion".to_string(),
        question: format!(
            "`{rel}` is outside the workspace. Delete this {kind_label} at `{}`? This cannot be undone.",
            resolved.display()
        ),
        options: vec![
            AskUserOption {
                label: "Yes, delete".to_string(),
                description: Some("Permanently delete this file/directory.".to_string()),
                preview: None,
            },
            AskUserOption {
                label: "No, keep it".to_string(),
                description: Some("Cancel the deletion and leave the file untouched.".to_string()),
                preview: None,
            },
        ],
        multi_select: false,
        allow_free_text: false,
    }];
    let call_id = format!("delete_confirm::{}", ctx.call_id);
    let answer: AskUserAnswer = ask_user_wait(ctx, &call_id, &questions).await;
    let entry = answer
        .iter()
        .find(|entry| entry.question_id == "delete_confirm");
    match entry {
        Some(entry) if entry.skipped => false,
        Some(entry) => entry.selected.iter().any(|label| label == "Yes, delete"),
        None => false,
    }
}
