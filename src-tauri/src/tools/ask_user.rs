use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;

use super::{
    yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue, TOOL_KIND_CONTEXT,
};
use crate::chat::ChatChunk;

pub const NAME: &str = "ask_user";

const DESCRIPTION: &str = "Ask the user one or more questions (1-4) and block until they answer. Use sparingly to confirm important choices. Each question may include selectable choices, multi-select, and an optional free-text input alongside the options.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskUserOption {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskUserQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<AskUserOption>,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default = "default_allow_free_text")]
    pub allow_free_text: bool,
}

fn default_allow_free_text() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskUserAnswerEntry {
    pub question_id: String,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub free_text: String,
    #[serde(default)]
    pub skipped: bool,
}

pub type AskUserAnswer = Vec<AskUserAnswerEntry>;

#[derive(Default)]
pub struct AskUserRegistry {
    pending: Mutex<HashMap<String, oneshot::Sender<AskUserAnswer>>>,
}

impl AskUserRegistry {
    pub fn insert(&self, key: String, sender: oneshot::Sender<AskUserAnswer>) {
        if let Ok(mut map) = self.pending.lock() {
            map.insert(key, sender);
        }
    }

    pub fn complete(&self, key: &str, answer: AskUserAnswer) -> bool {
        let Ok(mut map) = self.pending.lock() else {
            return false;
        };
        match map.remove(key) {
            Some(tx) => {
                let _ = tx.send(answer);
                true
            }
            None => false,
        }
    }

    pub fn cancel(&self, key: &str) -> bool {
        let Ok(mut map) = self.pending.lock() else {
            return false;
        };
        match map.remove(key) {
            Some(tx) => {
                let _ = tx.send(Vec::new());
                true
            }
            None => false,
        }
    }

    pub fn drain(&self) {
        let Ok(mut map) = self.pending.lock() else {
            return;
        };
        for (_, tx) in map.drain() {
            let _ = tx.send(Vec::new());
        }
    }
}

static ASK_USER_REGISTRY: OnceLock<Arc<AskUserRegistry>> = OnceLock::new();

pub fn ask_user_registry() -> Arc<AskUserRegistry> {
    ASK_USER_REGISTRY
        .get_or_init(|| Arc::new(AskUserRegistry::default()))
        .clone()
}

pub struct AskUserTool;

impl Tool for AskUserTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 4,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Stable id used in answers." },
                                "header": { "type": "string", "description": "Short tab label (1-4 words)." },
                                "question": { "type": "string", "description": "Full question text." },
                                "options": {
                                    "type": "array",
                                    "minItems": 2,
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": { "type": "string" },
                                            "description": { "type": "string" },
                                            "preview": { "type": "string" }
                                        },
                                        "required": ["label"]
                                    }
                                },
                                "multiSelect": { "type": "boolean", "description": "Allow multiple option selection (default false)." },
                                "allowFreeText": { "type": "boolean", "description": "Show a free-text input next to the options (default true)." }
                            },
                            "required": ["id", "header", "question", "options"]
                        }
                    }
                },
                "required": ["questions"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let _ = ctx;
        match parse_questions(args) {
            Ok(_) => super::context_error(None, "ask_user must run on the async dispatch path."),
            Err(message) => super::context_error(None, &message),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let questions = match parse_questions(&args) {
        Ok(value) => value,
        Err(message) => return super::context_error(None, &message),
    };
    let call_id = ctx.call_id.clone();
    let answer = ask_user_wait(ctx, &call_id, &questions).await;
    let text = format_answer(&questions, &answer);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some(if answer.is_empty() {
                "cancelled".into()
            } else {
                "ok".into()
            }),
            ..ToolDisplay::default()
        },
        snapshot: None,
    }
}

pub async fn ask_user_wait(
    ctx: &ToolContext<'_>,
    call_id: &str,
    questions: &[AskUserQuestion],
) -> AskUserAnswer {
    let (tx, rx) = oneshot::channel::<AskUserAnswer>();
    ask_user_registry().insert(call_id.to_string(), tx);

    if let Some(channel) = ctx.on_chunk {
        let payload = json!({
            "callId": call_id,
            "questions": questions,
        });
        let _ = channel.send(ChatChunk {
            kind: "question".to_string(),
            text: payload.to_string(),
        });
    }

    rx.await.unwrap_or_default()
}

fn parse_questions(args: &Value) -> Result<Vec<AskUserQuestion>, String> {
    let Some(value) = args.get("questions").and_then(Value::as_array) else {
        return Err("ask_user tool requires a `questions` array.".into());
    };
    if value.is_empty() {
        return Err("ask_user `questions` must contain at least one item.".into());
    }
    if value.len() > 4 {
        return Err("ask_user `questions` can hold at most 4 items.".into());
    }
    let parsed: Vec<AskUserQuestion> = serde_json::from_value(Value::Array(value.clone()))
        .map_err(|error| format!("ask_user failed to parse `questions`: {error}"))?;
    if parsed.iter().any(|question| question.options.len() < 2) {
        return Err("ask_user every question needs at least two options.".into());
    }
    Ok(parsed)
}

fn format_answer(questions: &[AskUserQuestion], answer: &[AskUserAnswerEntry]) -> String {
    if answer.is_empty() {
        return yaml_doc(&[("status", YamlValue::Str("cancelled"))]);
    }
    let mut fields: Vec<(&str, YamlValue<'_>)> = Vec::new();
    fields.push(("status", YamlValue::Str("ok")));
    let mut summary = String::new();
    for (idx, question) in questions.iter().enumerate() {
        let entry = answer.iter().find(|entry| entry.question_id == question.id);
        let line = match entry {
            Some(entry) => {
                let labels: Vec<String> = entry
                    .selected
                    .iter()
                    .filter_map(|sel| {
                        question
                            .options
                            .iter()
                            .find(|opt| opt.label == *sel)
                            .map(|opt| opt.label.clone())
                    })
                    .collect();
                let mut pieces = Vec::new();
                if !labels.is_empty() {
                    pieces.push(labels.join(", "));
                }
                if !entry.free_text.trim().is_empty() {
                    pieces.push(format!("free-text: {}", entry.free_text.trim()));
                }
                if pieces.is_empty() {
                    pieces.push("(no selection)".to_string());
                }
                format!("- {}: {}", question.header, pieces.join(" | "))
            }
            None => format!("- {}: (skipped)", question.header),
        };
        if idx > 0 {
            summary.push('\n');
        }
        summary.push_str(&line);
    }
    fields.push(("answers", YamlValue::Block(&summary)));
    yaml_doc(&fields)
}

#[tauri::command]
pub fn submit_ask_user_answer(
    call_id: String,
    answers: Vec<AskUserAnswerEntry>,
) -> Result<bool, String> {
    let delivered = ask_user_registry().complete(&call_id, answers);
    if !delivered {
        return Err(format!("ask_user call `{call_id}` was not pending."));
    }
    Ok(true)
}

#[tauri::command]
pub fn cancel_ask_user_answer(call_id: String) -> Result<bool, String> {
    Ok(ask_user_registry().cancel(&call_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_questions_errors() {
        let dir = std::env::temp_dir();
        let ctx = crate::tools::ToolContext::for_test(dir, 1);
        let outcome = execute_async("{}", &ctx).await;
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.text.contains("questions"));
    }

    #[test]
    fn submit_without_pending_errors() {
        let err = submit_ask_user_answer("missing".into(), Vec::new()).unwrap_err();
        assert!(err.contains("not pending"));
    }
}