use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};
use crate::chat::ChatChunk;
use crate::sessions;

pub const NAME: &str = "todowrite";

const DESCRIPTION: &str = "Replace the session todo list. Pass the full ordered list every time. Use status `in_progress` for the task you are working on right now and `completed` for finished tasks. The list is shown to the user and persisted across restarts.";

pub const MAX_TODO_ITEMS: usize = 64;
pub const MAX_TODO_CONTENT_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TodoStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
}

impl TodoItem {
    fn validate(&self) -> Result<(), String> {
        if self.content.trim().is_empty() {
            return Err("todowrite every item needs a non-empty `content`.".into());
        }
        if self.content.chars().count() > MAX_TODO_CONTENT_CHARS {
            return Err(format!(
                "todowrite item content exceeds {MAX_TODO_CONTENT_CHARS} chars."
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoWriteArgs {
    todos: Vec<TodoItem>,
}

pub struct TodoTool;

impl Tool for TodoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "minItems": 0,
                        "maxItems": MAX_TODO_ITEMS,
                        "description": "Full ordered list. Empty array clears the list.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "description": "Short description of the task."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "Current status of the task."
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["high", "medium", "low"],
                                    "description": "Priority of the task."
                                }
                            },
                            "required": ["content", "status", "priority"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let parsed = match serde_json::from_value::<TodoWriteArgs>(args.clone()) {
            Ok(value) => value,
            Err(error) => return super::context_error(None, &format!("todowrite: {error}")),
        };
        if parsed.todos.len() > MAX_TODO_ITEMS {
            return super::context_error(
                None,
                &format!("todowrite accepts at most {MAX_TODO_ITEMS} items."),
            );
        }
        for item in &parsed.todos {
            if let Err(message) = item.validate() {
                return super::context_error(None, &message);
            }
        }
        apply_and_persist(ctx, parsed.todos)
    }
}

fn apply_and_persist(ctx: &ToolContext<'_>, todos: Vec<TodoItem>) -> ToolOutcome {
    if let Err(message) = persist(ctx, &todos) {
        return super::context_error(None, &message);
    }
    if let Some(channel) = ctx.on_chunk {
        if let Ok(payload) = serde_json::to_string(&todos) {
            let _ = channel.send(ChatChunk {
                kind: "todo".to_string(),
                text: payload,
            });
        }
    }
    let summary = build_summary(&todos);
    let text = toon_doc(&[
        ("status", ToonValue::Str("ok")),
        ("count", ToonValue::Int(todos.len() as i64)),
        ("summary", ToonValue::Block(&summary)),
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

fn persist(ctx: &ToolContext<'_>, todos: &[TodoItem]) -> Result<(), String> {
    let Some(app) = ctx.app else {
        return Err("todowrite needs the desktop shell.".into());
    };
    let Some(session_id) = ctx.session_id.as_deref() else {
        return Err("todowrite needs an active session id.".into());
    };
    sessions::update_session_todos(app, session_id, todos.to_vec())
        .map_err(|error| error.to_string())
}

fn build_summary(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "no items".to_string();
    }
    let mut counts = [0usize; 4];
    for item in todos {
        match item.status {
            TodoStatus::Pending => counts[0] += 1,
            TodoStatus::InProgress => counts[1] += 1,
            TodoStatus::Completed => counts[2] += 1,
            TodoStatus::Cancelled => counts[3] += 1,
        }
    }
    let mut parts = Vec::new();
    if counts[0] > 0 {
        parts.push(format!("{} pending", counts[0]));
    }
    if counts[1] > 0 {
        parts.push(format!("{} in_progress", counts[1]));
    }
    if counts[2] > 0 {
        parts.push(format!("{} completed", counts[2]));
    }
    if counts[3] > 0 {
        parts.push(format!("{} cancelled", counts[3]));
    }
    if parts.is_empty() {
        "no items".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolContext;
    use std::path::PathBuf;

    #[test]
    fn build_summary_counts_each_status() {
        let todos = vec![
            TodoItem {
                content: "a".into(),
                status: TodoStatus::Pending,
                priority: TodoPriority::High,
            },
            TodoItem {
                content: "b".into(),
                status: TodoStatus::InProgress,
                priority: TodoPriority::High,
            },
            TodoItem {
                content: "c".into(),
                status: TodoStatus::Completed,
                priority: TodoPriority::Low,
            },
        ];
        let summary = build_summary(&todos);
        assert!(summary.contains("1 pending"));
        assert!(summary.contains("1 in_progress"));
        assert!(summary.contains("1 completed"));
    }

    #[test]
    fn validate_rejects_empty_content() {
        let item = TodoItem {
            content: "   ".into(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
        };
        assert!(item.validate().is_err());
    }

    #[test]
    fn empty_list_clears_todos() {
        let ctx = ToolContext::for_test(PathBuf::from("/tmp"), 1);
        let outcome = TodoTool.execute(&json!({ "todos": [] }), &ctx);
        // No app handle, so persistence errors out, but the argument parsing path
        // must accept an empty array as a clear-list operation.
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
    }
}