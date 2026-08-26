use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::AppHandle;
use thiserror::Error;

use crate::attachments::ChatAttachment;
use crate::providers::{attach_auth, load_all, ModelInfo, Provider, ProviderError, ProviderKind};
use crate::tools::{self, ModelToolCall, ToolContext, ToolDisplay};

const CHAT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const DEFAULT_TEMPERATURE: f64 = 1.0;
const DEFAULT_TOP_P: f64 = 0.95;
const DEFAULT_MAX_OUTPUT: u64 = 8192;
const OUTPUT_TOKEN_MAX: u64 = 32_000;
const TITLE_MAX_OUTPUT: u64 = 64;
const PROMPT_IMPROVE_MAX_OUTPUT: u64 = 4096;
const SKILL_COMPOSE_MAX_OUTPUT: u64 = 8192;
const PERSONALITY_COMPOSE_MAX_OUTPUT: u64 = 4096;
const MAX_TOOL_ROUNDS: usize = 12;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolCallTurn {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolResultTurn {
    pub call_id: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub reasoning_signature: Option<String>,
    #[serde(default)]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ChatToolCallTurn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ChatToolResultTurn>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatInput {
    pub provider_id: String,
    pub model_id: String,
    #[serde(default)]
    pub messages: Vec<ChatTurn>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedToolCall {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
    #[serde(default)]
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ToolDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRoundTrace {
    #[serde(default)]
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reasoning_signature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
    #[serde(default)]
    pub calls: Vec<PersistedToolCall>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatResult {
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reasoning_signature: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_rounds: Vec<ToolRoundTrace>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatChunk {
    pub kind: String,
    pub text: String,
}

struct ChatOutput {
    content: String,
    reasoning: String,
    reasoning_signature: String,
    tool_calls: Vec<ModelToolCall>,
    tool_rounds: Vec<ToolRoundTrace>,
}

#[derive(Clone)]
struct ToolResultTurn {
    call_id: String,
    name: String,
    content: String,
}

#[derive(Clone)]
struct Turn {
    assistant: bool,
    content: String,
    reasoning: String,
    reasoning_signature: String,
    attachments: Vec<ChatAttachment>,
    tool_calls: Vec<ModelToolCall>,
    tool_result: Option<ToolResultTurn>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatConfigLog {
    provider_id: String,
    provider_name: String,
    provider_kind: ProviderKind,
    base_url: String,
    model_id: String,
    effort: Option<String>,
    temperature: f64,
    max_output_tokens: u64,
    reasoning: bool,
    has_system: bool,
    has_api_key: bool,
    message: String,
}

struct ChatCall<'a> {
    model: &'a ModelInfo,
    turns: &'a [Turn],
    system: Option<&'a str>,
    effort: Option<&'a str>,
    max_output: u64,
    enable_reasoning: bool,
    tool_names: &'a [String],
    mcp_tools: &'a [crate::mcp_client::BoundMcpTool],
}

fn tools_enabled(call: &ChatCall<'_>) -> bool {
    !call.tool_names.is_empty() || !call.mcp_tools.is_empty()
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("message must not be empty")]
    EmptyMessage,
    #[error("provider with id {0} not found")]
    ProviderNotFound(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("api responded with status {status}: {body}")]
    ApiStatus { status: u16, body: String },
    #[error("empty model response")]
    EmptyResponse,
    #[error("interrupted by user")]
    Cancelled,
}

impl Serialize for ChatError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

impl From<ProviderError> for ChatError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error.to_string())
    }
}

fn chat_http_client() -> Result<reqwest::Client, ChatError> {
    reqwest::Client::builder()
        .timeout(CHAT_HTTP_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ChatError::Http(e.to_string()))
}

fn stream_http_client() -> Result<reqwest::Client, ChatError> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(120))
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ChatError::Http(e.to_string()))
}

fn host_of(provider: &Provider) -> String {
    provider.base_url.to_ascii_lowercase()
}

fn is_minimax(provider: &Provider, model: &ModelInfo) -> bool {
    if host_of(provider).contains("minimax") {
        return true;
    }
    if model.id.to_ascii_lowercase().contains("minimax") {
        return true;
    }
    model
        .family
        .as_deref()
        .is_some_and(|family| family.to_ascii_lowercase().contains("minimax"))
}

fn minimax_can_disable_thinking(model: &ModelInfo) -> bool {
    let id = model.id.to_ascii_lowercase();
    let family = model.family.as_deref().unwrap_or("").to_ascii_lowercase();
    id.contains("m3") || family.contains("m3")
}

fn enable_minimax_thinking(model: &ModelInfo, effort: Option<&str>) -> bool {
    match effort {
        Some("none") | Some("minimal") | Some("off") => !minimax_can_disable_thinking(model),
        _ => true,
    }
}

fn model_key(model: &ModelInfo) -> String {
    let mut key = model.id.to_ascii_lowercase();
    if let Some(family) = &model.family {
        key.push(' ');
        key.push_str(&family.to_ascii_lowercase());
    }
    key
}

fn is_glm52(key: &str) -> bool {
    key.contains("glm-5.2") || key.contains("glm-5-2") || key.contains("glm-5p2")
}

fn uses_max_completion_tokens(provider: &Provider, model: &ModelInfo) -> bool {
    if model.reasoning || is_minimax(provider, model) {
        return true;
    }
    let key = model_key(model);
    key.contains("o1")
        || key.contains("o3")
        || key.contains("o4")
        || key.contains("gpt-5")
        || key.contains("codex")
}

fn omits_temperature(provider: &Provider, model: &ModelInfo) -> bool {
    uses_max_completion_tokens(provider, model) && !is_minimax(provider, model)
}

fn token_field_error(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("max_tokens")
        || lower.contains("max_completion_tokens")
        || lower.contains("unsupported value") && lower.contains("temperature")
        || lower.contains("unsupported parameter") && lower.contains("temperature")
}

fn openai_tool_list(call: &ChatCall<'_>) -> serde_json::Value {
    let mut tools = match tools::openai_tools_for(call.tool_names) {
        serde_json::Value::Array(items) => items,
        _ => Vec::new(),
    };
    for item in call.mcp_tools {
        tools.push(json!({
            "type": "function",
            "function": {
                "name": item.wire_name,
                "description": item.description,
                "parameters": item.parameters,
                "strict": false,
            }
        }));
    }
    json!(tools)
}

fn anthropic_tool_list(call: &ChatCall<'_>) -> serde_json::Value {
    let mut tools = match tools::anthropic_tools_for(call.tool_names) {
        serde_json::Value::Array(items) => items,
        _ => Vec::new(),
    };
    for item in call.mcp_tools {
        tools.push(json!({
            "name": item.wire_name,
            "description": item.description,
            "input_schema": item.parameters,
        }));
    }
    json!(tools)
}

