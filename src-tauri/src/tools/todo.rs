use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};
use crate::chat::ChatChunk;
use crate::sessions;

pub const NAME: &str = "todowrite";

const DESCRIPTION: &str = "\
Replace the session todo list. Send the full list every call. There are no ids.\n\
Use it when the task has 3 or more steps, the user listed several tasks, or a step starts or finishes.\n\
Skip it for one straightforward step or a purely informational reply.\n\
Each item is {content, status, priority}. status is pending, in_progress, completed, or cancelled. priority is high, medium, or low.\n\
Keep at most one item in_progress. Mark completed only after that step is done. An empty todos array clears the list.\n\
A bad field returns an error that names the item index and the allowed values.";

const SHAPE: &str = "todowrite takes {\"todos\":[{\"content\":\"Run tests\",\"status\":\"in_progress\",\"priority\":\"high\"}]}. Do not send id, add, update, remove, or clear. Send the full list every time. status is pending, in_progress, completed, or cancelled. priority is high, medium, or low. An empty todos array clears the list.";

pub const MAX_TODO_ITEMS: usize = 64;
pub const MAX_TODO_CONTENT_CHARS: usize = 200;
pub const HISTORY_CAP: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoDiff {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<TodoItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<TodoItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cleared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoHistoryEvent {
    pub timestamp: i64,
    #[serde(default)]
    pub diff: TodoDiff,
}

#[derive(Debug)]
struct Incoming {
    content: String,
    status: TodoStatus,
    priority: u8,
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
                        "description": "The full ordered list. Send it again on every update. An empty array clears the list. Do not include ids.",
                        "maxItems": MAX_TODO_ITEMS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": MAX_TODO_CONTENT_CHARS,
                                    "description": "Short description of the task."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "pending, in_progress, completed, or cancelled. At most one in_progress."
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["high", "medium", "low"],
                                    "description": "high, medium, or low."
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
        let parsed = match parse_todos(args) {
            Ok(value) => value,
            Err(message) => return super::context_error(None, &message),
        };
        match apply(ctx, parsed) {
            Ok(outcome) => outcome,
            Err(message) => super::context_error(None, &message),
        }
    }
}

fn apply(ctx: &ToolContext<'_>, incoming: Vec<Incoming>) -> Result<ToolOutcome, String> {
    let current = read_current(ctx)?;
    let staged = replace_list(incoming, &current)?;
    let changed = staged.final_todos != current;
    if changed {
        persist_with_event(ctx, staged.final_todos.clone(), staged.diff.clone())?;
    }
    Ok(emit_outcome(ctx, staged.final_todos, &staged.diff, changed))
}

#[derive(Debug)]
struct Staged {
    final_todos: Vec<TodoItem>,
    diff: TodoDiff,
}

