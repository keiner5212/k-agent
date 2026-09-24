use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};
use crate::chat::ChatChunk;
use crate::sessions;

pub const NAME: &str = "todowrite";

const DESCRIPTION: &str = "Update the session todo list incrementally. Each item has a stable `id` so it can be added, updated, or removed across calls. Use `clear: true` to wipe the list (the empty list is NOT a clear). The list is shown to the user and persisted across restarts.";

pub const MAX_TODO_ITEMS: usize = 64;
pub const MAX_TODO_CONTENT_CHARS: usize = 200;
pub const MAX_TODO_ID_CHARS: usize = 64;
pub const PRIORITY_MIN: u8 = 0;
pub const PRIORITY_MAX: u8 = 10;
pub const PRIORITY_DEFAULT: u8 = 5;
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

impl TodoItem {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("todowrite every item needs a non-empty `id`.".into());
        }
        if self.id.chars().count() > MAX_TODO_ID_CHARS {
            return Err(format!(
                "todowrite item id exceeds {MAX_TODO_ID_CHARS} chars."
            ));
        }
        if self.content.trim().is_empty() {
            return Err("todowrite every item needs a non-empty `content`.".into());
        }
        if self.content.chars().count() > MAX_TODO_CONTENT_CHARS {
            return Err(format!(
                "todowrite item content exceeds {MAX_TODO_CONTENT_CHARS} chars."
            ));
        }
        if self.priority > PRIORITY_MAX {
            return Err(format!(
                "todowrite priority must be 0-{PRIORITY_MAX}, got {}.",
                self.priority
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoAdd {
    id: String,
    content: String,
    #[serde(default)]
    status: Option<TodoStatus>,
    #[serde(default)]
    priority: Option<u8>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TodoUpdate {
    id: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    status: Option<TodoStatus>,
    #[serde(default)]
    priority: Option<u8>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TodoWriteArgs {
    #[serde(default)]
    add: Vec<TodoAdd>,
    #[serde(default)]
    update: Vec<TodoUpdate>,
    #[serde(default)]
    remove: Vec<String>,
    #[serde(default)]
    clear: bool,
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

impl TodoDiff {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty() && !self.cleared
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoHistoryEvent {
    pub timestamp: i64,
    #[serde(default)]
    pub diff: TodoDiff,
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
                    "add": {
                        "type": "array",
                        "description": "Items to add. Each must have a unique stable id.",
                        "maxItems": MAX_TODO_ITEMS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": MAX_TODO_ID_CHARS,
                                    "description": "Stable id. Use a short slug like 'tsk-1' or a content hash. Cannot change later."
                                },
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": MAX_TODO_CONTENT_CHARS,
                                    "description": "Short description of the task."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"],
                                    "description": "Initial status. Defaults to pending."
                                },
                                "priority": {
                                    "type": "integer",
                                    "minimum": PRIORITY_MIN,
                                    "maximum": PRIORITY_MAX,
                                    "description": "Initial priority 0-10. Higher = more important. Defaults to 5."
                                }
                            },
                            "required": ["id", "content"]
                        }
                    },
                    "update": {
                        "type": "array",
                        "description": "Items to update by id. Only the fields you include change; the rest stay as they were.",
                        "maxItems": MAX_TODO_ITEMS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "description": "Id of an existing item to update."
                                },
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": MAX_TODO_CONTENT_CHARS
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed", "cancelled"]
                                },
                                "priority": {
                                    "type": "integer",
                                    "minimum": PRIORITY_MIN,
                                    "maximum": PRIORITY_MAX
                                }
                            },
                            "required": ["id"]
                        }
                    },
                    "remove": {
                        "type": "array",
                        "description": "Ids of items to remove.",
                        "items": { "type": "string" }
                    },
                    "clear": {
                        "type": "boolean",
                        "description": "Set to true to wipe the list. The empty list (no add/update/remove/clear) is a no-op, NOT a clear."
                    }
                },
                "anyOf": [
                    { "required": ["add"] },
                    { "required": ["update"] },
                    { "required": ["remove"] },
                    { "required": ["clear"] }
                ]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let parsed = match serde_json::from_value::<TodoWriteArgs>(args.clone()) {
            Ok(value) => value,
            Err(error) => return super::context_error(None, &format!("todowrite: {error}")),
        };
        let result = apply(ctx, parsed);
        match result {
            Ok(outcome) => outcome,
            Err(message) => super::context_error(None, &message),
        }
    }
}

