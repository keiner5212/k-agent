use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "create_folder";

const DESCRIPTION: &str = "Create a directory at an absolute or workspace-relative path. Parent directories are created. No-op if the directory already exists.";

pub struct CreateFolderTool;

impl Tool for CreateFolderTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "dirPath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path"
                    }
                },
                "required": ["dirPath"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(raw_path) = args.get("dirPath").and_then(Value::as_str) else {
            return super::action_error("", "create_folder tool requires a string `dirPath`.");
        };
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return super::action_error("", "create_folder tool `dirPath` is empty.");
        }
        let resolved = match resolve_path(ctx, trimmed) {
            Ok(value) => value,
            Err(message) => return super::action_error(trimmed, &message),
        };
        let rel = ctx.relative_path(&resolved);

        let metadata = match fs::metadata(&resolved) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(create_err) = fs::create_dir_all(&resolved) {
                    return super::action_error(
                        &rel,
                        &format!(
                            "Unable to create directory `{}`: {create_err}",
                            resolved.display()
                        ),
                    );
                }
                return ToolOutcome {
                    text: yaml_doc(&[
                        ("path", YamlValue::Str(&rel)),
                        ("status", YamlValue::Str("created")),
                    ]),
                    display: ToolDisplay {
                        kind: TOOL_KIND_ACTION.to_string(),
                        path: Some(rel),
                        status: Some("ok".into()),
                        ..ToolDisplay::default()
                    },
                    snapshot: None,
                };
            }
            Err(error) => {
                return super::action_error(
                    &rel,
                    &format!("Unable to stat `{}`: {error}", resolved.display()),
                );
            }
        };

        if metadata.is_dir() {
            return ToolOutcome {
                text: yaml_doc(&[
                    ("path", YamlValue::Str(&rel)),
                    ("status", YamlValue::Str("exists")),
                ]),
                display: ToolDisplay {
                    kind: TOOL_KIND_ACTION.to_string(),
                    path: Some(rel),
                    status: Some("ok".into()),
                    ..ToolDisplay::default()
                },
                snapshot: None,
            };
        }

        super::action_error(
            &rel,
            &format!("Path already exists and is not a directory: {}", resolved.display()),
        )
    }
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}