fn gemini_tool_list(call: &ChatCall<'_>) -> serde_json::Value {
    let mut declarations = match tools::gemini_tools_for(call.tool_names) {
        serde_json::Value::Array(mut groups) => groups
            .first_mut()
            .and_then(|group| group.get_mut("functionDeclarations"))
            .and_then(|value| value.as_array_mut())
            .map(std::mem::take)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for item in call.mcp_tools {
        declarations.push(json!({
            "name": item.wire_name,
            "description": item.description,
            "parameters": tools::sanitize_gemini_schema(&item.parameters),
        }));
    }
    json!([{ "functionDeclarations": declarations }])
}

fn thought_sig(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("")
        .to_string()
}

fn openai_compat_skips_effort(model: &ModelInfo) -> bool {
    let key = model_key(model);
    if key.contains("minimax") {
        return true;
    }
    if is_glm52(&key) {
        return false;
    }
    key.contains("deepseek")
        || key.contains("glm")
        || key.contains("kimi")
        || key.contains("k2p")
        || key.contains("qwen")
}

fn effort_is_off(effort: Option<&str>) -> bool {
    matches!(effort, Some("none" | "minimal" | "off"))
}

fn nonempty_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn last_user_text(turns: &[Turn]) -> &str {
    turns
        .iter()
        .rev()
        .find(|turn| !turn.assistant)
        .map(|turn| turn.content.as_str())
        .unwrap_or("")
}

fn last_user_has_input(turns: &[Turn]) -> bool {
    turns.iter().rev().any(|turn| {
        !turn.assistant && (!turn.content.trim().is_empty() || !turn.attachments.is_empty())
    })
}

fn user_turn(content: String) -> Turn {
    Turn {
        assistant: false,
        content,
        reasoning: String::new(),
        reasoning_signature: String::new(),
        attachments: Vec::new(),
        tool_calls: Vec::new(),
        tool_result: None,
    }
}

fn tool_arguments_json(name: &str, argument: Option<&str>, arguments: Option<&str>) -> String {
    let raw_arguments = arguments
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            serde_json::from_str::<serde_json::Value>(value)
                .ok()
                .and_then(|parsed| {
                    if parsed.is_object() {
                        Some(parsed)
                    } else {
                        None
                    }
                })
        });
    if let Some(parsed) = raw_arguments {
        return serde_json::to_string(&parsed).unwrap_or_else(|_| "{}".to_string());
    }
    let raw = argument.unwrap_or("").trim();
    if raw.is_empty() {
        return "{}".to_string();
    }
    if name == tools::SKILL_TOOL_NAME {
        return json!({ "name": raw }).to_string();
    }
    "{}".to_string()
}

fn tool_call_argument(name: &str, arguments: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(arguments) else {
        return String::new();
    };
    if name == tools::SKILL_TOOL_NAME {
        return value
            .get("name")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .unwrap_or("")
            .to_string();
    }
    if name == tools::READ_TOOL_NAME
        || name == tools::WRITE_TOOL_NAME
        || name == tools::EDIT_TOOL_NAME
        || name == tools::LIST_DIRECTORY_TOOL_NAME
    {
        let file_path = value
            .get(name_for_path_field(name))
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty());
        if name == tools::EDIT_TOOL_NAME {
            let old_string = value
                .get("oldString")
                .and_then(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty());
            return match (file_path, old_string) {
                (Some(path), Some(old)) => format!("{path} :: {old}"),
                (Some(path), None) => path.to_string(),
                _ => String::new(),
            };
        }
        return file_path.unwrap_or("").to_string();
    }
    String::new()
}

fn name_for_path_field(name: &str) -> &'static str {
    if name == tools::LIST_DIRECTORY_TOOL_NAME {
        "dirPath"
    } else {
        "filePath"
    }
}

fn normalize_turns(input: &[ChatTurn]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for item in input {
        let assistant = item.role.eq_ignore_ascii_case("assistant");
        if !assistant && !item.role.eq_ignore_ascii_case("user") {
            continue;
        }
        if let Some(result) = &item.tool_result {
            turns.push(Turn {
                assistant: false,
                content: String::new(),
                reasoning: String::new(),
                reasoning_signature: String::new(),
                attachments: Vec::new(),
                tool_calls: Vec::new(),
                tool_result: Some(ToolResultTurn {
                    call_id: result.call_id.clone(),
                    name: result.name.clone(),
                    content: result.content.clone(),
                }),
            });
            continue;
        }
        if assistant && !item.tool_calls.is_empty() {
            let tool_calls = item
                .tool_calls
                .iter()
                .map(|call| {
                    let id = if call.id.is_empty() {
                        tools::new_tool_call_id()
                    } else {
                        call.id.clone()
                    };
                    ModelToolCall {
                        id,
                        name: call.name.clone(),
                        arguments: tool_arguments_json(
                            &call.name,
                            call.argument.as_deref(),
                            call.arguments.as_deref(),
                        ),
                        thought_signature: call.thought_signature.clone().unwrap_or_default(),
                    }
                })
                .collect();
            turns.push(Turn {
                assistant: true,
                content: item.content.clone(),
                reasoning: item.reasoning.clone().unwrap_or_default(),
                reasoning_signature: item.reasoning_signature.clone().unwrap_or_default(),
                attachments: item.attachments.clone(),
                tool_calls,
                tool_result: None,
            });
            continue;
        }
        let content = item.content.clone();
        let reasoning = item.reasoning.clone().unwrap_or_default();
        let reasoning_signature = item.reasoning_signature.clone().unwrap_or_default();
        let attachments = item.attachments.clone();
        if content.trim().is_empty() && reasoning.trim().is_empty() && attachments.is_empty() {
            continue;
        }
        if let Some(last) = turns.last_mut() {
            if last.assistant == assistant
                && last.tool_calls.is_empty()
                && last.tool_result.is_none()
            {
                if !last.content.is_empty() && !content.is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(&content);
                last.attachments.extend(attachments);
                if assistant && !reasoning.trim().is_empty() {
                    last.reasoning = reasoning;
                    last.reasoning_signature = reasoning_signature;
                }
                continue;
            }
        }
        turns.push(Turn {
            assistant,
            content,
            reasoning,
            reasoning_signature,
            attachments,
            tool_calls: Vec::new(),
            tool_result: None,
        });
    }
    while turns.first().is_some_and(|turn| turn.assistant) {
        turns.remove(0);
    }
    while turns.last().is_some_and(|turn| turn.assistant) {
        turns.pop();
    }
    turns
}

fn turn_text(turn: &Turn) -> String {
    let mut text = turn.content.clone();
    for item in &turn.attachments {
        if item.kind != "text" && item.kind != "document" {
            continue;
        }
        let Some(extracted) = item
            .text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str("[");
        text.push_str(&item.name);
        text.push_str("]\n");
        text.push_str(extracted);
    }
    text
}

fn media_attachments(turn: &Turn) -> impl Iterator<Item = &ChatAttachment> {
    turn.attachments.iter().filter(|item| {
        matches!(item.kind.as_str(), "image" | "pdf" | "video" | "audio") && !item.data.is_empty()
    })
}

fn openai_user_content(turn: &Turn) -> serde_json::Value {
    let text = turn_text(turn);
    let media: Vec<&ChatAttachment> = media_attachments(turn).collect();
    if media.is_empty() {
        return json!(text);
    }
    let mut parts = Vec::new();
    if !text.trim().is_empty() {
        parts.push(json!({ "type": "text", "text": text }));
    }
    for item in media {
        if item.kind == "image" {
            parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", item.mime, item.data),
                },
            }));
            continue;
        }
        parts.push(json!({
            "type": "file",
            "file": {
                "filename": item.name,
                "file_data": format!("data:{};base64,{}", item.mime, item.data),
            },
        }));
    }
    json!(parts)
}

fn anthropic_user_content(turn: &Turn) -> serde_json::Value {
    let text = turn_text(turn);
    let media: Vec<&ChatAttachment> = media_attachments(turn).collect();
    if media.is_empty() {
        return json!(text);
    }
    let mut parts = Vec::new();
    for item in media {
        if item.kind == "image" {
            parts.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": item.mime,
                    "data": item.data,
                },
            }));
            continue;
        }
        parts.push(json!({
            "type": "document",
            "source": {
                "type": "base64",
                "media_type": item.mime,
                "data": item.data,
            },
        }));
    }
    if !text.trim().is_empty() {
        parts.push(json!({ "type": "text", "text": text }));
    }
    json!(parts)
}

fn gemini_user_parts(turn: &Turn) -> Vec<serde_json::Value> {
    let text = turn_text(turn);
    let mut parts = Vec::new();
    for item in media_attachments(turn) {
        parts.push(json!({
            "inlineData": {
                "mimeType": item.mime,
                "data": item.data,
            },
        }));
    }
    if !text.trim().is_empty() {
        parts.push(json!({ "text": text }));
    }
    if parts.is_empty() {
        parts.push(json!({ "text": "" }));
    }
    parts
}

