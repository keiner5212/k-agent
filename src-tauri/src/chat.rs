use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::AppHandle;
use thiserror::Error;

use crate::providers::{attach_auth, load_all, ModelInfo, Provider, ProviderError, ProviderKind};

const CHAT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const DEFAULT_TEMPERATURE: f64 = 1.0;
const DEFAULT_TOP_P: f64 = 0.95;
const DEFAULT_MAX_OUTPUT: u64 = 8192;
const TITLE_MAX_OUTPUT: u64 = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatInput {
    pub provider_id: String,
    pub model_id: String,
    pub message: String,
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatResult {
    pub content: String,
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
    has_api_key: bool,
    message: String,
}

struct ChatCall<'a> {
    model: &'a ModelInfo,
    message: &'a str,
    effort: Option<&'a str>,
    max_output: u64,
    enable_reasoning: bool,
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

fn is_minimax(provider: &Provider) -> bool {
    host_of(provider).contains("minimax")
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

fn strip_think(raw: &str) -> String {
    let mut rest = raw;
    let mut out = String::new();
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 7..];
        match rest.find("</think>") {
            Some(end) => rest = &rest[end + 8..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
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
        reasoning: call.enable_reasoning && call.model.reasoning,
        has_api_key: provider
            .api_key
            .as_ref()
            .is_some_and(|key| !key.trim().is_empty()),
        message: input.message.clone(),
    };
    match serde_json::to_string(&config) {
        Ok(json) => println!("[chat] config {json}"),
        Err(error) => eprintln!("[chat] config log failed: {error}"),
    }
}

#[derive(Default)]
struct ThinkFilter {
    pending: String,
    in_think: bool,
}

impl ThinkFilter {
    fn push(&mut self, incoming: &str) -> String {
        self.pending.push_str(incoming);
        let mut visible = String::new();
        loop {
            if self.in_think {
                if let Some(end) = self.pending.find("</think>") {
                    self.pending.drain(..end + 8);
                    self.in_think = false;
                    continue;
                }
                break;
            }
            if let Some(start) = self.pending.find("<think>") {
                visible.push_str(&self.pending[..start]);
                self.pending.drain(..start + 7);
                self.in_think = true;
                continue;
            }
            let hold = incomplete_think_prefix(&self.pending);
            if hold > 0 {
                let emit_at = self.pending.len() - hold;
                visible.push_str(&self.pending[..emit_at]);
                self.pending.drain(..emit_at);
            } else {
                visible.push_str(&self.pending);
                self.pending.clear();
            }
            break;
        }
        visible
    }

    fn finish(&mut self) -> String {
        if self.in_think {
            self.pending.clear();
            return String::new();
        }
        let rest = std::mem::take(&mut self.pending);
        rest
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

fn emit_visible(
    filter: &mut ThinkFilter,
    full: &mut String,
    piece: &str,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) {
    let visible = filter.push(piece);
    if visible.is_empty() {
        return;
    }
    full.push_str(&visible);
    if let Some(channel) = on_chunk {
        let _ = channel.send(visible);
    }
}

fn openai_delta_text(value: &serde_json::Value) -> Option<String> {
    let content = value.pointer("/choices/0/delta/content")?;
    match content {
        serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
        serde_json::Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
                .collect::<String>();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    }
}

fn anthropic_delta_text(value: &serde_json::Value) -> Option<String> {
    let delta_type = value.pointer("/delta/type")?.as_str()?;
    if delta_type != "text_delta" {
        return None;
    }
    value
        .pointer("/delta/text")
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn gemini_delta_text(value: &serde_json::Value) -> Option<String> {
    let parts = value.pointer("/candidates/0/content/parts")?.as_array()?;
    let text = parts
        .iter()
        .filter(|part| part.get("thought").and_then(|value| value.as_bool()) != Some(true))
        .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
        .collect::<String>();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

async fn collect_stream(
    resp: reqwest::Response,
    extract: impl Fn(&serde_json::Value) -> Option<String>,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) -> Result<String, ChatError> {
    let mut filter = ThinkFilter::default();
    let mut full = String::new();
    consume_sse(resp, |data| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            return Ok(());
        };
        if let Some(piece) = extract(&value) {
            emit_visible(&mut filter, &mut full, &piece, on_chunk);
        }
        Ok(())
    })
    .await?;
    let tail = filter.finish();
    if !tail.is_empty() {
        full.push_str(&tail);
        if let Some(channel) = on_chunk {
            let _ = channel.send(tail);
        }
    }
    let trimmed = full.trim();
    if trimmed.is_empty() {
        Err(ChatError::EmptyResponse)
    } else {
        Ok(full)
    }
}

async fn send_openai_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) -> Result<String, ChatError> {
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
        "messages": [{ "role": "user", "content": call.message }],
        "n": 1,
        "temperature": DEFAULT_TEMPERATURE,
    });
    if call.model.reasoning {
        body["max_completion_tokens"] = json!(call.max_output);
    } else {
        body["max_tokens"] = json!(call.max_output);
    }
    if is_minimax(provider) {
        body["top_p"] = json!(DEFAULT_TOP_P);
        if call.enable_reasoning {
            body["thinking"] = json!({ "type": "adaptive" });
            body["reasoning_split"] = json!(true);
        } else {
            body["thinking"] = json!({ "type": "disabled" });
        }
    }
    if call.enable_reasoning {
        if let Some(effort) = call.effort {
            body["reasoning_effort"] = json!(effort);
        }
    }
    if stream {
        body["stream"] = json!(true);
    }
    let mut req = client.post(&url).json(&body);
    if stream {
        req = req.header(reqwest::header::ACCEPT, "text/event-stream");
    }
    let resp = attach_auth(req, provider)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, openai_delta_text, on_chunk).await;
    }
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ChatError::ApiStatus {
            status: status.as_u16(),
            body,
        });
    }
    let body: OpenAiChatResponse = resp
        .json()
        .await
        .map_err(|e| ChatError::Parse(e.to_string()))?;
    body.choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.visible_text())
        .ok_or(ChatError::EmptyResponse)
}

