mod skill;

use serde_json::{json, Value};
use tauri::AppHandle;

pub const SKILL_TOOL_NAME: &str = skill::NAME;

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
}

pub struct ToolContext<'a> {
    pub app: &'a AppHandle,
}

pub struct ToolOutcome {
    pub text: String,
}

trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome;
}

fn all_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(skill::SkillTool)]
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
                    "parameters": spec.parameters,
                })
            })
            .collect::<Vec<_>>(),
    }])
}