fn openai_assistant_message(turn: &Turn) -> serde_json::Value {
    let mut message = serde_json::Map::new();
    message.insert("role".into(), json!("assistant"));
    if turn.content.is_empty() && !turn.tool_calls.is_empty() {
        message.insert("content".into(), serde_json::Value::Null);
    } else {
        message.insert("content".into(), json!(turn.content));
    }
    if !turn.reasoning.is_empty() {
        message.insert("reasoning_content".into(), json!(turn.reasoning));
    }
    if !turn.tool_calls.is_empty() {
        let tool_calls = turn
            .tool_calls
            .iter()
            .map(|call| {
                json!({
                    "id": call.id,
                    "type": "function",
                    "function": {
                        "name": call.name,
                        "arguments": call.arguments,
                    }
                })
            })
            .collect::<Vec<_>>();
        message.insert("tool_calls".into(), json!(tool_calls));
    }
    serde_json::Value::Object(message)
}

fn openai_messages(system: Option<&str>, turns: &[Turn]) -> serde_json::Value {
    let mut messages = Vec::with_capacity(turns.len() + 1);
    if let Some(system) = nonempty_text(system) {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for turn in turns {
        if let Some(result) = &turn.tool_result {
            messages.push(json!({
                "role": "tool",
                "tool_call_id": result.call_id,
                "content": result.content,
            }));
            continue;
        }
        if turn.assistant {
            messages.push(openai_assistant_message(turn));
            continue;
        }
        messages.push(json!({ "role": "user", "content": openai_user_content(turn) }));
    }
    json!(messages)
}

fn anthropic_messages(turns: &[Turn]) -> serde_json::Value {
    let mut messages = Vec::with_capacity(turns.len());
    for turn in turns {
        if let Some(result) = &turn.tool_result {
            messages.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": result.call_id,
                    "content": result.content,
                }],
            }));
            continue;
        }
        if !turn.assistant {
            messages.push(json!({ "role": "user", "content": anthropic_user_content(turn) }));
            continue;
        }
        messages.push(json!({
            "role": "assistant",
            "content": anthropic_assistant_blocks(turn),
        }));
    }
    json!(messages)
}

fn anthropic_assistant_blocks(turn: &Turn) -> Vec<serde_json::Value> {
    let mut blocks = Vec::new();
    if !turn.reasoning.is_empty() {
        let mut thinking = json!({
            "type": "thinking",
            "thinking": turn.reasoning,
        });
        if !turn.reasoning_signature.is_empty() {
            thinking["signature"] = json!(turn.reasoning_signature);
        }
        blocks.push(thinking);
    }
    if !turn.content.is_empty() {
        blocks.push(json!({ "type": "text", "text": turn.content }));
    }
    for call in &turn.tool_calls {
        let input = serde_json::from_str::<serde_json::Value>(&call.arguments)
            .unwrap_or(serde_json::Value::Null);
        blocks.push(json!({
            "type": "tool_use",
            "id": call.id,
            "name": call.name,
            "input": input,
        }));
    }
    if blocks.is_empty() {
        blocks.push(json!({ "type": "text", "text": "" }));
    }
    blocks
}

fn gemini_contents(turns: &[Turn]) -> serde_json::Value {
    let mut contents = Vec::with_capacity(turns.len());
    for turn in turns {
        if let Some(result) = &turn.tool_result {
            contents.push(json!({
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": result.name,
                        "response": { "output": result.content },
                    }
                }],
            }));
            continue;
        }
        if turn.assistant {
            let mut parts = Vec::new();
            if !turn.reasoning.is_empty() {
                let mut thought = json!({
                    "text": turn.reasoning,
                    "thought": true,
                });
                if !turn.reasoning_signature.is_empty() {
                    thought["thoughtSignature"] = json!(turn.reasoning_signature);
                }
                parts.push(thought);
            }
            if !turn.content.is_empty() {
                parts.push(json!({ "text": turn.content }));
            }
            let fallback_signature = nonempty_text(Some(&turn.reasoning_signature)).or_else(|| {
                turn.tool_calls
                    .iter()
                    .find(|call| !call.thought_signature.is_empty())
                    .map(|call| call.thought_signature.as_str())
            });
            for call in &turn.tool_calls {
                let args = serde_json::from_str::<serde_json::Value>(&call.arguments)
                    .unwrap_or(serde_json::Value::Null);
                let mut part = json!({
                    "functionCall": {
                        "name": call.name,
                        "args": args,
                    }
                });
                let signature = nonempty_text(Some(&call.thought_signature)).or(fallback_signature);
                if let Some(signature) = signature {
                    part["thoughtSignature"] = json!(signature);
                }
                parts.push(part);
            }
            if parts.is_empty() {
                continue;
            }
            contents.push(json!({
                "role": "model",
                "parts": parts,
            }));
            continue;
        }
        contents.push(json!({
            "role": "user",
            "parts": gemini_user_parts(turn),
        }));
    }
    json!(contents)
}

fn is_claude_host(provider: &Provider) -> bool {
    let host = host_of(provider);
    host.contains("anthropic.com") || host.contains("api.anthropic")
}

fn is_gemini_3(model_id: &str) -> bool {
    let id = model_id.to_ascii_lowercase();
    id.contains("gemini-3")
}

fn lookup_model(provider: &Provider, model_id: &str) -> ModelInfo {
    provider
        .models
        .iter()
        .find(|model| model.id == model_id)
        .cloned()
        .unwrap_or_else(|| ModelInfo::detected(model_id))
}

fn output_tokens(model: &ModelInfo) -> u64 {
    model
        .max_output_tokens
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_OUTPUT)
        .min(OUTPUT_TOKEN_MAX)
}

fn capped_output(model: &ModelInfo, cap: u64) -> u64 {
    output_tokens(model).min(cap).max(1)
}

fn resolve_effort(model: &ModelInfo, requested: Option<&str>) -> Option<String> {
    if !model.reasoning && model.effort_levels.is_empty() {
        return None;
    }
    if let Some(effort) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        if model.effort_levels.is_empty() || model.effort_levels.iter().any(|level| level == effort)
        {
            return Some(effort.to_string());
        }
    }
    for preferred in ["high", "xhigh", "max", "medium"] {
        if model.effort_levels.iter().any(|level| level == preferred) {
            return Some(preferred.to_string());
        }
    }
    if let Some(last) = model.effort_levels.last() {
        return Some(last.clone());
    }
    if model.reasoning {
        return Some("high".into());
    }
    None
}

fn is_effort_model(model: &ModelInfo) -> bool {
    model.reasoning || !model.effort_levels.is_empty()
}

fn gemini_thinking_level(effort: &str) -> &'static str {
    match effort {
        "none" | "minimal" => "minimal",
        "low" => "low",
        "medium" => "medium",
        _ => "high",
    }
}

fn extract_think(raw: &str) -> (String, String) {
    let mut rest = raw;
    let mut content = String::new();
    let mut reasoning = String::new();
    while let Some(start) = rest.find("<think>") {
        content.push_str(&rest[..start]);
        rest = &rest[start + 7..];
        match rest.find("</think>") {
            Some(end) => {
                reasoning.push_str(&rest[..end]);
                rest = &rest[end + 8..];
            }
            None => {
                reasoning.push_str(rest);
                rest = "";
                break;
            }
        }
    }
    content.push_str(rest);
    (content.trim().to_string(), reasoning.trim().to_string())
}

fn log_chat_config(provider: &Provider, input: &SendChatInput, call: &ChatCall<'_>) {
    let config = ChatConfigLog {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        provider_kind: provider.kind,
        base_url: provider.base_url.clone(),
        model_id: input.model_id.clone(),
        effort: call.effort.map(str::to_string),
        temperature: DEFAULT_TEMPERATURE,
        max_output_tokens: call.max_output,
        reasoning: call.enable_reasoning,
        has_system: call.system.is_some(),
        has_api_key: provider
            .api_key
            .as_ref()
            .is_some_and(|key| !key.trim().is_empty()),
        message: last_user_text(call.turns).to_string(),
    };
    match serde_json::to_string(&config) {
        Ok(json) => println!("[chat] config {json}"),
        Err(error) => eprintln!("[chat] config log failed: {error}"),
    }
}

#[derive(Default)]
struct ThinkSplit {
    pending: String,
    in_think: bool,
}

struct SplitOut {
    content: String,
    reasoning: String,
}

