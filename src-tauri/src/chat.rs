use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use thiserror::Error;

use crate::providers::{attach_auth, load_all, Provider, ProviderError, ProviderKind};

const CHAT_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

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
    has_api_key: bool,
    message: String,
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

fn log_chat_config(provider: &Provider, input: &SendChatInput) {
    let config = ChatConfigLog {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        provider_kind: provider.kind,
        base_url: provider.base_url.clone(),
        model_id: input.model_id.clone(),
        effort: input.effort.clone(),
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

async fn send_openai_like(provider: &Provider, model_id: &str, message: &str) -> Result<String, ChatError> {
    let client = chat_http_client()?;
    let base = provider.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");
    let body = serde_json::json!({
        "model": model_id,
        "messages": [{ "role": "user", "content": message }],
    });
    let resp = attach_auth(client.post(&url).json(&body), provider)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
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
        .and_then(|choice| choice.message.content)
        .filter(|text| !text.is_empty())
        .ok_or(ChatError::EmptyResponse)
}

async fn send_anthropic_like(
    provider: &Provider,
    model_id: &str,
    message: &str,
) -> Result<String, ChatError> {
    let client = chat_http_client()?;
    let base = provider.base_url.trim_end_matches('/');
    let url = format!("{base}/v1/messages");
    let body = serde_json::json!({
        "model": model_id,
        "max_tokens": 4096,
        "messages": [{ "role": "user", "content": message }],
    });
    let resp = attach_auth(client.post(&url).json(&body), provider)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
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
    body.content
        .into_iter()
        .find_map(|block| {
            if block.block_type == "text" {
                Some(block.text)
            } else {
                None
            }
        })
        .filter(|text| !text.is_empty())
        .ok_or(ChatError::EmptyResponse)
}

async fn send_gemini_like(
    provider: &Provider,
    model_id: &str,
    message: &str,
) -> Result<String, ChatError> {
    let client = chat_http_client()?;
    let base = provider.base_url.trim_end_matches('/');
    let url = match provider.api_key.as_deref() {
        Some(key) => format!("{base}/v1beta/models/{model_id}:generateContent?key={key}"),
        None => format!("{base}/v1beta/models/{model_id}:generateContent"),
    };
    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": message }],
        }],
    });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| ChatError::Http(e.to_string()))?;
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
    body.candidates
        .into_iter()
        .next()
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts.into_iter().next())
        .and_then(|part| part.text)
        .filter(|text| !text.is_empty())
        .ok_or(ChatError::EmptyResponse)
}

async fn send_message(provider: &Provider, model_id: &str, message: &str) -> Result<String, ChatError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(ChatError::EmptyMessage);
    }
    match provider.kind {
        ProviderKind::OpenAiLike => send_openai_like(provider, model_id, message).await,
        ProviderKind::AnthropicLike => send_anthropic_like(provider, model_id, message).await,
        ProviderKind::GeminiLike => send_gemini_like(provider, model_id, message).await,
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
    let providers = load_all(&app).await?;
    let provider = providers
        .into_iter()
        .find(|item| item.id == input.provider_id)
        .ok_or_else(|| ChatError::ProviderNotFound(input.provider_id.clone()))?;
    let prompt = format!("{TITLE_PROMPT}{}", input.message.trim());
    let title = normalize_generated_title(
        &send_message(&provider, &input.model_id, &prompt).await?,
    );
    if title.is_empty() {
        return Err(ChatError::EmptyResponse);
    }
    Ok(GenerateSessionTitleResult { title })
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
    content: Option<String>,
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
}

#[tauri::command]
pub async fn send_chat_message(app: AppHandle, input: SendChatInput) -> Result<SendChatResult, ChatError> {
    let providers = load_all(&app).await?;
    let provider = providers
        .into_iter()
        .find(|item| item.id == input.provider_id)
        .ok_or_else(|| ChatError::ProviderNotFound(input.provider_id.clone()))?;
    log_chat_config(&provider, &input);
    let content = send_message(&provider, &input.model_id, &input.message).await?;
    Ok(SendChatResult { content })
}