fn apply(ctx: &ToolContext<'_>, args: TodoWriteArgs) -> Result<ToolOutcome, String> {
    let current = read_current(ctx)?;
    let staged = stage(args, current)?;
    if !staged.diff.is_empty() {
        persist_with_event(ctx, staged.final_todos.clone(), staged.diff.clone())?;
    }
    Ok(emit_outcome(ctx, staged.final_todos, staged.diff))
}

#[derive(Debug)]
struct Staged {
    final_todos: Vec<TodoItem>,
    diff: TodoDiff,
}

fn id_list(items: &[TodoItem]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn stage(args: TodoWriteArgs, current: Vec<TodoItem>) -> Result<Staged, String> {
    let remove_ids = sanitize_remove_ids(args.remove);
    let mut next = current;
    let mut diff = TodoDiff::default();

    if args.clear && !next.is_empty() {
        diff.removed = next.iter().map(|item| item.id.clone()).collect();
        diff.cleared = true;
        next.clear();
    }

    let removed_now: Vec<String> = remove_ids
        .iter()
        .filter(|id| next.iter().any(|item| &item.id == *id))
        .cloned()
        .collect();
    if !removed_now.is_empty() {
        next.retain(|item| !remove_ids.iter().any(|id| id == &item.id));
        diff.removed.extend(removed_now);
        diff.removed.sort();
        diff.removed.dedup();
    }

    let updates = plan_updates(args.update, &next)?;
    let added = build_additions(args.add, &next)?;

    for update in &updates {
        let Some(target) = next.iter().find(|item| item.id == update.id) else {
            return Err(format!(
                "todowrite: cannot update unknown id `{}`. Known ids: {}.",
                update.id,
                id_list(&next)
            ));
        };
        diff.updated.push(apply_update_to_snapshot(target, update));
    }
    for update in updates {
        let Some(target) = next.iter_mut().find(|item| item.id == update.id) else {
            return Err(format!(
                "todowrite: cannot update unknown id `{}`. Known ids: {}.",
                update.id,
                id_list(&next)
            ));
        };
        if let Some(value) = update.content {
            target.content = value;
        }
        if let Some(status) = update.status {
            target.status = status;
        }
        if let Some(priority) = update.priority {
            target.priority = priority;
        }
    }
    for item in &added {
        diff.added.push(item.clone());
    }
    next.extend(added);

    if next.len() > MAX_TODO_ITEMS {
        return Err(format!(
            "todowrite accepts at most {MAX_TODO_ITEMS} items, got {}.",
            next.len()
        ));
    }
    Ok(Staged {
        final_todos: next,
        diff,
    })
}

fn build_additions(raw: Vec<TodoAdd>, current: &[TodoItem]) -> Result<Vec<TodoItem>, String> {
    let mut built: Vec<TodoItem> = Vec::with_capacity(raw.len());
    for entry in raw {
        let item = TodoItem {
            id: entry.id.trim().to_string(),
            content: entry.content.trim().to_string(),
            status: entry.status.unwrap_or(TodoStatus::Pending),
            priority: entry.priority.unwrap_or(PRIORITY_DEFAULT),
        };
        item.validate()?;
        if built.iter().any(|other| other.id == item.id) {
            return Err(format!("todowrite: duplicate id `{}` in `add`.", item.id));
        }
        if current.iter().any(|existing| existing.id == item.id) {
            return Err(format!(
                "todowrite: id `{}` already exists. Use `update` instead of `add`. Known ids: {}.",
                item.id,
                id_list(current)
            ));
        }
        built.push(item);
    }
    Ok(built)
}

#[derive(Debug)]
struct ResolvedUpdate {
    id: String,
    content: Option<String>,
    status: Option<TodoStatus>,
    priority: Option<u8>,
}

fn plan_updates(raw: Vec<TodoUpdate>, current: &[TodoItem]) -> Result<Vec<ResolvedUpdate>, String> {
    let mut resolved = Vec::with_capacity(raw.len());
    for entry in raw {
        let id = entry.id.trim().to_string();
        if id.is_empty() {
            return Err("todowrite update entries need a non-empty `id`.".into());
        }
        if !current.iter().any(|item| item.id == id) {
            return Err(format!(
                "todowrite: cannot update unknown id `{id}`. Known ids: {}.",
                id_list(current)
            ));
        }
        let content = entry.content.map(|value| value.trim().to_string());
        if let Some(value) = &content {
            if value.is_empty() {
                return Err("todowrite update content cannot be empty.".into());
            }
            if value.chars().count() > MAX_TODO_CONTENT_CHARS {
                return Err(format!(
                    "todowrite update content exceeds {MAX_TODO_CONTENT_CHARS} chars."
                ));
            }
        }
        if let Some(priority) = entry.priority {
            if priority > PRIORITY_MAX {
                return Err(format!(
                    "todowrite update priority must be 0-{PRIORITY_MAX}."
                ));
            }
        }
        resolved.push(ResolvedUpdate {
            id,
            content,
            status: entry.status,
            priority: entry.priority,
        });
    }
    Ok(resolved)
}

fn apply_update_to_snapshot(existing: &TodoItem, update: &ResolvedUpdate) -> TodoItem {
    let mut snapshot = existing.clone();
    if let Some(value) = &update.content {
        snapshot.content = value.clone();
    }
    if let Some(status) = update.status {
        snapshot.status = status;
    }
    if let Some(priority) = update.priority {
        snapshot.priority = priority;
    }
    snapshot
}

fn sanitize_remove_ids(raw: Vec<String>) -> Vec<String> {
    let mut ids: Vec<String> = raw
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn emit_outcome(ctx: &ToolContext<'_>, todos: Vec<TodoItem>, diff: TodoDiff) -> ToolOutcome {
    let summary = build_summary(&todos);
    let status_label = if diff.is_empty() { "noop" } else { "ok" };
    let text = toon_doc(&[
        ("status", ToonValue::Str(status_label)),
        ("count", ToonValue::Int(todos.len() as i64)),
        ("summary", ToonValue::Block(&summary)),
    ]);
    if !diff.is_empty() {
        if let Some(channel) = ctx.on_chunk {
            if let Ok(payload) = serde_json::to_string(&serde_json::json!({
                "todos": todos,
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
    use crate::tools::ToolContext;
    use std::path::PathBuf;

    fn ctx() -> ToolContext<'static> {
        ToolContext::for_test(PathBuf::from("/tmp"), 1)
    }

    fn item(id: &str, content: &str, status: TodoStatus, priority: u8) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            content: content.to_string(),
            status,
            priority,
        }
    }

    fn todo_write_args(
        add: &[(&str, &str, u8)],
        update: &[(&str, Option<&str>, Option<TodoStatus>, Option<u8>)],
        remove: &[&str],
        clear: bool,
    ) -> TodoWriteArgs {
        TodoWriteArgs {
            add: add
                .iter()
                .map(|(id, content, priority)| TodoAdd {
                    id: (*id).to_string(),
                    content: (*content).to_string(),
                    status: Some(TodoStatus::Pending),
                    priority: Some(*priority),
                })
                .collect(),
            update: update
                .iter()
                .map(|(id, content, status, priority)| TodoUpdate {
                    id: (*id).to_string(),
                    content: content.map(|v| v.to_string()),
                    status: *status,
                    priority: *priority,
                })
                .collect(),
            remove: remove.iter().map(|s| (*s).to_string()).collect(),
            clear,
        }
    }

    #[test]
    fn build_summary_counts_each_status() {
        let todos = vec![
            item("a", "a", TodoStatus::Pending, 9),
            item("b", "b", TodoStatus::InProgress, 7),
            item("c", "c", TodoStatus::Completed, 1),
        ];
        let summary = build_summary(&todos);
        assert!(summary.contains("1 pending"));
        assert!(summary.contains("1 in_progress"));
        assert!(summary.contains("1 completed"));
    }

    #[test]
    fn validate_rejects_empty_content() {
        assert!(item("x", "   ", TodoStatus::Pending, 5).validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_id() {
        assert!(item("  ", "ok", TodoStatus::Pending, 5).validate().is_err());
    }

    #[test]
    fn validate_rejects_priority_above_max() {
        assert!(item("x", "ok", TodoStatus::Pending, 11).validate().is_err());
    }

    #[test]
    fn validate_accepts_priority_zero() {
        assert!(item("x", "ok", TodoStatus::Pending, 0).validate().is_ok());
    }

    #[test]
    fn empty_args_is_a_noop_not_clear() {
        let args: TodoWriteArgs = serde_json::from_value(json!({})).expect("parse");
        assert!(!args.clear);
        assert!(args.add.is_empty());
        assert!(args.update.is_empty());
        assert!(args.remove.is_empty());
    }

    #[test]
    fn diff_is_empty_when_no_operations() {
        let diff = TodoDiff::default();
        assert!(diff.is_empty());
    }

    #[test]
    fn stage_adds_new_items_atomically() {
        let current = vec![item("existing", "old", TodoStatus::Pending, 5)];
        let args = todo_write_args(&[("new-1", "one", 5), ("new-2", "two", 5)], &[], &[], false);
        let staged = stage(args, current).expect("stage");
        assert_eq!(staged.final_todos.len(), 3);
        assert_eq!(staged.diff.added.len(), 2);
        assert!(!staged.diff.cleared);
        assert!(!staged.diff.is_empty());
    }

    #[test]
    fn stage_rolls_back_when_add_duplicate_id() {
        let current = vec![item("dup", "old", TodoStatus::Pending, 5)];
        let args = todo_write_args(&[("dup", "new", 5)], &[], &[], false);
        let err = stage(args, current).expect_err("should reject duplicate");
        assert!(err.contains("already exists"));
    }

    #[test]
    fn stage_rolls_back_when_add_duplicate_within_same_call() {
        let current: Vec<TodoItem> = vec![];
        let args = todo_write_args(&[("dup", "one", 5), ("dup", "two", 5)], &[], &[], false);
        let err = stage(args, current).expect_err("should reject duplicate");
        assert!(err.contains("duplicate id"));
    }

    #[test]
    fn stage_rolls_back_when_update_unknown_id() {
        let current = vec![item("real", "x", TodoStatus::Pending, 5)];
        let args = todo_write_args(
            &[],
            &[("ghost", None, Some(TodoStatus::Completed), None)],
            &[],
            false,
        );
        let err = stage(args, current).expect_err("should reject unknown id");
        assert!(err.contains("cannot update unknown id"));
    }

    #[test]
    fn stage_rolls_back_when_update_priority_out_of_range() {
        let current = vec![item("real", "x", TodoStatus::Pending, 5)];
        let args = TodoWriteArgs {
            update: vec![TodoUpdate {
                id: "real".into(),
                content: None,
                status: None,
                priority: Some(99),
            }],
            ..TodoWriteArgs::default()
        };
        assert!(stage(args, current).is_err());
    }

    #[test]
    fn stage_clear_marks_diff_cleared_only_when_items_existed() {
        let staged = stage(todo_write_args(&[], &[], &[], true), vec![]).expect("stage");
        assert!(staged.diff.is_empty());
        assert!(!staged.diff.cleared);

        let staged = stage(
            todo_write_args(&[], &[], &[], true),
            vec![item("a", "x", TodoStatus::Pending, 5)],
        )
        .expect("stage");
        assert!(staged.diff.cleared);
        assert!(staged.diff.removed.contains(&"a".to_string()));
    }

    #[test]
    fn stage_remove_unknown_ids_silently_ignored() {
        let current = vec![item("real", "x", TodoStatus::Pending, 5)];
        let staged = stage(todo_write_args(&[], &[], &["ghost"], false), current).expect("stage");
        assert_eq!(staged.final_todos.len(), 1);
        assert!(staged.diff.removed.is_empty());
    }

    #[test]
    fn stage_combined_add_update_remove_apply_in_order() {
        let current = vec![
            item("keep", "k", TodoStatus::Pending, 3),
            item("drop", "d", TodoStatus::Pending, 3),
        ];
        let args = TodoWriteArgs {
            add: vec![TodoAdd {
                id: "new".into(),
                content: "n".into(),
                status: Some(TodoStatus::Pending),
                priority: Some(7),
            }],
            update: vec![TodoUpdate {
                id: "keep".into(),
                content: None,
                status: Some(TodoStatus::InProgress),
                priority: None,
            }],
            remove: vec!["drop".into()],
            clear: false,
        };
        let staged = stage(args, current).expect("stage");
        assert_eq!(staged.final_todos.len(), 2);
        assert!(staged.diff.added.iter().any(|item| item.id == "new"));
        assert!(staged
            .diff
            .updated
            .iter()
            .any(|item| item.id == "keep" && item.status == TodoStatus::InProgress));
        assert_eq!(staged.diff.removed, vec!["drop".to_string()]);
    }

    #[test]
    fn stage_updates_diff_with_full_snapshot_after_change() {
        let current = vec![item("x", "old", TodoStatus::Pending, 3)];
        let args = TodoWriteArgs {
            update: vec![TodoUpdate {
                id: "x".into(),
                content: Some("new".into()),
                status: None,
                priority: Some(8),
            }],
            ..TodoWriteArgs::default()
        };
        let staged = stage(args, current).expect("stage");
        assert_eq!(staged.diff.updated.len(), 1);
        let snapshot = &staged.diff.updated[0];
        assert_eq!(snapshot.content, "new");
        assert_eq!(snapshot.priority, 8);
        assert_eq!(snapshot.status, TodoStatus::Pending);
    }

    #[test]
    fn stage_rejects_list_above_max() {
        let mut current = Vec::new();
        for index in 0..MAX_TODO_ITEMS {
            current.push(item(&format!("k-{index}"), "x", TodoStatus::Pending, 5));
        }
        let args = TodoWriteArgs {
            add: vec![TodoAdd {
                id: "overflow".into(),
                content: "x".into(),
                status: Some(TodoStatus::Pending),
                priority: Some(5),
            }],
            ..TodoWriteArgs::default()
        };
        assert!(stage(args, current).is_err());
    }

    #[test]
    fn sanitize_remove_ids_dedups_and_trims() {
        let ids = sanitize_remove_ids(vec![
            " a ".into(),
            "a".into(),
            "".into(),
            "b".into(),
            "b".into(),
        ]);
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn emit_outcome_with_noop_diff_labels_status_noop() {
        let ctx = ctx();
        let todos: Vec<TodoItem> = vec![];
        let diff = TodoDiff::default();
        let outcome = emit_outcome(&ctx, todos, diff);
        let stored = outcome.display.status.expect("status");
        assert_eq!(stored, "noop");
    }

    #[test]
    fn status_serializes_in_snake_case() {
        let json = serde_json::to_string(&TodoStatus::InProgress).expect("serialize");
        assert_eq!(json, "\"in_progress\"");
        let json = serde_json::to_string(&TodoStatus::Pending).expect("serialize");
        assert_eq!(json, "\"pending\"");
    }
}