impl ThinkSplit {
    fn push(&mut self, incoming: &str) -> SplitOut {
        self.pending.push_str(incoming);
        let mut content = String::new();
        let mut reasoning = String::new();
        loop {
            if self.in_think {
                if let Some(end) = self.pending.find("</think>") {
                    reasoning.push_str(&self.pending[..end]);
                    self.pending.drain(..end + 8);
                    self.in_think = false;
                    continue;
                }
                reasoning.push_str(&self.pending);
                self.pending.clear();
                break;
            }
            if let Some(start) = self.pending.find("<think>") {
                content.push_str(&self.pending[..start]);
                self.pending.drain(..start + 7);
                self.in_think = true;
                continue;
            }
            let hold = incomplete_think_prefix(&self.pending);
            if hold > 0 {
                let emit_at = self.pending.len() - hold;
                content.push_str(&self.pending[..emit_at]);
                self.pending.drain(..emit_at);
            } else {
                content.push_str(&self.pending);
                self.pending.clear();
            }
            break;
        }
        SplitOut { content, reasoning }
    }

    fn finish(&mut self) -> SplitOut {
        if self.in_think {
            return SplitOut {
                content: String::new(),
                reasoning: std::mem::take(&mut self.pending),
            };
        }
        SplitOut {
            content: std::mem::take(&mut self.pending),
            reasoning: String::new(),
        }
    }
}

fn incomplete_think_prefix(text: &str) -> usize {
    const TAG: &str = "<think>";
    for n in (1..TAG.len()).rev() {
        if text.ends_with(&TAG[..n]) {
            return n;
        }
    }
    0
}

async fn consume_sse(
    resp: reqwest::Response,
    mut on_data: impl FnMut(&str) -> Result<(), ChatError>,
) -> Result<(), ChatError> {
    use futures_util::StreamExt;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ChatError::ApiStatus {
            status: status.as_u16(),
            body,
        });
    }
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut data_lines: Vec<String> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| ChatError::Http(error.to_string()))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let mut line = buf[..idx].to_string();
            buf.drain(..idx + 1);
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                if data_lines.is_empty() {
                    continue;
                }
                let data = data_lines.join("\n");
                data_lines.clear();
                if data.trim() == "[DONE]" {
                    return Ok(());
                }
                on_data(&data)?;
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.trim_start().to_string());
            }
        }
    }
    if !data_lines.is_empty() {
        let data = data_lines.join("\n");
        if data.trim() != "[DONE]" {
            on_data(&data)?;
        }
    }
    Ok(())
}

fn emit_chunk(on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>, kind: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(channel) = on_chunk {
        let _ = channel.send(ChatChunk {
            kind: kind.to_string(),
            text: text.to_string(),
        });
    }
}

fn emit_tool_call(on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>, call: &ModelToolCall) {
    let argument = tool_call_argument(&call.name, &call.arguments);
    let text = if argument.is_empty() {
        call.name.clone()
    } else {
        format!("{}\n{argument}", call.name)
    };
    emit_chunk(on_chunk, "tool", &text);
}

fn take_split(
    full_content: &mut String,
    full_reasoning: &mut String,
    split: SplitOut,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) {
    if !split.reasoning.is_empty() {
        full_reasoning.push_str(&split.reasoning);
        emit_chunk(on_chunk, "reasoning", &split.reasoning);
    }
    if !split.content.is_empty() {
        full_content.push_str(&split.content);
        emit_chunk(on_chunk, "content", &split.content);
    }
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|item| item.as_str()))
            .collect(),
        _ => String::new(),
    }
}

struct StreamToolDelta {
    index: usize,
    id: String,
    name: String,
    arguments: String,
    thought_signature: String,
    replace_args: bool,
}

struct StreamDelta {
    content: String,
    reasoning: String,
    reasoning_signature: String,
    tools: Vec<StreamToolDelta>,
}

impl Default for StreamDelta {
    fn default() -> Self {
        Self {
            content: String::new(),
            reasoning: String::new(),
            reasoning_signature: String::new(),
            tools: Vec::new(),
        }
    }
}

#[derive(Default)]
struct ToolCallBuilder {
    id: String,
    name: String,
    arguments: String,
    thought_signature: String,
}

fn merge_tool_delta(slots: &mut Vec<(usize, ToolCallBuilder)>, delta: StreamToolDelta) {
    if let Some((_, slot)) = slots.iter_mut().find(|(index, _)| *index == delta.index) {
        if !delta.id.is_empty() {
            slot.id = delta.id;
        }
        if !delta.name.is_empty() {
            slot.name = delta.name;
        }
        if delta.replace_args {
            if !delta.arguments.is_empty() {
                slot.arguments = delta.arguments;
            }
        } else {
            slot.arguments.push_str(&delta.arguments);
        }
        if !delta.thought_signature.is_empty() {
            slot.thought_signature = delta.thought_signature;
        }
        return;
    }
    slots.push((
        delta.index,
        ToolCallBuilder {
            id: delta.id,
            name: delta.name,
            arguments: delta.arguments,
            thought_signature: delta.thought_signature,
        },
    ));
    slots.sort_by_key(|(index, _)| *index);
}

fn finish_stream_tools(slots: Vec<(usize, ToolCallBuilder)>) -> Vec<ModelToolCall> {
    slots
        .into_iter()
        .filter(|(_, slot)| !slot.name.is_empty())
        .map(|(_, slot)| {
            let arguments = if slot.arguments.trim().is_empty() {
                "{}".to_string()
            } else {
                slot.arguments
            };
            let id = if slot.id.is_empty() {
                tools::new_tool_call_id()
            } else {
                slot.id
            };
            ModelToolCall {
                id,
                name: slot.name,
                arguments,
                thought_signature: slot.thought_signature,
            }
        })
        .collect()
}

fn openai_delta_pair(value: &serde_json::Value) -> StreamDelta {
    let delta = value.pointer("/choices/0/delta");
    let content = json_text(delta.and_then(|item| item.get("content")));
    let reasoning_content = json_text(delta.and_then(|item| item.get("reasoning_content")));
    let reasoning = if !reasoning_content.is_empty() {
        reasoning_content
    } else {
        json_text(delta.and_then(|item| item.get("reasoning")))
    };
    let mut tools = Vec::new();
    if let Some(calls) = delta
        .and_then(|item| item.get("tool_calls"))
        .and_then(|item| item.as_array())
    {
        for call in calls {
            let index = call
                .get("index")
                .and_then(|item| item.as_u64())
                .unwrap_or(0) as usize;
            let id = call
                .get("id")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            let function = call.get("function");
            let name = function
                .and_then(|item| item.get("name"))
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = function
                .and_then(|item| item.get("arguments"))
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            tools.push(StreamToolDelta {
                index,
                id,
                name,
                arguments,
                thought_signature: String::new(),
                replace_args: false,
            });
        }
    }
    StreamDelta {
        content,
        reasoning,
        reasoning_signature: String::new(),
        tools,
    }
}

fn anthropic_delta_pair(value: &serde_json::Value) -> StreamDelta {
    let event_type = value
        .get("type")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let index = value
        .get("index")
        .and_then(|item| item.as_u64())
        .unwrap_or(0) as usize;
    if event_type == "content_block_start" {
        let block = value.get("content_block");
        let block_type = block
            .and_then(|item| item.get("type"))
            .and_then(|item| item.as_str())
            .unwrap_or("");
        if block_type == "tool_use" {
            let id = block
                .and_then(|item| item.get("id"))
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            let name = block
                .and_then(|item| item.get("name"))
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            return StreamDelta {
                tools: vec![StreamToolDelta {
                    index,
                    id,
                    name,
                    arguments: String::new(),
                    thought_signature: String::new(),
                    replace_args: false,
                }],
                ..StreamDelta::default()
            };
        }
        return StreamDelta::default();
    }
    let delta_type = value
        .pointer("/delta/type")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    match delta_type {
        "text_delta" => StreamDelta {
            content: value
                .pointer("/delta/text")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string(),
            ..StreamDelta::default()
        },
        "thinking_delta" => StreamDelta {
            reasoning: value
                .pointer("/delta/thinking")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string(),
            ..StreamDelta::default()
        },
        "signature_delta" => StreamDelta {
            reasoning_signature: value
                .pointer("/delta/signature")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string(),
            ..StreamDelta::default()
        },
        "input_json_delta" => StreamDelta {
            tools: vec![StreamToolDelta {
                index,
                id: String::new(),
                name: String::new(),
                arguments: value
                    .pointer("/delta/partial_json")
                    .and_then(|item| item.as_str())
                    .unwrap_or("")
                    .to_string(),
                thought_signature: String::new(),
                replace_args: false,
            }],
            ..StreamDelta::default()
        },
        _ => StreamDelta::default(),
    }
}

