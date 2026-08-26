use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutcome, ToolSpec};

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
            return ToolOutcome {
                text: "write tool requires a string `content`.".into(),
            };
        };
        let Some(raw_path) = args.get("filePath").and_then(Value::as_str) else {
            return ToolOutcome {
                text: "write tool requires a string `filePath`.".into(),
            };
        };
        let trimmed_path = raw_path.trim();
        if trimmed_path.is_empty() {
            return ToolOutcome {
                text: "write tool `filePath` is empty.".into(),
            };
        }
        let resolved = match resolve_path(ctx, trimmed_path) {
            Ok(value) => value,
            Err(message) => return ToolOutcome { text: message },
        };
        if let Some(parent) = resolved.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(error) = fs::create_dir_all(parent) {
                    return ToolOutcome {
                        text: format!(
                            "Unable to create parent directory `{}`: {error}",
                            parent.display()
                        ),
                    };
                }
            }
        }
        match fs::write(&resolved, content) {
            Ok(()) => ToolOutcome {
                text: format!(
                    "Wrote file successfully: {} ({} bytes).",
                    resolved.display(),
                    content.len()
                ),
            },
            Err(error) => ToolOutcome {
                text: format!("Unable to write `{}`: {error}", resolved.display()),
            },
        }
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}
