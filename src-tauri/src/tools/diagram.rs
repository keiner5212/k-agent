use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

use super::{toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue};

pub const NAME: &str = "diagram";

const DESCRIPTION: &str = "\
Write one Mermaid diagram from a full brief. Pass the picture in words. Do not write Mermaid yourself. \
The result source is already checked when Node is installed. Paste that source in a mermaid fence and do not edit it. \
If status is error, say the diagram did not parse. Do not invent a replacement.\
";

const MAX_BRIEF_CHARS: usize = 8_000;
const MAX_ATTEMPTS: u8 = 3;
const MAX_OUTPUT: u64 = 4_096;
const PARSE_TIMEOUT: Duration = Duration::from_secs(20);

const SYSTEM: &str = "\
You write one Mermaid diagram. Output only this fence and nothing else:

```mermaid
diagram here
```

No theme. No %%{init}%%. No style. No classDef. No click. No HTML. No colors.

Use one of these first lines:
- flowchart TD
- sequenceDiagram
- classDiagram
- stateDiagram-v2
- erDiagram
- pie
- gitGraph

Flowchart rules:
- A node id is one token, letters and digits. No spaces.
- Put labels with spaces or punctuation in brackets or quotes: A[\"User API\"]
- Edges: A --> B, A -->|label| B, A -.-> B, A ==> B
- A decision is D{Ready?}
- A group is: subgraph auth [Auth] then nodes then a line that is just end
- Keep direction TD unless the brief asks for LR.

Sequence rules:
- participant Web as Browser
- Alice->>Bob: request
- Alice-->>Bob: reply
- Note over Alice,Bob: text

Class rules:
- class Animal
- Animal : +int age
- Animal <|-- Dog

State rules:
- [*] --> Idle
- Idle --> Run : start

ER rules:
- USER ||--o{ ORDER : places
- Names are one token.

Pie rules:
- pie title Pets
- \"Dogs\" : 40

Git rules:
- gitGraph
- commit
- branch feature
- checkout feature
- commit

One diagram. At most 30 nodes. If a parser error is included, fix that error and return the full fence again.\
";

pub struct DiagramTool;

impl Tool for DiagramTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "brief": {
                        "type": "string",
                        "description": "Full description of the diagram: actors, steps, and relationships. Not Mermaid source."
                    }
                },
                "required": ["brief"]
            }),
        }
    }

    fn execute(&self, _args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        super::context_error(None, "diagram must run on the async path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let brief = args
        .get("brief")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if brief.is_empty() {
        return super::context_error(None, "diagram requires a brief.");
    }
    if brief.chars().count() > MAX_BRIEF_CHARS {
        return super::context_error(None, "diagram brief exceeds 8000 chars.");
    }
    let Some(app) = ctx.app else {
        return super::context_error(None, "diagram needs the desktop shell.");
    };
    let Some((provider_id, model_id)) = generation_target(app, ctx) else {
        return super::context_error(None, "diagram needs a model.");
    };
    let limit = limit_provider_data(app);
    let checker = checker_script(app).filter(|_| node_available());

    let mut user = format!("{brief}\n\nReturn one mermaid fence and nothing else.");
    let mut last_error = String::new();
    let mut attempts = 0u8;
    while attempts < MAX_ATTEMPTS {
        attempts += 1;
        let generated = match crate::chat::complete_quiet(
            app,
            &provider_id,
            &model_id,
            SYSTEM,
            &user,
            MAX_OUTPUT,
            limit,
        )
        .await
        {
            Ok(text) => text,
            Err(error) => return super::context_error(None, &error.to_string()),
        };
        let source = extract_mermaid(&generated);
        if source.is_empty() {
            last_error = "empty diagram".to_string();
            user = retry_prompt(&last_error, &source);
            continue;
        }
        let Some(script) = checker.as_ref() else {
            return ok_source(&source, false, attempts);
        };
        match parse_source(script, &source).await {
            Ok(()) => return ok_source(&source, true, attempts),
            Err(error) => {
                last_error = error;
                user = retry_prompt(&last_error, &source);
            }
        }
    }
    super::context_error(None, &format!("diagram did not parse: {last_error}"))
}

fn ok_source(source: &str, checked: bool, attempts: u8) -> ToolOutcome {
    let checked_label = if checked { "true" } else { "false" };
    ToolOutcome {
        text: toon_doc(&[
            ("status", ToonValue::Str("ok")),
            ("checked", ToonValue::Str(checked_label)),
            ("attempts", ToonValue::Str(&attempts.to_string())),
            ("source", ToonValue::Block(source)),
        ]),
        display: ToolDisplay {
            kind: super::TOOL_KIND_CONTEXT.to_string(),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn retry_prompt(error: &str, source: &str) -> String {
    format!(
        "Parser error:\n{error}\n\nPrevious source:\n{source}\n\nFix the diagram. Return one mermaid fence and nothing else."
    )
}

fn extract_mermaid(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if let Some(mark) = lower.find("```mermaid") {
        let rest = raw[mark + "```mermaid".len()..].trim_start_matches(['\r', '\n']);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    let trimmed = raw.trim();
    if let Some(start) = trimmed.find("```") {
        let rest = trimmed[start + 3..].trim_start_matches(['\r', '\n']);
        let rest = rest
            .trim_start_matches(|ch: char| ch.is_ascii_alphabetic())
            .trim_start_matches(['\r', '\n']);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn generation_target(app: &AppHandle, ctx: &ToolContext<'_>) -> Option<(String, String)> {
    if let Some(settings) = crate::load_ui_settings(app) {
        if let Some(model) = settings.get("appGenerationModel") {
            let provider_id = model
                .get("providerId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let model_id = model.get("modelId").and_then(Value::as_str).unwrap_or("");
            if !provider_id.is_empty() && !model_id.is_empty() {
                return Some((provider_id.to_string(), model_id.to_string()));
            }
        }
    }
    ctx.nested
        .as_ref()
        .map(|scope| (scope.provider_id.clone(), scope.model_id.clone()))
}

fn limit_provider_data(app: &AppHandle) -> bool {
    crate::load_ui_settings(app)
        .and_then(|settings| settings.get("limitProviderDataUse")?.as_bool())
        .unwrap_or(false)
}

fn node_available() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn checker_script(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(dir) = app.path().resource_dir() {
        for candidate in [
            dir.join("resources/mermaid-check.cjs"),
            dir.join("mermaid-check.cjs"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/mermaid-check.cjs");
    dev.is_file().then_some(dev)
}

async fn parse_source(script: &Path, source: &str) -> Result<(), String> {
    let mut child = tokio::process::Command::new("node")
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("node failed to start: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(source.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        drop(stdin);
    }
    let output = tokio::time::timeout(PARSE_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "mermaid parse timed out".to_string())?
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(stdout.trim()).map_err(|_| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail: String = stderr.chars().take(500).collect();
        if detail.is_empty() {
            "mermaid checker failed".to_string()
        } else {
            format!("mermaid checker failed: {detail}")
        }
    })?;
    if parsed.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(());
    }
    Err(parsed
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("parse failed")
        .to_string())
}