fn gemini_delta_pair(value: &serde_json::Value) -> StreamDelta {
    let Some(parts) = value
        .pointer("/candidates/0/content/parts")
        .and_then(|item| item.as_array())
    else {
        return StreamDelta::default();
    };
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_signature = String::new();
    let mut tools = Vec::new();
    for part in parts {
        let part_signature = thought_sig(
            part.get("thoughtSignature")
                .and_then(|item| item.as_str()),
        );
        if !part_signature.is_empty() {
            reasoning_signature = part_signature.clone();
        }
        if let Some(call) = part.get("functionCall") {
            let name = call
                .get("name")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            let args = call.get("args").cloned().unwrap_or(serde_json::Value::Null);
            let arguments = if args.is_null() {
                "{}".to_string()
            } else {
                serde_json::to_string(&args).unwrap_or_else(|_| args.to_string())
            };
            tools.push(StreamToolDelta {
                index: tools.len(),
                id: String::new(),
                name,
                arguments,
                thought_signature: if part_signature.is_empty() {
                    reasoning_signature.clone()
                } else {
                    part_signature
                },
                replace_args: true,
            });
            continue;
        }
        let Some(text) = part.get("text").and_then(|item| item.as_str()) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if part.get("thought").and_then(|item| item.as_bool()) == Some(true) {
            reasoning.push_str(text);
        } else {
            content.push_str(text);
        }
    }
    StreamDelta {
        content,
        reasoning,
        reasoning_signature,
        tools,
    }
}

async fn collect_stream(
    resp: reqwest::Response,
    extract: impl Fn(&serde_json::Value) -> StreamDelta,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) -> Result<ChatOutput, ChatError> {
    let mut filter = ThinkSplit::default();
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_signature = String::new();
    let mut tool_slots: Vec<(usize, ToolCallBuilder)> = Vec::new();
    consume_sse(resp, |data| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            return Ok(());
        };
        let delta = extract(&value);
        if !delta.reasoning_signature.is_empty() {
            reasoning_signature.push_str(&delta.reasoning_signature);
        }
        if !delta.reasoning.is_empty() {
            reasoning.push_str(&delta.reasoning);
            emit_chunk(on_chunk, "reasoning", &delta.reasoning);
        }
        if !delta.content.is_empty() {
            take_split(
                &mut content,
                &mut reasoning,
                filter.push(&delta.content),
                on_chunk,
            );
        }
        for tool in delta.tools {
            merge_tool_delta(&mut tool_slots, tool);
        }
        Ok(())
    })
    .await?;
    take_split(&mut content, &mut reasoning, filter.finish(), on_chunk);
    let tool_calls = finish_stream_tools(tool_slots);
    if content.trim().is_empty() && reasoning.trim().is_empty() && tool_calls.is_empty() {
        Err(ChatError::EmptyResponse)
    } else {
        Ok(ChatOutput {
            content,
            reasoning,
            reasoning_signature,
            tool_calls,
            tool_rounds: Vec::new(),
        })
    }
}

fn apply_openai_like_reasoning(
    body: &mut serde_json::Value,
    provider: &Provider,
    call: &ChatCall<'_>,
) {
    if is_minimax(provider, call.model) {
        body["top_p"] = json!(DEFAULT_TOP_P);
        if call.enable_reasoning {
            body["thinking"] = json!({ "type": "adaptive" });
            body["reasoning_split"] = json!(true);
        } else {
            body["thinking"] = json!({ "type": "disabled" });
        }
        return;
    }
    if openai_compat_skips_effort(call.model) {
        return;
    }
    if let Some(effort) = call.effort {
        body["reasoning_effort"] = json!(effort);
    }
}

fn apply_anthropic_like_reasoning(
    body: &mut serde_json::Value,
    provider: &Provider,
    call: &ChatCall<'_>,
) {
    if is_minimax(provider, call.model) {
        body["top_p"] = json!(DEFAULT_TOP_P);
        if call.enable_reasoning {
            body["thinking"] = json!({ "type": "adaptive" });
        } else {
            body["thinking"] = json!({ "type": "disabled" });
        }
        return;
    }
    if is_claude_host(provider) && call.enable_reasoning && call.model.reasoning {
        let budget = (call.max_output / 2).clamp(1024, 16_384);
        let max_tokens = call.max_output.max(budget + 1024);
        body["max_tokens"] = json!(max_tokens);
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": budget,
        });
        return;
    }
    if call.enable_reasoning {
        if let Some(effort) = call.effort {
            if model_key(call.model).contains("kimi") {
                body["thinking"] = json!({ "type": "adaptive" });
                body["effort"] = json!(effort);
            } else {
                body["reasoning_effort"] = json!(effort);
            }
        }
    }
}

fn apply_gemini_like_reasoning(generation: &mut serde_json::Value, call: &ChatCall<'_>) {
    if !call.enable_reasoning || !call.model.reasoning {
        return;
    }
    if is_gemini_3(&call.model.id) {
        let level = call.effort.map(gemini_thinking_level).unwrap_or("high");
        generation["thinkingConfig"] = json!({
            "includeThoughts": true,
            "thinkingLevel": level,
        });
        return;
    }
    generation["thinkingConfig"] = json!({
        "includeThoughts": true,
        "thinkingBudget": -1,
    });
}

async fn send_openai_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) -> Result<ChatOutput, ChatError> {
    let stream = on_chunk.is_some();
    let client = if stream {
        stream_http_client()?
    } else {
        chat_http_client()?
    };
    let base = provider.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");
    let mut body = json!({
        "model": call.model.id,
        "messages": openai_messages(call.system, call.turns),
        "n": 1,
    });
    if !omits_temperature(provider, call.model) {
        body["temperature"] = json!(DEFAULT_TEMPERATURE);
    }
    let mut completion_tokens = uses_max_completion_tokens(provider, call.model);
    if completion_tokens {
        body["max_completion_tokens"] = json!(call.max_output);
    } else {
        body["max_tokens"] = json!(call.max_output);
    }
    apply_openai_like_reasoning(&mut body, provider, call);
    if tools_enabled(call) {
        body["tools"] = openai_tool_list(call);
    }
    if stream {
        body["stream"] = json!(true);
    }
    let send_once = |payload: &serde_json::Value| {
        let mut req = client.post(&url).json(payload);
        if stream {
            req = req.header(reqwest::header::ACCEPT, "text/event-stream");
        }
        attach_auth(req, provider)
    };
    let resp = send_once(&body)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, openai_delta_pair, on_chunk).await;
    }
    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        if !stream && token_field_error(&err_body) {
            if completion_tokens {
                body.as_object_mut()
                    .map(|map| map.remove("max_completion_tokens"));
                body["max_tokens"] = json!(call.max_output);
                completion_tokens = false;
            } else {
                body.as_object_mut().map(|map| map.remove("max_tokens"));
                body["max_completion_tokens"] = json!(call.max_output);
                completion_tokens = true;
            }
            let _ = completion_tokens;
            if omits_temperature(provider, call.model) {
                body["temperature"] = json!(DEFAULT_TEMPERATURE);
            } else {
                body.as_object_mut().map(|map| map.remove("temperature"));
            }
            let retry = send_once(&body)
                .send()
                .await
                .map_err(|e| ChatError::Http(e.to_string()))?;
            let retry_status = retry.status();
            if retry_status.is_success() {
                let parsed: OpenAiChatResponse = retry
                    .json()
                    .await
                    .map_err(|e| ChatError::Parse(e.to_string()))?;
                return parsed
                    .choices
                    .into_iter()
                    .next()
                    .and_then(|choice| choice.message.into_output())
                    .ok_or(ChatError::EmptyResponse);
            }
            let retry_body = retry.text().await.unwrap_or_default();
            return Err(ChatError::ApiStatus {
                status: retry_status.as_u16(),
                body: retry_body,
            });
        }
        return Err(ChatError::ApiStatus {
            status: status.as_u16(),
            body: err_body,
        });
    }
    let body: OpenAiChatResponse = resp
        .json()
        .await
        .map_err(|e| ChatError::Parse(e.to_string()))?;
    body.choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.into_output())
        .ok_or(ChatError::EmptyResponse)
}

