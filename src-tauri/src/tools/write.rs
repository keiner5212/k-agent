use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    line_add_remove, yaml_doc, FileSnapshot, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec,
    YamlValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "write";

const DESCRIPTION: &str = "Create or overwrite a file. Path is absolute or workspace-relative. Parent dirs are created. Read first when editing an existing file.";

pub struct WriteTool;

impl Tool for WriteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "File contents"
                    },
                    "filePath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path"
                    }
                },
                "required": ["content", "filePath"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(content) = args.get("content").and_then(Value::as_str) else {
            return super::action_error("", "write tool requires a string `content`.");
        };
        let Some(raw_path) = args.get("filePath").and_then(Value::as_str) else {
            return super::action_error("", "write tool requires a string `filePath`.");
        };
        let trimmed_path = raw_path.trim();
        if trimmed_path.is_empty() {
            return super::action_error("", "write tool `filePath` is empty.");
        }
        let resolved = match resolve_path(ctx, trimmed_path) {
            Ok(value) => value,
            Err(message) => return super::action_error(trimmed_path, &message),
        };
        let rel = ctx.relative_path(&resolved);
        if let Some(parent) = resolved.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(error) = fs::create_dir_all(parent) {
                    return super::action_error(
                        &rel,
                        &format!(
                            "Unable to create parent directory `{}`: {error}",
                            parent.display()
                        ),
                    );
                }
            }
        }
        let before = fs::read_to_string(&resolved).unwrap_or_default();
        match fs::write(&resolved, content) {
            Ok(()) => {
                let (added, removed) = line_add_remove(&before, content);
                ToolOutcome {
                    text: yaml_doc(&[
                        ("path", YamlValue::Str(&rel)),
                        ("status", YamlValue::Str("ok")),
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
                        before,
                        after: content.to_string(),
                    }),
                }
            }
            Err(error) => super::action_error(
                &rel,
                &format!("Unable to write `{}`: {error}", resolved.display()),
            ),
        }
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}