fn parse_todos(args: &Value) -> Result<Vec<Incoming>, String> {
    let Some(obj) = args.as_object() else {
        return Err(SHAPE.to_string());
    };
    for key in ["add", "update", "remove", "clear", "id"] {
        if obj.contains_key(key) {
            return Err(SHAPE.to_string());
        }
    }
    let Some(list) = obj.get("todos") else {
        return Err(format!("todowrite: missing `todos`. {SHAPE}"));
    };
    let Some(items) = list.as_array() else {
        return Err(format!(
            "todowrite: `todos` must be an array, got {}. {SHAPE}",
            json_kind(list)
        ));
    };
    if items.len() > MAX_TODO_ITEMS {
        return Err(format!(
            "todowrite accepts at most {MAX_TODO_ITEMS} items, got {}.",
            items.len()
        ));
    }
    let mut incoming = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        incoming.push(parse_item(index, item)?);
    }
    let active: Vec<&str> = incoming
        .iter()
        .filter(|item| item.status == TodoStatus::InProgress)
        .map(|item| item.content.as_str())
        .collect();
    if active.len() > 1 {
        return Err(format!(
            "todowrite: only one item can be in_progress. These are in_progress: {}. Leave the others pending.",
            active
                .iter()
                .map(|content| format!("\"{content}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(incoming)
}

fn parse_item(index: usize, value: &Value) -> Result<Incoming, String> {
    let Some(obj) = value.as_object() else {
        return Err(format!(
            "todowrite: todos[{index}] must be an object with content, status, and priority. Got {}.",
            json_kind(value)
        ));
    };
    if obj.contains_key("id") {
        return Err(format!(
            "todowrite: todos[{index}] includes `id`. This tool has no ids. Send content, status, and priority only."
        ));
    }
    let content = match obj.get("content") {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(other) => {
            return Err(format!(
                "todowrite: todos[{index}].content must be a string, got {}.",
                json_kind(other)
            ));
        }
        None => {
            return Err(format!(
                "todowrite: todos[{index}] is missing `content`. Each item needs content, status, and priority."
            ));
        }
    };
    if content.is_empty() {
        return Err(format!(
            "todowrite: todos[{index}].content is empty. Write a short task description."
        ));
    }
    if content.chars().count() > MAX_TODO_CONTENT_CHARS {
        return Err(format!(
            "todowrite: todos[{index}].content exceeds {MAX_TODO_CONTENT_CHARS} chars."
        ));
    }
    let status = match obj.get("status") {
        Some(Value::String(text)) => parse_status(index, text)?,
        Some(other) => {
            return Err(format!(
                "todowrite: todos[{index}].status must be pending, in_progress, completed, or cancelled. Got {}.",
                json_kind(other)
            ));
        }
        None => {
            return Err(format!(
                "todowrite: todos[{index}] is missing `status`. Use pending, in_progress, completed, or cancelled."
            ));
        }
    };
    let priority = match obj.get("priority") {
        Some(Value::String(text)) => parse_priority(index, text)?,
        Some(other) => {
            return Err(format!(
                "todowrite: todos[{index}].priority must be \"high\", \"medium\", or \"low\". Got {}.",
                json_kind(other)
            ));
        }
        None => {
            return Err(format!(
                "todowrite: todos[{index}] is missing `priority`. Use \"high\", \"medium\", or \"low\"."
            ));
        }
    };
    Ok(Incoming {
        content,
        status,
        priority,
    })
}

fn parse_status(index: usize, raw: &str) -> Result<TodoStatus, String> {
    match raw.trim() {
        "pending" => Ok(TodoStatus::Pending),
        "in_progress" => Ok(TodoStatus::InProgress),
        "completed" => Ok(TodoStatus::Completed),
        "cancelled" => Ok(TodoStatus::Cancelled),
        other => Err(format!(
            "todowrite: todos[{index}].status must be pending, in_progress, completed, or cancelled. Got \"{other}\"."
        )),
    }
}

fn parse_priority(index: usize, raw: &str) -> Result<u8, String> {
    match raw.trim() {
        "high" => Ok(8),
        "medium" => Ok(5),
        "low" => Ok(2),
        other => Err(format!(
            "todowrite: todos[{index}].priority must be \"high\", \"medium\", or \"low\". Got \"{other}\"."
        )),
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn replace_list(incoming: Vec<Incoming>, current: &[TodoItem]) -> Result<Staged, String> {
    let next = assign_ids(incoming, current);
    Ok(Staged {
        final_todos: next.clone(),
        diff: diff_lists(current, &next),
    })
}

fn assign_ids(incoming: Vec<Incoming>, current: &[TodoItem]) -> Vec<TodoItem> {
    let mut used_current = vec![false; current.len()];
    let mut reserved: HashSet<String> = current.iter().map(|item| item.id.clone()).collect();
    let mut out = Vec::with_capacity(incoming.len());
    for item in incoming {
        let id =
            if let Some(index) = current.iter().enumerate().position(|(index, existing)| {
                !used_current[index] && existing.content == item.content
            }) {
                used_current[index] = true;
                current[index].id.clone()
            } else {
                let id = fresh_id(&reserved);
                reserved.insert(id.clone());
                id
            };
        out.push(TodoItem {
            id,
            content: item.content,
            status: item.status,
            priority: item.priority,
        });
    }
    out
}

fn fresh_id(used: &HashSet<String>) -> String {
    let mut n = 1u32;
    loop {
        let id = format!("t{n}");
        if !used.contains(&id) {
            return id;
        }
        n += 1;
    }
}

fn diff_lists(previous: &[TodoItem], next: &[TodoItem]) -> TodoDiff {
    let mut diff = TodoDiff::default();
    if previous.is_empty() && next.is_empty() {
        return diff;
    }
    if !previous.is_empty() && next.is_empty() {
        diff.cleared = true;
        diff.removed = previous.iter().map(|item| item.id.clone()).collect();
        return diff;
    }
    let next_ids: HashSet<&str> = next.iter().map(|item| item.id.as_str()).collect();
    for item in previous {
        if !next_ids.contains(item.id.as_str()) {
            diff.removed.push(item.id.clone());
        }
    }
    for item in next {
        match previous.iter().find(|old| old.id == item.id) {
            None => diff.added.push(item.clone()),
            Some(old) if old != item => diff.updated.push(item.clone()),
            Some(_) => {}
        }
    }
    diff
}

fn status_name(status: TodoStatus) -> &'static str {
    match status {
        TodoStatus::Pending => "pending",
        TodoStatus::InProgress => "in_progress",
        TodoStatus::Completed => "completed",
        TodoStatus::Cancelled => "cancelled",
    }
}

fn priority_word(priority: u8) -> &'static str {
    if priority >= 8 {
        "high"
    } else if priority >= 4 {
        "medium"
    } else {
        "low"
    }
}

fn emit_outcome(
    ctx: &ToolContext<'_>,
    todos: Vec<TodoItem>,
    diff: &TodoDiff,
    changed: bool,
) -> ToolOutcome {
    let summary = build_summary(&todos);
    let list = list_block(&todos);
    let status_label = if changed { "ok" } else { "noop" };
    let text = toon_doc(&[
        ("status", ToonValue::Str(status_label)),
        ("count", ToonValue::Int(todos.len() as i64)),
        ("summary", ToonValue::Str(&summary)),
        ("todos", ToonValue::Block(&list)),
    ]);
    if changed {
        if let Some(channel) = ctx.on_chunk {
            if let Ok(payload) = serde_json::to_string(&serde_json::json!({
                "todos": &todos,
                "diff": diff,
            })) {
                let _ = channel.send(ChatChunk {
                    kind: "todo".to_string(),
                    text: payload,
                });
            }
        }
    }
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some(status_label.into()),
            todos: Some(todos),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn list_block(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "(empty)".to_string();
    }
    todos
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let content = item.content.replace('\n', " ");
            format!(
                "{}. [{}] [{}] {content}",
                index + 1,
                status_name(item.status),
                priority_word(item.priority)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_current(ctx: &ToolContext<'_>) -> Result<Vec<TodoItem>, String> {
    let Some(app) = ctx.app else {
        return Err("todowrite needs the desktop shell.".into());
    };
    let Some(session_id) = ctx.session_id.as_deref() else {
        return Err("todowrite needs an active session id.".into());
    };
    sessions::read_session_todos(app, session_id).map_err(|error| error.to_string())
}

fn persist_with_event(
    ctx: &ToolContext<'_>,
    todos: Vec<TodoItem>,
    diff: TodoDiff,
) -> Result<(), String> {
    let Some(app) = ctx.app else {
        return Err("todowrite needs the desktop shell.".into());
    };
    let Some(session_id) = ctx.session_id.as_deref() else {
        return Err("todowrite needs an active session id.".into());
    };
    let event = TodoHistoryEvent {
        timestamp: chrono::Utc::now().timestamp(),
        diff,
    };
    sessions::append_session_todo_event(app, session_id, todos, event)
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

    fn item(id: &str, content: &str, status: TodoStatus, priority: u8) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            content: content.to_string(),
            status,
            priority,
        }
    }

    fn todos(raw: Value) -> Vec<Incoming> {
        parse_todos(&raw).expect("parse")
    }

    #[test]
    fn build_summary_counts_each_status() {
        let todos = vec![
            item("a", "a", TodoStatus::Pending, 2),
            item("b", "b", TodoStatus::InProgress, 8),
            item("c", "c", TodoStatus::Completed, 5),
        ];
        let summary = build_summary(&todos);
        assert!(summary.contains("1 pending"));
        assert!(summary.contains("1 in_progress"));
        assert!(summary.contains("1 completed"));
    }

    #[test]
    fn replace_keeps_id_when_content_matches() {
        let current = vec![item("step-1", "Run tests", TodoStatus::Pending, 5)];
        let staged = replace_list(
            todos(json!({
                "todos": [{
                    "content": "Run tests",
                    "status": "in_progress",
                    "priority": "high"
                }]
            })),
            &current,
        )
        .expect("replace");
        assert_eq!(staged.final_todos[0].id, "step-1");
        assert_eq!(staged.final_todos[0].status, TodoStatus::InProgress);
        assert_eq!(staged.final_todos[0].priority, 8);
        assert_eq!(staged.diff.updated.len(), 1);
    }

    #[test]
    fn replace_assigns_id_for_a_new_item() {
        let staged = replace_list(
            todos(json!({
                "todos": [{
                    "content": "Write docs",
                    "status": "pending",
                    "priority": "low"
                }]
            })),
            &[],
        )
        .expect("replace");
        assert_eq!(staged.final_todos[0].id, "t1");
        assert_eq!(staged.final_todos[0].priority, 2);
        assert_eq!(staged.diff.added.len(), 1);
    }

    #[test]
    fn empty_list_clears() {
        let current = vec![item("t1", "Run tests", TodoStatus::Pending, 5)];
        let staged = replace_list(todos(json!({ "todos": [] })), &current).expect("replace");
        assert!(staged.final_todos.is_empty());
        assert!(staged.diff.cleared);
    }

    #[test]
    fn missing_todos_names_the_shape() {
        let error = parse_todos(&json!({})).expect_err("missing");
        assert!(error.contains("missing `todos`"));
        assert!(error.contains("priority"));
    }

    #[test]
    fn legacy_id_shape_is_rejected() {
        let error = parse_todos(&json!({
            "add": [{ "id": "step-1", "content": "Run tests" }]
        }))
        .expect_err("legacy");
        assert!(error.contains("Do not send id"));
    }

    #[test]
    fn bad_status_names_the_index_and_allowed_values() {
        let error = parse_todos(&json!({
            "todos": [{
                "content": "Run tests",
                "status": "doing",
                "priority": "high"
            }]
        }))
        .expect_err("status");
        assert!(error.contains("todos[0].status"));
        assert!(error.contains("in_progress"));
        assert!(error.contains("doing"));
    }

    #[test]
    fn numeric_priority_is_rejected() {
        let error = parse_todos(&json!({
            "todos": [{
                "content": "Run tests",
                "status": "pending",
                "priority": 9
            }]
        }))
        .expect_err("priority");
        assert!(error.contains("todos[0].priority"));
        assert!(error.contains("high"));
    }

    #[test]
    fn two_in_progress_items_are_rejected() {
        let error = parse_todos(&json!({
            "todos": [
                { "content": "A", "status": "in_progress", "priority": "high" },
                { "content": "B", "status": "in_progress", "priority": "low" }
            ]
        }))
        .expect_err("active");
        assert!(error.contains("only one item can be in_progress"));
        assert!(error.contains("\"A\""));
        assert!(error.contains("\"B\""));
    }
}