async fn send_anthropic_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) -> Result<ChatOutput, ChatError> {
    let stream = on_chunk.is_some();
    let client = if stream {
        stream_http_client()?
    } else {
        chat_http_client()?
    };
    let base = provider.base_url.trim_end_matches('/');
    let url = format!("{base}/v1/messages");
    let mut body = json!({
        "model": call.model.id,
        "max_tokens": call.max_output,
        "temperature": DEFAULT_TEMPERATURE,
        "messages": anthropic_messages(call.turns),
    });
    if let Some(system) = nonempty_text(call.system) {
        body["system"] = json!(system);
    }
    apply_anthropic_like_reasoning(&mut body, provider, call);
    if tools_enabled(call) {
        body["tools"] = anthropic_tool_list(call);
    }
    if stream {
        body["stream"] = json!(true);
    }
    let mut req = client.post(&url).json(&body);
    if stream {
        req = req.header(reqwest::header::ACCEPT, "text/event-stream");
    }
    req = attach_auth(req, provider);
    if is_claude_host(provider) && call.enable_reasoning && call.model.reasoning {
        req = req.header("anthropic-beta", "interleaved-thinking-2025-05-14");
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, anthropic_delta_pair, on_chunk).await;
    }
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ChatError::ApiStatus {
            status: status.as_u16(),
            body,
        });
    }
    let body: AnthropicChatResponse = resp
        .json()
        .await
        .map_err(|e| ChatError::Parse(e.to_string()))?;
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_signature = String::new();
    let mut tool_calls = Vec::new();
    for block in body.content {
        match block.block_type.as_str() {
            "thinking" => {
                if !block.thinking.is_empty() {
                    reasoning.push_str(&block.thinking);
                } else if !block.text.is_empty() {
                    reasoning.push_str(&block.text);
                }
                if !block.signature.is_empty() {
                    reasoning_signature.push_str(&block.signature);
                }
            }
            "text" => content.push_str(&block.text),
            "tool_use" => {
                if block.id.is_empty() || block.name.is_empty() {
                    continue;
                }
                let arguments = if block.input.is_null() {
                    "{}".to_string()
                } else {
                    serde_json::to_string(&block.input).unwrap_or_else(|_| block.input.to_string())
                };
                tool_calls.push(ModelToolCall {
                    id: block.id.clone(),
                    name: block.name.clone(),
                    arguments,
                    thought_signature: String::new(),
                });
            }
            _ => {}
        }
    }
    let content = content.trim().to_string();
    let reasoning = reasoning.trim().to_string();
    if content.is_empty() && reasoning.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(ChatOutput {
        content,
        reasoning,
        reasoning_signature,
        tool_calls,
        tool_rounds: Vec::new(),
    })
}

async fn send_gemini_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) -> Result<ChatOutput, ChatError> {
    let stream = on_chunk.is_some();
    let client = if stream {
        stream_http_client()?
    } else {
        chat_http_client()?
    };
    let base = provider.base_url.trim_end_matches('/');
    let method = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    let url = match provider.api_key.as_deref() {
        Some(key) if stream => {
            format!(
                "{base}/v1beta/models/{}:{method}?alt=sse&key={key}",
                call.model.id
            )
        }
        Some(key) => format!("{base}/v1beta/models/{}:{method}?key={key}", call.model.id),
        None if stream => format!("{base}/v1beta/models/{}:{method}?alt=sse", call.model.id),
        None => format!("{base}/v1beta/models/{}:{method}", call.model.id),
    };
    let mut generation = json!({
        "maxOutputTokens": call.max_output,
    });
    if !is_gemini_3(&call.model.id) {
        generation["temperature"] = json!(DEFAULT_TEMPERATURE);
        generation["topP"] = json!(DEFAULT_TOP_P);
    }
    apply_gemini_like_reasoning(&mut generation, call);
    if tools_enabled(call) && generation.get("thinkingConfig").is_none() {
        generation["thinkingConfig"] = json!({ "includeThoughts": true });
    }
    let mut body = json!({
        "contents": gemini_contents(call.turns),
        "generationConfig": generation,
    });
    if let Some(system) = nonempty_text(call.system) {
        body["systemInstruction"] = json!({
            "parts": [{ "text": system }],
        });
    }
    if tools_enabled(call) {
        body["tools"] = gemini_tool_list(call);
    }
    let mut req = client.post(&url).json(&body);
    if stream {
        req = req.header(reqwest::header::ACCEPT, "text/event-stream");
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, gemini_delta_pair, on_chunk).await;
    }
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ChatError::ApiStatus {
            status: status.as_u16(),
            body,
        });
    }
    let body: GeminiChatResponse = resp
        .json()
        .await
        .map_err(|e| ChatError::Parse(e.to_string()))?;
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut reasoning_signature = String::new();
    let mut tool_calls = Vec::new();
    if let Some(parts) = body
        .candidates
        .into_iter()
        .next()
        .and_then(|candidate| candidate.content)
        .map(|item| item.parts)
    {
        for part in parts {
            let part_signature = thought_sig(part.thought_signature.as_deref());
            if !part_signature.is_empty() {
                reasoning_signature = part_signature.clone();
            }
            if let Some(call) = part.function_call {
                let arguments = if call.args.is_null() {
                    "{}".to_string()
                } else {
                    serde_json::to_string(&call.args).unwrap_or_else(|_| call.args.to_string())
                };
                tool_calls.push(ModelToolCall {
                    id: format!("gemini_{}_{}", call.name, tool_calls.len()),
                    name: call.name,
                    arguments,
                    thought_signature: if part_signature.is_empty() {
                        reasoning_signature.clone()
                    } else {
                        part_signature
                    },
                });
                continue;
            }
            let Some(text) = part.text else {
                continue;
            };
            if part.thought {
                reasoning.push_str(&text);
            } else {
                content.push_str(&text);
            }
        }
    }
    let content = content.trim().to_string();
    let reasoning = reasoning.trim().to_string();
    if content.is_empty() && reasoning.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(ChatOutput {
        content,
        reasoning,
        reasoning_signature,
        tool_calls,
        tool_rounds: Vec::new(),
    })
}

async fn dispatch_provider(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
) -> Result<ChatOutput, ChatError> {
    match provider.kind {
        ProviderKind::OpenAiLike => send_openai_like(provider, call, on_chunk).await,
        ProviderKind::AnthropicLike => send_anthropic_like(provider, call, on_chunk).await,
        ProviderKind::GeminiLike => send_gemini_like(provider, call, on_chunk).await,
    }
}

