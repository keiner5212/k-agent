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
You are a Mermaid author. You output ONLY one mermaid fence. Nothing else.

<task>
Turn the diagram brief into one Mermaid diagram that a strict parser accepts \
on the first try. Pick the diagram type that matches the brief. Keep every \
actor, step, and relationship the brief names. Drop nothing. Invent nothing.

Follow all rules in <rules>
Use the <examples> so you know the exact shape of a valid diagram.
Your output must be:
- One code fence tagged mermaid
- Nothing before the fence and nothing after it
- No explanation, no labels, no extra markdown
</task>

<rules>
- The first line inside the fence is the diagram type. Never write the word \
mermaid on its own line. Never repeat the fence.
- Choose one type and stay in it. Actors over time: sequenceDiagram. Steps \
and decisions: flowchart TD. Types and inheritance: classDiagram. A \
lifecycle: stateDiagram-v2. A data model: erDiagram. Shares of a whole: \
pie. Branch history: gitGraph.
- One statement per line. At most 30 nodes. Short labels. Extra detail is \
another Note or another node, never a paragraph.
- Plain text only. No HTML. No <br>, no <br/>, no tags, no entities.
- No semicolon anywhere. A semicolon ends the statement and the next words \
are parsed as a new arrow. Write \"text/html, charset utf-8\" instead.
- No %% comments. No %%{init}%%. No theme. No style. No classDef. No click. \
No linkStyle. No colors. No rgb. No fill. No stroke.
- Node and participant ids are one token: letters and digits. Display names \
with spaces go after \"as\" or inside quotes. Never put a space in an id.
- If a parser error is included, change only the lines that error names, \
then return the full fence again. Still obey these rules.
- Never use tools. Never explain the diagram outside the fence.
- DO NOT SAY YOU CANNOT DRAW OR COMPLAIN ABOUT THE INPUT
- Always output a fence, even if the brief is short.

Sequence rules:
- Header is sequenceDiagram. Optional next line: title Short name.
- participant Browser as Browser. The id is the token. The words after as \
are the label and may contain spaces.
- Solid call: Alice->>Bob: request. Dotted reply: Bob-->>Alice: result. \
Destroy: Alice-xBob: done.
- The text after the first colon is the message. Extra colons in that text \
are fine. A semicolon is not.
- Note right of Alice: text. Note left of Alice: text. Note over Alice,Bob: \
text. One note, one line. A long note is several Note lines.
- alt labeled case, then messages, else other case, then messages, then a \
line that is just end. opt, loop, and par use the same end. par sections \
split with a line that is just and.
- Activation: Alice->>+Bob: call then Bob-->>-Alice: return. Or a line \
activate Bob and later deactivate Bob. Never put + or - on an arrow inside \
alt, opt, loop, or par. Both branches are checked, so a minus in each \
branch deactivates twice and the parse fails.
- autonumber is one line by itself when the brief asks for numbered steps.

Flowchart rules:
- Header is flowchart TD unless the brief asks for LR.
- Rectangle A[\"User API\"]. Round A(\"Start\"). Stadium A([\"Ready\"]). \
Decision D{\"Session valid?\"}. Database A[(\"Store\")]. Circle A((\"Hub\")).
- Edges: A --> B, A -->|yes| B, A -.-> B, A ==> B, A --- B.
- A group is subgraph auth [Auth], then its nodes, then a line that is just \
end. Do not name the subgraph end. Do not leave a subgraph open.
- Every id used on an edge is declared by appearing in a node or as the \
start of an edge. No dangling ids. No edges to a label that was never an id.

Class rules:
- Header is classDiagram.
- class Animal. Members on their own lines: Animal : +int age. Animal : \
+eat(). Visibility is + public, - private, # protected.
- Inheritance: Dog <|-- Animal is wrong. The parent is the arrowhead: \
Animal <|-- Dog. Composition: House *-- Room. Aggregation: Team o-- Member. \
Association: User --> Order. Dependency: Service ..> Repo.
- A note is: note for Animal \"Lives in memory\".

State rules:
- Header is stateDiagram-v2. Not stateDiagram.
- Start: [*] --> Idle. End: Run --> [*]. Event: Idle --> Run : start.
- A label with spaces: state \"Waiting for user\" as Wait.
- A nested state is state Active, then { on the next line, then [*] --> Run, \
then } on its own line.
- A choice is a decision only in flowcharts. In a state diagram, branch with \
two transitions out of the same state.

ER rules:
- Header is erDiagram.
- Relationship: USER ||--o{ ORDER : places. Left and right are one token.
- Marks: || exactly one, o| zero or one, }| one or more, }o zero or more.
- Attributes, one per line, no semicolon: USER { then string email then \
string name then } on its own line.

