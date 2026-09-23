use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "create_folder";

const DESCRIPTION: &str = "Create a directory at an absolute or workspace-relative path. Paths outside the workspace wait for the user to allow or deny. Parent directories are created. No-op if the directory already exists.";

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
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &resolved,
            ctx.workspace_path().as_deref(),
        ) {
            return super::action_error(
                trimmed,
                "create_folder outside the workspace must run on the async dispatch path.",
            );
        }
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
                    text: toon_doc(&[
                        ("path", ToonValue::Str(&rel)),
                        ("status", ToonValue::Str("created")),
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
                text: toon_doc(&[
                    ("path", ToonValue::Str(&rel)),
                    ("status", ToonValue::Str("exists")),
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
            &format!(
                "Path already exists and is not a directory: {}",
                resolved.display()
            ),
        )
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let raw = args
        .get("dirPath")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    super::tool_utils::workspace::guard(ctx, raw, "Create", true, || {
        CreateFolderTool.execute(&args, ctx)
    })
    .await
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
            "k-agent-mkdir-test-{}-{}",
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
    fn creates_nested_folder() {
        let dir = tempdir();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = CreateFolderTool.execute(&json!({"dirPath": "a/b"}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("ok"));
        assert!(dir.join("a/b").is_dir());
        let again = CreateFolderTool.execute(&json!({"dirPath": "a/b"}), &ctx);
        assert_eq!(again.display.status.as_deref(), Some("ok"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_empty_path() {
        let dir = tempdir();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = CreateFolderTool.execute(&json!({"dirPath": "  "}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        let _ = fs::remove_dir_all(&dir);
    }
}