async fn send_message(
    app: &AppHandle,
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<ChatChunk>>,
    session_id: Option<&str>,
) -> Result<ChatOutput, ChatError> {
    if !last_user_has_input(call.turns) {
        return Err(ChatError::EmptyMessage);
    }
    if !tools_enabled(call) {
        let mut output = dispatch_provider(provider, call, on_chunk).await?;
        output.content = tools::truncate_assistant(&output.content);
        return Ok(ChatOutput {
            content: output.content,
            reasoning: output.reasoning,
            reasoning_signature: output.reasoning_signature,
            tool_calls: output.tool_calls,
            tool_rounds: Vec::new(),
        });
    }

    let mut turns = call.turns.to_vec();
    let ctx = ToolContext { app };
    let mut tool_rounds: Vec<ToolRoundTrace> = Vec::new();

    for _ in 0..MAX_TOOL_ROUNDS {
        let round_call = ChatCall {
            model: call.model,
            turns: &turns,
            system: call.system,
            effort: call.effort,
            max_output: call.max_output,
            enable_reasoning: call.enable_reasoning,
            tool_names: call.tool_names,
            mcp_tools: call.mcp_tools,
        };
        let output = dispatch_provider(provider, &round_call, on_chunk).await?;
        if on_chunk.is_none() && !output.reasoning.is_empty() {
            emit_chunk(on_chunk, "reasoning", &output.reasoning);
        }

        if output.tool_calls.is_empty() {
            if !output.content.is_empty() {
                let content = tools::truncate_assistant(&output.content);
                if on_chunk.is_none() {
                    emit_chunk(on_chunk, "content", &content);
                }
                return Ok(ChatOutput {
                    content,
                    reasoning: output.reasoning,
                    reasoning_signature: output.reasoning_signature,
                    tool_calls: Vec::new(),
                    tool_rounds,
                });
            }
            return Ok(ChatOutput {
                content: output.content,
                reasoning: output.reasoning,
                reasoning_signature: output.reasoning_signature,
                tool_calls: Vec::new(),
                tool_rounds,
            });
        }

        let model_calls: Vec<ModelToolCall> = output
            .tool_calls
            .iter()
            .map(|tc| {
                let id = if tc.id.is_empty() {
                    tools::new_tool_call_id()
                } else {
                    tc.id.clone()
                };
                ModelToolCall {
                    id,
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    thought_signature: tc.thought_signature.clone(),
                }
            })
            .collect();

        turns.push(Turn {
            assistant: true,
            content: output.content.clone(),
            reasoning: output.reasoning.clone(),
            reasoning_signature: output.reasoning_signature.clone(),
            attachments: Vec::new(),
            tool_calls: model_calls.clone(),
            tool_result: None,
        });

        let mut persisted_calls = Vec::new();
        for tc in &model_calls {
            emit_tool_call(on_chunk, tc);
            let (raw_text, display) =
                if let Some(mcp) = call.mcp_tools.iter().find(|item| item.wire_name == tc.name) {
                    let args = serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                    let text =
                        match crate::mcp_client::call_tool(&mcp.server, &mcp.tool_name, args).await
                        {
                            Ok(text) => text,
                            Err(error) => {
                                format!("MCP tool `{}` failed: {error}", mcp.tool_name)
                            }
                        };
                    (
                        text,
                        Some(ToolDisplay {
                            kind: tools::TOOL_KIND_CONTEXT.to_string(),
                            ..ToolDisplay::default()
                        }),
                    )
                } else if call.tool_names.iter().any(|name| name == &tc.name) {
                    let outcome = tools::execute(&tc.name, &tc.arguments, &ctx);
                    if let Some(snapshot) = outcome.snapshot {
                        if let Some(sid) = session_id {
                            let _ = crate::sessions::write_file_revision(
                                app,
                                sid,
                                &tc.id,
                                &snapshot.before,
                                &snapshot.after,
                            );
                        }
                    }
                    (outcome.text, Some(outcome.display))
                } else {
                    (
                        format!("Tool `{}` is not enabled for this agent.", tc.name),
                        Some(ToolDisplay {
                            kind: tools::TOOL_KIND_CONTEXT.to_string(),
                            status: Some("error".into()),
                            ..ToolDisplay::default()
                        }),
                    )
                };
            let outcome_text = tools::truncate_output(&raw_text);
            let argument = tool_call_argument(&tc.name, &tc.arguments);
            let mut display = display;
            if tc.name == tools::SKILL_TOOL_NAME && !argument.is_empty() {
                let meta = display.get_or_insert_with(|| ToolDisplay {
                    kind: tools::TOOL_KIND_CONTEXT.to_string(),
                    ..ToolDisplay::default()
                });
                if meta.skill_name.as_deref().unwrap_or("").is_empty() {
                    meta.skill_name = Some(argument.clone());
                }
            }
            persisted_calls.push(PersistedToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                argument: if argument.is_empty() {
                    None
                } else {
                    Some(argument)
                },
                arguments: nonempty_text(Some(&tc.arguments)).map(str::to_string),
                thought_signature: nonempty_text(Some(&tc.thought_signature)).map(str::to_string),
                output: outcome_text.clone(),
                display,
            });
            turns.push(Turn {
                assistant: false,
                content: String::new(),
                reasoning: String::new(),
                reasoning_signature: String::new(),
                attachments: Vec::new(),
                tool_calls: Vec::new(),
                tool_result: Some(ToolResultTurn {
                    call_id: tc.id.clone(),
                    name: tc.name.clone(),
                    content: outcome_text,
                }),
            });
        }

        tool_rounds.push(ToolRoundTrace {
            reasoning: output.reasoning.clone(),
            reasoning_signature: output.reasoning_signature.clone(),
            content: output.content.clone(),
            calls: persisted_calls,
        });
    }

    Err(ChatError::Provider("tool loop exceeded max rounds".into()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSessionTitleInput {
    pub provider_id: String,
    pub model_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSessionTitleResult {
    pub title: String,
}

const TITLE_PROMPT: &str =
    "Reply with only a short session title (max 6 words, no quotes, no trailing punctuation) for this chat message:\n\n";

fn normalize_generated_title(raw: &str) -> String {
    raw.lines()
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

#[tauri::command]
pub async fn generate_session_title(
    app: AppHandle,
    input: GenerateSessionTitleInput,
) -> Result<GenerateSessionTitleResult, ChatError> {
    let (provider, model) = load_provider_model(&app, &input.provider_id, &input.model_id).await?;
    let prompt = format!("{TITLE_PROMPT}{}", input.message.trim());
    let turns = vec![user_turn(prompt)];
    let call = ChatCall {
        model: &model,
        turns: &turns,
        system: None,
        effort: None,
        max_output: capped_output(&model, TITLE_MAX_OUTPUT),
        enable_reasoning: false,
        tool_names: &[],
        mcp_tools: &[],
    };
    let title =
        normalize_generated_title(&send_message(&app, &provider, &call, None, None).await?.content);
    if title.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(GenerateSessionTitleResult { title })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GenerateAppContentKind {
    ImprovePrompt,
    ComposeSkill,
    ComposePersonality,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAppContentInput {
    pub provider_id: String,
    pub model_id: String,
    pub kind: GenerateAppContentKind,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAppContentResult {
    pub text: String,
}

const IMPROVE_PROMPT_PREFIX: &str = "Improve this user prompt for an AI coding assistant. Make it clearer, more specific, and actionable. Keep the same intent and language. Reply with only the improved prompt, no preamble or quotes:\n\n";

const COMPOSE_SKILL_WITH_CONTENT: &str = "Reorganize this SKILL.md for a professional agent skill. Keep valid YAML frontmatter (name, description). Structure the body with clear sections such as when to use, instructions, examples, and edge cases. Be concise and actionable. Reply with only the full SKILL.md file, no preamble:\n\n";

const COMPOSE_SKILL_EMPTY: &str = "Write a professional SKILL.md for an agent skill.\n\nSkill name: {name}\nDescription: {description}\n\nUse YAML frontmatter (name, description) and body sections such as when to use, instructions, examples, and edge cases. Reply with only the full SKILL.md file, no preamble.";

const COMPOSE_PERSONALITY_WITH_CONTENT: &str = "Reorganize this agent personality for a coding assistant. Use short paragraphs and bullet lists. Define role, behavior, constraints, and workflow. Stay under 200 lines. Reply with only the personality text (no frontmatter), no preamble:\n\n";

const COMPOSE_PERSONALITY_EMPTY: &str = "Write an agent personality for a coding assistant.\n\nAgent name: {name}\nDescription: {description}\n\nUse short paragraphs and bullet lists. Define role, behavior, constraints, and workflow. Stay under 200 lines. Reply with only the personality text (no frontmatter), no preamble.";

fn normalize_generated_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let body = trimmed.trim_start_matches('`');
        let body = body
            .strip_prefix("markdown")
            .or_else(|| body.strip_prefix("md"))
            .unwrap_or(body);
        let body = body.trim_start_matches('\n');
        if let Some(end) = body.rfind("```") {
            return body[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn build_app_content_prompt(input: &GenerateAppContentInput) -> Result<String, ChatError> {
    let content = input.content.trim();
    let prompt = match input.kind {
        GenerateAppContentKind::ImprovePrompt => {
            if content.is_empty() {
                return Err(ChatError::EmptyMessage);
            }
            format!("{IMPROVE_PROMPT_PREFIX}{content}")
        }
        GenerateAppContentKind::ComposeSkill => {
            if content.is_empty() {
                let name = input.name.trim();
                if name.is_empty() {
                    return Err(ChatError::Provider("skill name is required".into()));
                }
                COMPOSE_SKILL_EMPTY
                    .replace("{name}", name)
                    .replace("{description}", input.description.trim())
            } else {
                format!("{COMPOSE_SKILL_WITH_CONTENT}{content}")
            }
        }
        GenerateAppContentKind::ComposePersonality => {
            if content.is_empty() {
                let name = input.name.trim();
                if name.is_empty() {
                    return Err(ChatError::Provider("agent name is required".into()));
                }
                COMPOSE_PERSONALITY_EMPTY
                    .replace("{name}", name)
                    .replace("{description}", input.description.trim())
            } else {
                format!("{COMPOSE_PERSONALITY_WITH_CONTENT}{content}")
            }
        }
    };
    Ok(prompt)
}

fn app_content_output_cap(kind: GenerateAppContentKind) -> u64 {
    match kind {
        GenerateAppContentKind::ImprovePrompt => PROMPT_IMPROVE_MAX_OUTPUT,
        GenerateAppContentKind::ComposeSkill => SKILL_COMPOSE_MAX_OUTPUT,
        GenerateAppContentKind::ComposePersonality => PERSONALITY_COMPOSE_MAX_OUTPUT,
    }
}

#[tauri::command]
pub async fn generate_app_content(
    app: AppHandle,
    input: GenerateAppContentInput,
) -> Result<GenerateAppContentResult, ChatError> {
    let (provider, model) = load_provider_model(&app, &input.provider_id, &input.model_id).await?;
    let prompt = build_app_content_prompt(&input)?;
    let turns = vec![user_turn(prompt)];
    let max_output = capped_output(&model, app_content_output_cap(input.kind));
    let call = ChatCall {
        model: &model,
        turns: &turns,
        system: None,
        effort: None,
        max_output,
        enable_reasoning: false,
        tool_names: &[],
        mcp_tools: &[],
    };
    let text = normalize_generated_text(&send_message(&app, &provider, &call, None, None).await?.content);
    if text.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(GenerateAppContentResult { text })
}

async fn load_provider_model(
    app: &AppHandle,
    provider_id: &str,
    model_id: &str,
) -> Result<(Provider, ModelInfo), ChatError> {
    let providers = load_all(app).await?;
    let mut provider = providers
        .into_iter()
        .find(|item| item.id == provider_id)
        .ok_or_else(|| ChatError::ProviderNotFound(provider_id.to_string()))?;
    let catalog = crate::catalog::load(app).await;
    for model in &mut provider.models {
        catalog.apply(model);
    }
    let mut model = lookup_model(&provider, model_id);
    catalog.apply(&mut model);
    Ok((provider, model))
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatMessage,
}

#[derive(Deserialize)]
struct OpenAiChatMessage {
    content: Option<OpenAiContent>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiToolFunction,
}

#[derive(Deserialize)]
struct OpenAiToolFunction {
    name: String,
    arguments: String,
}

impl OpenAiChatMessage {
    fn into_output(self) -> Option<ChatOutput> {
        let raw = self
            .content
            .and_then(|content| content.into_text())
            .unwrap_or_default();
        let (content, tagged) = extract_think(&raw);
        let mut reasoning = self.reasoning_content.unwrap_or_default();
        if !tagged.is_empty() {
            if !reasoning.is_empty() {
                reasoning.push('\n');
            }
            reasoning.push_str(&tagged);
        }
        let reasoning = reasoning.trim().to_string();
        let tool_calls = self
            .tool_calls
            .into_iter()
            .map(|call| ModelToolCall {
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
                thought_signature: String::new(),
            })
            .collect::<Vec<_>>();
        if content.is_empty() && reasoning.is_empty() && tool_calls.is_empty() {
            None
        } else {
            Some(ChatOutput {
                content,
                reasoning,
                reasoning_signature: String::new(),
                tool_calls,
                tool_rounds: Vec::new(),
            })
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OpenAiContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

impl OpenAiContent {
    fn into_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Parts(parts) => {
                let text = parts
                    .into_iter()
                    .filter(|part| {
                        part.kind
                            .as_deref()
                            .is_none_or(|kind| kind == "text" || kind == "output_text")
                    })
                    .filter_map(|part| part.text)
                    .collect::<Vec<_>>()
                    .join("");
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct OpenAiContentPart {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicChatResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    signature: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: serde_json::Value,
}

#[derive(Deserialize)]
struct GeminiChatResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiPart {
    text: Option<String>,
    #[serde(default)]
    thought: bool,
    #[serde(default, rename = "thoughtSignature")]
    thought_signature: Option<String>,
    #[serde(default, rename = "functionCall")]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Deserialize)]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    cancel: tauri::State<'_, crate::CancelRegistry>,
    input: SendChatInput,
    on_chunk: tauri::ipc::Channel<ChatChunk>,
) -> Result<SendChatResult, ChatError> {
    let (_cancel_guard, cancel_rx) = match input.session_id.clone() {
        Some(id) => {
            let (guard, rx) = crate::CancelHandle::new(cancel.inner(), id);
            (Some(guard), Some(rx))
        }
        None => (None, None),
    };
    let (provider, model) = load_provider_model(&app, &input.provider_id, &input.model_id).await?;
    let mut turns = normalize_turns(&input.messages);
    if let Some(session_id) = input.session_id.as_deref() {
        for turn in &mut turns {
            let _ = crate::sessions::hydrate_attachments(&app, Some(session_id), &mut turn.attachments);
        }
    }
    if !last_user_has_input(&turns) {
        return Err(ChatError::EmptyMessage);
    }
    let effort = resolve_effort(&model, input.effort.as_deref());
    let enable_reasoning = if is_minimax(&provider, &model) {
        enable_minimax_thinking(&model, effort.as_deref())
    } else if effort_is_off(effort.as_deref()) {
        false
    } else {
        is_effort_model(&model)
    };
    let system = nonempty_text(input.system.as_deref()).map(str::to_string);
    let registered: std::collections::HashSet<String> = tools::specs()
        .into_iter()
        .map(|spec| spec.name.to_string())
        .collect();
    let tool_names: Vec<String> = input
        .tool_names
        .iter()
        .filter(|name| registered.contains(name.as_str()))
        .cloned()
        .collect();
    let mcp_tools = crate::mcp_client::bound_tools_for_chat(&app).await;
    let call = ChatCall {
        model: &model,
        turns: &turns,
        system: system.as_deref(),
        effort: effort.as_deref(),
        max_output: output_tokens(&model),
        enable_reasoning,
        tool_names: &tool_names,
        mcp_tools: &mcp_tools,
    };
    log_chat_config(&provider, &input, &call);
    let send_fut = send_message(
        &app,
        &provider,
        &call,
        Some(&on_chunk),
        input.session_id.as_deref(),
    );
    tokio::pin!(send_fut);
    let output = match cancel_rx {
        Some(rx) => {
            tokio::select! {
                biased;
                result = rx => match result {
                    Ok(()) => return Err(ChatError::Cancelled),
                    Err(_) => send_fut.await?,
                },
                output = &mut send_fut => output?,
            }
        }
        None => send_fut.await?,
    };
    Ok(SendChatResult {
        content: output.content,
        reasoning: output.reasoning,
        reasoning_signature: output.reasoning_signature,
        tool_rounds: output.tool_rounds,
    })
}
