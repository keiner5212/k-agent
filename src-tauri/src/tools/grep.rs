use std::process::Command;

use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "grep";

const DESCRIPTION: &str = "Search file contents with ripgrep. pattern is a regex. path defaults to the workspace. glob limits files (for example *.rs). Uses the configured worker cores. Results are capped.";

const MAX_MATCHES: usize = 100;
const MAX_LINE_CHARS: usize = 400;

pub struct GrepTool;

impl Tool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Ripgrep regex."
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory. Absolute or workspace-relative. Default: workspace root."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Optional glob, for example *.ts or *.{rs,toml}."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(pattern) = args.get("pattern").and_then(Value::as_str) else {
            return error_outcome("grep requires a string `pattern`.");
        };
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return error_outcome("grep `pattern` is empty.");
        }
        let raw_path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let resolved =
            match crate::pathutil::resolve_tool_path(raw_path, ctx.workspace_path().as_deref()) {
                Ok(path) => path,
                Err(error) => return error_outcome(&error.to_string()),
            };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &resolved,
            ctx.workspace_path().as_deref(),
        ) {
            return error_outcome(
                "grep outside the workspace must run on the async dispatch path.",
            );
        }
        let glob = args
            .get("glob")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let jobs = ctx.parallelism.clamp(1, 16);
        let mut command = Command::new("rg");
        command
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color")
            .arg("never")
            .arg("--max-columns")
            .arg(MAX_LINE_CHARS.to_string())
            .arg("--max-count")
            .arg("20")
            .arg("-j")
            .arg(jobs.to_string())
            .arg("--glob")
            .arg("!.git/**")
            .arg("--glob")
            .arg("!node_modules/**")
            .arg("--glob")
            .arg("!target/**");
        if !glob.is_empty() {
            command.arg("--glob").arg(glob);
        }
        command.arg("--").arg(pattern).arg(&resolved);
        let output = match command.output() {
            Ok(output) => output,
            Err(error) => {
                return error_outcome(&format!(
                    "grep could not start ripgrep (`rg`): {error}. Install ripgrep."
                ));
            }
        };
        if output.status.code() == Some(1) {
            return ok_outcome(pattern, "0", "false", "");
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return error_outcome(&format!("grep: {}", stderr.trim()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let workspace = ctx.workspace_path();
        let mut lines: Vec<String> = Vec::new();
        for line in stdout.lines() {
            if lines.len() >= MAX_MATCHES {
                break;
            }
            lines.push(trim_line(&relativize_match(line, workspace.as_deref())));
        }
        let truncated = stdout.lines().count() > lines.len();
        let body = lines.join("\n");
        ok_outcome(
            pattern,
            &lines.len().to_string(),
            if truncated { "true" } else { "false" },
            &body,
        )
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
    super::tool_utils::workspace::guard(ctx, raw, "Search", false, || GrepTool.execute(&args, ctx))
        .await
}

fn relativize_match(line: &str, workspace: Option<&std::path::Path>) -> String {
    let Some((path, rest)) = split_match(line) else {
        return line.to_string();
    };
    let Some(root) = workspace else {
        return line.to_string();
    };
    let relative = std::path::Path::new(path)
        .strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string());
    let relative = relative.trim_start_matches("./").to_string();
    format!("{relative}:{rest}")
}

fn split_match(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b':' {
            index += 1;
            continue;
        }
        let tail = &line[index + 1..];
        let digits = tail.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digits > 0 && tail.as_bytes().get(digits) == Some(&b':') {
            return Some((&line[..index], &tail));
        }
        index += 1;
    }
    None
}

fn trim_line(line: &str) -> String {
    let mut out = String::new();
    for ch in line.chars().take(MAX_LINE_CHARS) {
        out.push(ch);
    }
    out
}

fn ok_outcome(pattern: &str, count: &str, truncated: &str, matches: &str) -> ToolOutcome {
    let count_num: i64 = count.parse().unwrap_or(0);
    let text = toon_doc(&[
        ("pattern", ToonValue::Str(pattern)),
        ("count", ToonValue::Int(count_num)),
        ("truncated", ToonValue::Str(truncated)),
        ("matches", ToonValue::Block(matches)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
    }
}

fn error_outcome(message: &str) -> ToolOutcome {
    let text = toon_doc(&[
        ("status", ToonValue::Str("error")),
        ("error", ToonValue::Block(message)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("error".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
    }
}
