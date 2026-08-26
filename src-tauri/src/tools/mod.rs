mod edit;
mod list_directory;
mod read;
mod skill;
mod write;

use serde::{Deserialize, Serialize};
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

pub const TOOL_KIND_CONTEXT: &str = "context";
pub const TOOL_KIND_ACTION: &str = "action";

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

    pub fn relative_path(&self, path: &std::path::Path) -> String {
        crate::pathutil::relative_to_workspace(path, self.workspace_path().as_deref())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDisplay {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub before: String,
    pub after: String,
}

pub struct ToolOutcome {
    pub text: String,
    pub display: ToolDisplay,
    pub snapshot: Option<FileSnapshot>,
}

impl ToolOutcome {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            display: ToolDisplay {
                kind: TOOL_KIND_CONTEXT.to_string(),
                ..ToolDisplay::default()
            },
            snapshot: None,
        }
    }
}

pub enum YamlValue<'a> {
    Str(&'a str),
    Int(i64),
    Block(&'a str),
}

pub fn yaml_doc(fields: &[(&str, YamlValue<'_>)]) -> String {
    let mut out = String::new();
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        match value {
            YamlValue::Str(text) => {
                out.push_str(key);
                out.push_str(": ");
                out.push_str(&yaml_quote(text));
            }
            YamlValue::Int(number) => {
                out.push_str(key);
                out.push_str(": ");
                out.push_str(&number.to_string());
            }
            YamlValue::Block(text) => {
                out.push_str(key);
                out.push_str(": |\n");
                if text.is_empty() {
                    continue;
                }
                for line in text.split('\n') {
                    out.push_str("  ");
                    out.push_str(line);
                    out.push('\n');
                }
                if out.ends_with('\n') {
                    out.pop();
                }
            }
        }
    }
    out
}

fn yaml_quote(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.starts_with(' ')
        || value.ends_with(' ')
        || matches!(value, "true" | "false" | "null" | "yes" | "no")
        || value.bytes().any(|byte| {
            matches!(
                byte,
                b':' | b'#'
                    | b'\n'
                    | b'\r'
                    | b'"'
                    | b'\''
                    | b'['
                    | b']'
                    | b'{'
                    | b'}'
                    | b'&'
                    | b'*'
                    | b'!'
                    | b'|'
                    | b'>'
                    | b'%'
                    | b'@'
                    | b'`'
                    | b','
            )
        });
    if !needs_quotes {
        return value.to_string();
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}

pub fn line_add_remove(before: &str, after: &str) -> (u32, u32) {
    if before == after {
        return (0, 0);
    }
    let old_lines: Vec<&str> = before.split('\n').collect();
    let new_lines: Vec<&str> = after.split('\n').collect();
    let old_empty = before.is_empty();
    let new_empty = after.is_empty();
    if old_empty {
        return (line_count(after), 0);
    }
    if new_empty {
        return (0, line_count(before));
    }
    let mut start = 0usize;
    while start < old_lines.len() && start < new_lines.len() && old_lines[start] == new_lines[start]
    {
        start += 1;
    }
    let mut old_end = old_lines.len();
    let mut new_end = new_lines.len();
    while old_end > start && new_end > start && old_lines[old_end - 1] == new_lines[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }
    (
        (new_end.saturating_sub(start)) as u32,
        (old_end.saturating_sub(start)) as u32,
    )
}

fn line_count(text: &str) -> u32 {
    if text.is_empty() {
        0
    } else {
        text.split('\n').count() as u32
    }
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
    ToolOutcome::text(format!("Unknown tool `{name}`."))
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

pub fn context_error(path: Option<&str>, message: &str) -> ToolOutcome {
    let text = match path {
        Some(path) => yaml_doc(&[
            ("path", YamlValue::Str(path)),
            ("status", YamlValue::Str("error")),
            ("error", YamlValue::Str(message)),
        ]),
        None => yaml_doc(&[
            ("status", YamlValue::Str("error")),
            ("error", YamlValue::Str(message)),
        ]),
    };
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: path.map(str::to_string),
            status: Some("error".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
    }
}

pub fn action_error(path: &str, message: &str) -> ToolOutcome {
    ToolOutcome {
        text: yaml_doc(&[
            ("path", YamlValue::Str(path)),
            ("status", YamlValue::Str("error")),
            ("error", YamlValue::Str(message)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            path: Some(path.to_string()),
            status: Some("error".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_doc_quotes_special_paths() {
        let text = yaml_doc(&[("path", YamlValue::Str("src/foo:bar"))]);
        assert_eq!(text, "path: \"src/foo:bar\"");
    }

    #[test]
    fn yaml_block_indents_body() {
        let text = yaml_doc(&[("body", YamlValue::Block("one\ntwo"))]);
        assert_eq!(text, "body: |\n  one\n  two");
    }

    #[test]
    fn line_add_remove_counts_hunk() {
        let before = "a\nb\nc\n";
        let after = "a\nB\nc\n";
        assert_eq!(line_add_remove(before, after), (1, 1));
    }

    #[test]
    fn line_add_remove_create() {
        assert_eq!(line_add_remove("", "a\nb"), (2, 0));
    }
}