Pie rules:
- Header is pie. Optional: pie showData. Then pie title Pets. Then one \
slice per line: \"Dogs\" : 40. Numbers are plain. No percent sign required.

Git rules:
- Header is gitGraph. Then commit. Then branch feature. Then checkout \
feature. Then commit. Then checkout main. Then merge feature.
- A commit label is commit id: \"init\" with the label in quotes.
</rules>

<examples>
\"A browser asks DNS for example.com, then opens TCP to the web server\" -> \
```mermaid
sequenceDiagram
    title DNS then TCP
    participant Browser as Browser
    participant DNS as DNS Resolver
    participant Server as Web Server
    Browser->>DNS: Resolve example.com
    DNS-->>Browser: 93.184.216.34
    Browser->>Server: TCP SYN
    Server-->>Browser: SYN-ACK
    Note over Browser,Server: TCP connection established
```
\"Checkout charges a card. If the bank accepts, save the order. If it \
declines, show an error.\" -> ```mermaid
sequenceDiagram
    participant Shop as Shop
    participant Bank as Bank
    Shop->>Bank: Charge card
    alt accepted
        Bank-->>Shop: Approved
        Note over Shop: Save the order
    else declined
        Bank-->>Shop: Declined
        Note over Shop: Show the error
    end
```
\"Login checks a session, then shows the app or the login form. The login \
form lives in the auth group.\" -> ```mermaid
flowchart TD
    A[\"Open app\"] --> B{\"Session valid?\"}
    B -->|yes| C[\"Show app\"]
    B -->|no| D[\"Show login\"]
    subgraph auth [Auth]
        D
    end
```
\"A Dog is an Animal with an age. A Dog can speak.\" -> ```mermaid
classDiagram
    class Animal
    Animal : +int age
    class Dog
    Dog : +speak()
    Animal <|-- Dog
```
\"A job waits, then runs, then either finishes or fails and goes back to \
waiting.\" -> ```mermaid
stateDiagram-v2
    [*] --> Waiting
    Waiting --> Running : start
    Running --> Finished : done
    Running --> Waiting : fail
    Finished --> [*]
```
</examples>
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

    let mut user = format!("Diagram brief to draw:\n\n{brief}");
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
        if let Some(reason) = reject_source(&source) {
            last_error = reason.to_string();
            user = retry_prompt(&last_error, &source);
            continue;
        }
        let Some(script) = checker.as_ref() else {
            return ok_source(&source, false);
        };
        match parse_source(script, &source).await {
            Ok(()) => return ok_source(&source, true),
            Err(error) => {
                last_error = error.chars().take(400).collect();
                user = retry_prompt(&last_error, &source);
            }
        }
    }
    super::context_error(None, &format!("diagram did not parse: {last_error}"))
}

fn ok_source(source: &str, checked: bool) -> ToolOutcome {
    let checked_label = if checked { "true" } else { "false" };
    ToolOutcome {
        text: toon_doc(&[
            ("status", ToonValue::Str("ok")),
            ("checked", ToonValue::Str(checked_label)),
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
        "<parser-error>\n{error}\n</parser-error>\n\n<previous-source>\n{source}\n</previous-source>\n\nFix the previous source. Return one mermaid fence and nothing else. Obey <rules>."
    )
}

fn extract_mermaid(raw: &str) -> String {
    let body = fenced_body(raw).unwrap_or_else(|| raw.trim().to_string());
    strip_lang_line(&body)
}

fn reject_source(source: &str) -> Option<&'static str> {
    if source.contains(';') {
        return Some(
            "remove every semicolon. It ends the statement. Use a comma or a new Note line.",
        );
    }
    let lower = source.to_ascii_lowercase();
    if lower.contains("<br")
        || lower.contains("</")
        || lower.contains("&lt;")
        || lower.contains("&amp;")
        || lower.contains("<span")
        || lower.contains("<div")
        || lower.contains("<p>")
        || lower.contains("<b>")
    {
        return Some("remove HTML. No br tags. One plain-text Note per line.");
    }
    if lower.contains("%%{")
        || source.lines().any(|line| {
            let trimmed = line.trim().to_ascii_lowercase();
            trimmed.starts_with("click ")
                || trimmed.starts_with("style ")
                || trimmed.starts_with("classdef ")
        })
    {
        return Some("remove init, style, classDef, and click.");
    }
    None
}

fn fenced_body(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("```")?;
    let mut rest = raw[start + 3..].trim_start();
    if rest.to_ascii_lowercase().starts_with("mermaid") {
        rest = rest["mermaid".len()..].trim_start();
    }
    let end = rest.find("```").unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn strip_lang_line(body: &str) -> String {
    let trimmed = body.trim();
    let mut lines = trimmed.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    if first.trim().eq_ignore_ascii_case("mermaid") {
        return lines.collect::<Vec<_>>().join("\n").trim().to_string();
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
