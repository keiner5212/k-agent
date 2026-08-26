mod edit;
mod list_directory;
mod read;
mod skill;
mod write;

use serde_json::{json, Map, Value};
use tauri::AppHandle;
use uuid::Uuid;

pub const MAX_TOOL_OUTPUT_CHARS: usize = 50 * 1024;
pub const MAX_TOOL_OUTPUT_LINES: usize = 2_000;
pub const MAX_ASSISTANT_CHARS: usize = 200_000;

pub const SKILL_TOOL_NAME: &str = skill::NAME;
pub const READ_TOOL_NAME: &str = read::NAME;
pub const WRITE_TOOL_NAME: &str = write::NAME;
pub const EDIT_TOOL_NAME: &str = edit::NAME;
pub const LIST_DIRECTORY_TOOL_NAME: &str = list_directory::NAME;

pub fn new_tool_call_id() -> String {
    format!("call_{}", Uuid::new_v4().simple())
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub thought_signature: String,
}

pub struct ToolContext<'a> {
    pub app: &'a AppHandle,
}

impl ToolContext<'_> {
    pub fn workspace_path(&self) -> Option<std::path::PathBuf> {
        crate::pathutil::workspace_from_app(self.app)
    }
}

pub struct ToolOutcome {
    pub text: String,
}

trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome;
}

fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(skill::SkillTool),
        Box::new(read::ReadTool),
        Box::new(write::WriteTool),
        Box::new(edit::EditTool),
        Box::new(list_directory::ListDirectoryTool),
    ]
}

pub fn specs() -> Vec<ToolSpec> {
    all_tools().iter().map(|tool| tool.spec()).collect()
}

pub fn execute(name: &str, arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    for tool in all_tools() {
        if tool.spec().name == name {
            return tool.execute(&args, ctx);
        }
    }
    ToolOutcome {
        text: format!("Unknown tool `{name}`."),
    }
}

fn specs_for(names: &[String]) -> Vec<ToolSpec> {
    specs()
        .into_iter()
        .filter(|spec| names.iter().any(|name| name == spec.name))
        .collect()
}

pub fn openai_tools_for(names: &[String]) -> Value {
    json!(specs_for(names)
        .into_iter()
        .map(|spec| {
            json!({
                "type": "function",
                "function": {
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                    "strict": false,
                }
            })
        })
        .collect::<Vec<_>>())
}

pub fn anthropic_tools_for(names: &[String]) -> Value {
    json!(specs_for(names)
        .into_iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "input_schema": spec.parameters,
            })
        })
        .collect::<Vec<_>>())
}

pub fn gemini_tools_for(names: &[String]) -> Value {
    json!([{
        "functionDeclarations": specs_for(names)
            .into_iter()
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": sanitize_gemini_schema(&spec.parameters),
                })
            })
            .collect::<Vec<_>>(),
    }])
}

pub fn truncate_output(text: &str) -> String {
    truncate_text(text, MAX_TOOL_OUTPUT_LINES, MAX_TOOL_OUTPUT_CHARS)
}

pub fn truncate_assistant(text: &str) -> String {
    truncate_text(text, usize::MAX, MAX_ASSISTANT_CHARS)
}

fn truncate_text(text: &str, max_lines: usize, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    let total_lines = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };
    let mut chars = 0usize;
    let mut lines = 0usize;
    let mut end = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            lines += 1;
            if lines >= max_lines {
                end = idx;
                break;
            }
        }
        chars += 1;
        if chars >= max_chars {
            end = idx + ch.len_utf8();
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end >= text.len() && lines < max_lines && total_chars <= max_chars {
        return text.to_string();
    }
    let shown = text[..end].trim_end();
    let shown_chars = shown.chars().count();
    let shown_lines = if shown.is_empty() {
        0
    } else {
        shown.lines().count()
    };
    format!(
        "{shown}\n\n[truncated: showing {shown_chars}/{total_chars} chars, {shown_lines}/{total_lines} lines. Use read with offset/limit for the rest.]"
    )
}

pub fn sanitize_gemini_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, child) in map {
                match key.as_str() {
                    "type" | "description" | "format" | "enum" | "required" | "nullable"
                    | "minLength" | "maxLength" | "minimum" | "maximum" => {
                        out.insert(key.clone(), child.clone());
                    }
                    "properties" => {
                        if let Value::Object(props) = child {
                            let mut cleaned = Map::new();
                            for (name, schema) in props {
                                cleaned.insert(name.clone(), sanitize_gemini_schema(schema));
                            }
                            out.insert(key.clone(), Value::Object(cleaned));
                        }
                    }
                    "items" => {
                        out.insert(key.clone(), sanitize_gemini_schema(child));
                    }
                    "anyOf" | "oneOf" | "allOf" => {
                        if let Value::Array(items) = child {
                            out.insert(
                                key.clone(),
                                Value::Array(items.iter().map(sanitize_gemini_schema).collect()),
                            );
                        }
                    }
                    _ => {}
                }
            }
            if !out.contains_key("type") {
                out.insert("type".into(), json!("object"));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_gemini_schema).collect()),
        other => other.clone(),
    }
}
