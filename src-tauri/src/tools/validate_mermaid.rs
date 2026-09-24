use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "validate_mermaid";

const DESCRIPTION: &str = "Check one mermaid diagram with the same parser the chat uses. Call this before a mermaid fence goes in the reply. status ok means the source parsed. status error returns the parser message. Fix the source and call again. Do not put a failed diagram in the reply.";

const MAX_SOURCE_CHARS: usize = 100_000;
const MAX_ERROR_CHARS: usize = 2_000;

const SHIM: &str = r#"const DOMPurify = {
  addHook() {},
  removeHook() {},
  sanitize(value) {
    return value == null ? "" : String(value);
  },
  isSupported: true,
};
export default DOMPurify;
"#;

const LOADER: &str = r#"export async function resolve(specifier, context, nextResolve) {
  if (specifier === "dompurify") {
    return {
      shortCircuit: true,
      url: new URL("./dompurify-shim.mjs", import.meta.url).href,
    };
  }
  return nextResolve(specifier, context);
}
"#;

const REGISTER: &str = r#"import { register } from "node:module";
register("./loader.mjs", import.meta.url);
"#;

const CHECK: &str = r#"import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const entry = process.env.MERMAID_ENTRY;
const resultPath = process.env.RESULT_PATH;
const fail = (error) => {
  writeFileSync(resultPath, JSON.stringify({ ok: false, error }));
  process.exit(0);
};
if (!entry || !resultPath) fail("validate_mermaid: parser paths were not set.");
const source = readFileSync(0, "utf8");
try {
  const mermaid = (await import(pathToFileURL(entry).href)).default;
  await mermaid.parse(source);
  writeFileSync(resultPath, JSON.stringify({ ok: true }));
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  fail(message);
}
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateArgs {
    source: String,
}

#[derive(Debug, Deserialize)]
struct ParseResult {
    ok: bool,
    #[serde(default)]
    error: String,
}

static RESULT_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct ValidateMermaidTool;

impl Tool for ValidateMermaidTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": MAX_SOURCE_CHARS,
                        "description": "Mermaid diagram source only. No surrounding prose. A wrapping ```mermaid fence is stripped."
                    }
                },
                "required": ["source"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        outcome_for(args)
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let args_for_block = args.clone();
    let ctx_marker = ctx.call_id.clone();
    let parsed = tokio::task::spawn_blocking(move || outcome_for(&args_for_block)).await;
    match parsed {
        Ok(outcome) => outcome,
        Err(_) => error_outcome(&format!(
            "validate_mermaid: parser task failed ({ctx_marker})."
        )),
    }
}

fn outcome_for(args: &Value) -> ToolOutcome {
    let source = match diagram_from_args(args) {
        Ok(source) => source,
        Err(message) => return error_outcome(&message),
    };
    match parse_source(&source) {
        Ok(()) => ok_outcome(),
        Err(message) => error_outcome(&clip(&message)),
    }
}

fn diagram_from_args(args: &Value) -> Result<String, String> {
    let parsed = serde_json::from_value::<ValidateArgs>(args.clone())
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    let source = unwrap_fence(&parsed.source);
    if source.trim().is_empty() {
        return Err("validate_mermaid: source is empty.".into());
    }
    if source.chars().count() > MAX_SOURCE_CHARS {
        return Err(format!(
            "validate_mermaid: source exceeds {MAX_SOURCE_CHARS} chars."
        ));
    }
    Ok(source)
}

fn unwrap_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return raw.to_string();
    };
    let rest = rest.trim_start();
    let rest = rest.strip_prefix("mermaid").unwrap_or(rest);
    let rest = rest.trim_start_matches(['\r', '\n']);
    let rest = rest.strip_suffix("```").unwrap_or(rest);
    rest.trim().to_string()
}

fn parse_source(source: &str) -> Result<(), String> {
    let entry = mermaid_entry().ok_or("validate_mermaid: mermaid package was not found.")?;
    let dir = script_dir()?;
    let result_path = result_file(&dir);
    let mut child = Command::new("node")
        .arg("--import")
        .arg(dir.join("register.mjs"))
        .arg(dir.join("check.mjs"))
        .env("MERMAID_ENTRY", &entry)
        .env("RESULT_PATH", &result_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "validate_mermaid: node is not available.".to_string())?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "validate_mermaid: could not write the diagram.".to_string())?;
        stdin
            .write_all(source.as_bytes())
            .map_err(|error| format!("validate_mermaid: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    let parsed = fs::read_to_string(&result_path).ok();
    let _ = fs::remove_file(&result_path);
    if let Some(body) = parsed {
        let result: ParseResult =
            serde_json::from_str(&body).map_err(|error| format!("validate_mermaid: {error}"))?;
        if result.ok {
            return Ok(());
        }
        let message = result.error.trim();
        if message.is_empty() {
            return Err("validate_mermaid: parser rejected the diagram.".into());
        }
        return Err(message.to_string());
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            return Err("validate_mermaid: parser exited without a result.".into());
        }
        return Err(format!("validate_mermaid: {detail}"));
    }
    Err("validate_mermaid: parser exited without a result.".into())
}

fn mermaid_entry() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../node_modules/mermaid/dist/mermaid.core.mjs"),
    );
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..8 {
            let Some(current) = dir else { break };
            candidates.push(current.join("node_modules/mermaid/dist/mermaid.core.mjs"));
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn script_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("k-agent-validate-mermaid");
    fs::create_dir_all(&dir).map_err(|error| format!("validate_mermaid: {error}"))?;
    fs::write(dir.join("dompurify-shim.mjs"), SHIM)
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    fs::write(dir.join("loader.mjs"), LOADER)
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    fs::write(dir.join("register.mjs"), REGISTER)
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    fs::write(dir.join("check.mjs"), CHECK)
        .map_err(|error| format!("validate_mermaid: {error}"))?;
    Ok(dir)
}

fn result_file(dir: &Path) -> PathBuf {
    let seq = RESULT_SEQ.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    dir.join(format!("result-{millis}-{seq}.json"))
}

fn clip(message: &str) -> String {
    let mut out = String::new();
    for ch in message.chars() {
        if out.chars().count() >= MAX_ERROR_CHARS {
            break;
        }
        out.push(ch);
    }
    out
}

fn ok_outcome() -> ToolOutcome {
    let text = toon_doc(&[("status", ToonValue::Str("ok"))]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_is_an_error() {
        let outcome = outcome_for(&json!({ "source": "   " }));
        assert!(outcome.text.contains("status: error"));
        assert!(outcome.text.contains("source is empty"));
    }

    #[test]
    fn unwraps_a_mermaid_fence() {
        let source = unwrap_fence("```mermaid\nflowchart LR\n  A-->B\n```");
        assert_eq!(source, "flowchart LR\n  A-->B");
    }

    #[test]
    fn parse_accepts_a_flowchart_and_rejects_a_broken_sequence() {
        let ok = parse_source("flowchart LR\n  A[Status line] --> B[Headers]");
        assert!(ok.is_ok(), "{ok:?}");
        let err = parse_source("sequenceDiagram\n  A->>B: hi\n  1").expect_err("bad diagram");
        assert!(err.contains("Parse error"));
    }
}