async fn send_anthropic_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) -> Result<String, ChatError> {
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
        "messages": [{ "role": "user", "content": call.message }],
    });
    if is_minimax(provider) {
        body["top_p"] = json!(DEFAULT_TOP_P);
        if call.enable_reasoning {
            body["thinking"] = json!({ "type": "adaptive" });
        } else {
            body["thinking"] = json!({ "type": "disabled" });
        }
    } else if is_claude_host(provider) && call.enable_reasoning && call.model.reasoning {
        let budget = (call.max_output / 2).clamp(1024, 16_384);
        let max_tokens = call.max_output.max(budget + 1024);
        body["max_tokens"] = json!(max_tokens);
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": budget,
        });
    }
    if call.enable_reasoning {
        if let Some(effort) = call.effort {
            if !is_claude_host(provider) {
                body["reasoning_effort"] = json!(effort);
            }
        }
    }
    if stream {
        body["stream"] = json!(true);
    }
    let mut req = client.post(&url).json(&body);
    if stream {
        req = req.header(reqwest::header::ACCEPT, "text/event-stream");
    }
    let resp = attach_auth(req, provider)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, anthropic_delta_text, on_chunk).await;
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
    let text = body
        .content
        .into_iter()
        .filter(|block| block.block_type == "text")
        .map(|block| block.text)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(text)
}

async fn send_gemini_like(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) -> Result<String, ChatError> {
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
            format!("{base}/v1beta/models/{}:{method}?alt=sse&key={key}", call.model.id)
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
    if call.enable_reasoning && call.model.reasoning {
        if is_gemini_3(&call.model.id) {
            let level = call.effort.map(gemini_thinking_level).unwrap_or("high");
            generation["thinkingConfig"] = json!({ "thinkingLevel": level });
        } else {
            generation["thinkingConfig"] = json!({ "thinkingBudget": -1 });
        }
    }
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": call.message }],
        }],
        "generationConfig": generation,
    });
    let mut req = client.post(&url).json(&body);
    if stream {
        req = req.header(reqwest::header::ACCEPT, "text/event-stream");
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
    if stream {
        return collect_stream(resp, gemini_delta_text, on_chunk).await;
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
    let text = body
        .candidates
        .into_iter()
        .next()
        .and_then(|candidate| candidate.content)
        .map(|content| {
            content
                .parts
                .into_iter()
                .filter(|part| !part.thought)
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join("")
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(ChatError::EmptyResponse)?;
    Ok(text)
}

async fn send_message(
    provider: &Provider,
    call: &ChatCall<'_>,
    on_chunk: Option<&tauri::ipc::Channel<String>>,
) -> Result<String, ChatError> {
    if call.message.trim().is_empty() {
        return Err(ChatError::EmptyMessage);
    }
    match provider.kind {
        ProviderKind::OpenAiLike => send_openai_like(provider, call, on_chunk).await,
        ProviderKind::AnthropicLike => send_anthropic_like(provider, call, on_chunk).await,
        ProviderKind::GeminiLike => send_gemini_like(provider, call, on_chunk).await,
    }
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
    let call = ChatCall {
        model: &model,
        message: prompt.trim(),
        effort: None,
        max_output: capped_output(&model, TITLE_MAX_OUTPUT),
        enable_reasoning: false,
    };
    let title = normalize_generated_title(&send_message(&provider, &call, None).await?);
    if title.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(GenerateSessionTitleResult { title })
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
}

impl OpenAiChatMessage {
    fn visible_text(self) -> Option<String> {
        let text = self.content.and_then(|content| content.into_text())?;
        let cleaned = strip_think(&text);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
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
}

#[tauri::command]
pub async fn send_chat_message(
    app: AppHandle,
    input: SendChatInput,
    on_chunk: tauri::ipc::Channel<String>,
) -> Result<SendChatResult, ChatError> {
    let (provider, model) = load_provider_model(&app, &input.provider_id, &input.model_id).await?;
    let message = input.message.trim();
    if message.is_empty() {
        return Err(ChatError::EmptyMessage);
    }
    let effort = resolve_effort(&model, input.effort.as_deref());
    let enable_reasoning = is_effort_model(&model) || is_minimax(&provider);
    let call = ChatCall {
        model: &model,
        message,
        effort: effort.as_deref(),
        max_output: output_tokens(&model),
        enable_reasoning,
    };
    log_chat_config(&provider, &input, &call);
    let content = send_message(&provider, &call, Some(&on_chunk)).await?;
    Ok(SendChatResult { content })
}
