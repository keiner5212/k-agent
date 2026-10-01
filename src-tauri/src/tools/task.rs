use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "task";

const DESCRIPTION: &str = "\
Hand a multi-step side job to another saved agent and wait for its result text. \
Do not use this for one read, one grep, or one file edit. \
agent is that agent's name. prompt is the full task, including what to return. \
description is 3 to 5 words. The child uses the parent instructions plus a subagent clause. \
The child cannot start another task. Several task calls run one at a time. \
Each returns its text before the next starts. At most 8 model rounds. \
The user can open the child chat from this call while it runs.\
";

const MAX_PROMPT_CHARS: usize = 12_000;

pub struct TaskTool;

impl Tool for TaskTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "3 to 5 words."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Full task for the other agent, including the result to return."
                    },
                    "agent": {
                        "type": "string",
                        "description": "Name of a saved agent."
                    }
                },
                "required": ["description", "prompt", "agent"]
            }),
        }
    }

    fn execute(&self, _args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        super::context_error(None, "task must run on the async path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let description = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let agent = args
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if description.is_empty() || prompt.is_empty() || agent.is_empty() {
        return super::context_error(None, "task requires description, prompt, and agent.");
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return super::context_error(None, "task prompt exceeds 12000 chars.");
    }
    let Some(scope) = ctx.nested.clone() else {
        return super::context_error(None, "task needs a chat session.");
    };
    if scope.task_depth >= 1 {
        return super::context_error(None, "task cannot start another task.");
    }
    let Some(app) = ctx.app else {
        return super::context_error(None, "task needs the desktop shell.");
    };
    match crate::chat::run_task(app, &scope, ctx, agent, description, prompt).await {
        Ok(run) => ToolOutcome {
            text: toon_doc(&[
                ("agent", ToonValue::Str(agent)),
                ("description", ToonValue::Str(description)),
                ("status", ToonValue::Str("ok")),
                ("result", ToonValue::Block(&run.text)),
            ]),
            display: ToolDisplay {
                kind: TOOL_KIND_CONTEXT.to_string(),
                status: Some("ok".into()),
                child_session_id: run.child_session_id,
                ..ToolDisplay::default()
            },
            snapshot: None,
            image_png: None,
            file: None,
        },
        Err(message) => super::context_error(None, &message),
    }
}
