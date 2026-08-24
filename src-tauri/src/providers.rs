use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

const CONFIG_DIR: &str = ".k-agent";
const PROVIDERS_FILE: &str = "providers.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderKind {
    #[serde(rename = "openai-like")]
    OpenAiLike,
    #[serde(rename = "anthropic-like")]
    AnthropicLike,
    #[serde(rename = "gemini-like")]
    GeminiLike,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub multimodal: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Error, Serialize)]
pub enum ProviderError {
    #[error("path resolution failed: {0}")]
    Path(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("provider with id {0} not found")]
    NotFound(String),
    #[error("api responded with status {status}: {body}")]
    ApiStatus { status: u16, body: String },
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiModel {
    id: String,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    multimodal: bool,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiSingleModelResponse {
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    context_window: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    max_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    display_name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    family: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    multimodal: bool,
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    family: Option<String>,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    input_token_limit: Option<u64>,
    #[serde(default)]
    output_token_limit: Option<u64>,
    #[serde(default)]
    supported_generation_methods: Option<Vec<String>>,
}

fn providers_path(app: &AppHandle) -> Result<PathBuf, ProviderError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| ProviderError::Path(e.to_string()))?;
    Ok(home.join(CONFIG_DIR).join(PROVIDERS_FILE))
}

async fn load(path: &Path) -> Result<Vec<Provider>, ProviderError> {
    match tokio::fs::read_to_string(path).await {
        Ok(data) if data.trim().is_empty() => Ok(Vec::new()),
        Ok(data) => serde_json::from_str(&data).map_err(|e| ProviderError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(ProviderError::Io(e.to_string())),
    }
}

async fn save(path: &Path, providers: &[Provider]) -> Result<(), ProviderError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ProviderError::Io(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(providers)
        .map_err(|e| ProviderError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| ProviderError::Io(e.to_string()))
}

fn http_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ProviderError::Http(e.to_string()))
}

fn attach_auth(req: reqwest::RequestBuilder, provider: &Provider) -> reqwest::RequestBuilder {
    match provider.kind {
        ProviderKind::OpenAiLike => {
            if let Some(key) = provider.api_key.as_deref() {
                req.bearer_auth(key)
            } else {
                req
            }
        }
        ProviderKind::AnthropicLike => {
            if let Some(key) = provider.api_key.as_deref() {
                req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            } else {
                req
            }
        }
        ProviderKind::GeminiLike => req,
    }
}

async fn fetch_models(provider: &Provider) -> Result<Vec<String>, ProviderError> {
    let client = http_client()?;
    let base = provider.base_url.trim_end_matches('/').to_string();

    match provider.kind {
        ProviderKind::OpenAiLike => {
            let url = format!("{base}/models");
            let req = client.get(&url);
            let resp = attach_auth(req, provider)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ProviderError::ApiStatus {
                    status: status.as_u16(),
                    body,
                });
            }
            let body: OpenAiModelsResponse = resp
                .json()
                .await
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            Ok(body.data.into_iter().map(|m| m.id).collect())
        }
        ProviderKind::AnthropicLike => {
            let url = format!("{base}/v1/models");
            let req = client.get(&url);
            let resp = attach_auth(req, provider)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ProviderError::ApiStatus {
                    status: status.as_u16(),
                    body,
                });
            }
            let body: AnthropicModelsResponse = resp
                .json()
                .await
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            Ok(body.data.into_iter().map(|m| m.id).collect())
        }
        ProviderKind::GeminiLike => {
            let url = match provider.api_key.as_deref() {
                Some(key) => format!("{base}/v1beta/models?key={key}"),
                None => format!("{base}/v1beta/models"),
            };
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ProviderError::ApiStatus {
                    status: status.as_u16(),
                    body,
                });
            }
            let body: GeminiModelsResponse = resp
                .json()
                .await
                .map_err(|e| ProviderError::Parse(e.to_string()))?;
            Ok(body
                .models
                .into_iter()
                .map(|m| m.name.strip_prefix("models/").unwrap_or(&m.name).to_string())
                .collect())
        }
    }
}

async fn fetch_model_details(provider: &Provider, model_id: &str) -> ModelInfo {
    let client = match http_client() {
        Ok(c) => c,
        Err(_) => return ModelInfo { id: model_id.to_string(), context_window: None, max_output_tokens: None, display_name: None, family: None, multimodal: false },
    };
    let base = provider.base_url.trim_end_matches('/').to_string();

    match provider.kind {
        ProviderKind::OpenAiLike => {
            let url = format!("{base}/models/{model_id}");
            let resp = attach_auth(client.get(&url), provider).send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(detail) = resp.json::<OpenAiSingleModelResponse>().await {
                        return ModelInfo {
                            id: detail.id,
                            context_window: detail.context_window.or(detail.max_tokens),
                            max_output_tokens: None,
                            display_name: detail.display_name,
                            family: detail.family,
                            multimodal: detail.multimodal,
                        };
                    }
                } else if resp.status().as_u16() == 404 {
                    let _ = resp.bytes().await;
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo {
                id: model_id.to_string(),
                context_window: None,
                max_output_tokens: None,
                display_name: None,
                family: None,
                multimodal: false,
            }
        }
        ProviderKind::AnthropicLike => {
            let url = format!("{base}/v1/models/{model_id}");
            let resp = attach_auth(client.get(&url), provider).send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(detail) = resp.json::<AnthropicModel>().await {
                        return ModelInfo {
                            id: detail.id,
                            context_window: detail.context_window,
                            max_output_tokens: detail.max_tokens,
                            display_name: detail.display_name,
                            family: detail.family,
                            multimodal: false,
                        };
                    }
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo {
                id: model_id.to_string(),
                context_window: None,
                max_output_tokens: None,
                display_name: None,
                family: None,
                multimodal: false,
            }
        }
        ProviderKind::GeminiLike => {
            let url = match provider.api_key.as_deref() {
                Some(key) => format!("{base}/v1beta/models/{model_id}?key={key}"),
                None => format!("{base}/v1beta/models/{model_id}"),
            };
            let resp = client.get(&url).send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(detail) = resp.json::<GeminiModel>().await {
                        let multimodal = detail
                            .supported_generation_methods
                            .as_ref()
                            .map(|methods| methods.iter().any(|m| m.contains("image") || m.contains("vision")))
                            .unwrap_or(false);
                        return ModelInfo {
                            id: model_id.to_string(),
                            context_window: detail.input_token_limit,
                            max_output_tokens: detail.output_token_limit,
                            display_name: detail.display_name,
                            family: None,
                            multimodal,
                        };
                    }
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo {
                id: model_id.to_string(),
                context_window: None,
                max_output_tokens: None,
                display_name: None,
                family: None,
                multimodal: false,
            }
        }
    }
}

async fn fetch_all_model_details(
    provider: &Provider,
    model_ids: &[String],
) -> Vec<ModelInfo> {
    let mut details = Vec::with_capacity(model_ids.len());
    for id in model_ids {
        details.push(fetch_model_details(provider, id).await);
    }
    details
}

#[tauri::command]
pub async fn list_providers(app: AppHandle) -> Result<Vec<Provider>, ProviderError> {
    let path = providers_path(&app)?;
    load(&path).await
}

#[tauri::command]
pub async fn save_provider(
    app: AppHandle,
    input: SaveProviderInput,
) -> Result<Provider, ProviderError> {
    let path = providers_path(&app)?;
    let mut providers = load(&path).await?;

    let id = input
        .id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let existing_idx = providers.iter().position(|p| p.id == id);
    let now = Utc::now().timestamp();

    let mut stored = Provider {
        id: id.clone(),
        name: input.name.trim().to_string(),
        kind: input.kind,
        base_url: input.base_url.trim().to_string(),
        api_key: input.api_key.and_then(|k| {
            let trimmed = k.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }),
        models: Vec::new(),
        last_synced_at: None,
    };

    match fetch_models(&stored).await {
        Ok(model_ids) => {
            stored.models = fetch_all_model_details(&stored, &model_ids).await;
            stored.last_synced_at = Some(now);
        }
        Err(_) => {
            stored.models = Vec::new();
            stored.last_synced_at = None;
        }
    }

    if let Some(idx) = existing_idx {
        providers[idx] = stored.clone();
    } else {
        providers.push(stored.clone());
    }
    save(&path, &providers).await?;
    Ok(stored)
}

#[tauri::command]
pub async fn delete_provider(app: AppHandle, id: String) -> Result<(), ProviderError> {
    let path = providers_path(&app)?;
    let mut providers = load(&path).await?;
    let before = providers.len();
    providers.retain(|p| p.id != id);
    if providers.len() == before {
        return Err(ProviderError::NotFound(id));
    }
    save(&path, &providers).await
}

#[tauri::command]
pub async fn refresh_provider_models(
    app: AppHandle,
    id: String,
) -> Result<Provider, ProviderError> {
    let path = providers_path(&app)?;
    let mut providers = load(&path).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| ProviderError::NotFound(id.clone()))?;

    let model_ids = fetch_models(provider).await?;
    let details = fetch_all_model_details(provider, &model_ids).await;
    provider.models = details;
    provider.last_synced_at = Some(Utc::now().timestamp());

    let updated = provider.clone();
    save(&path, &providers).await?;
    Ok(updated)
}

#[tauri::command]
pub async fn refresh_single_model(
    app: AppHandle,
    id: String,
    model_id: String,
) -> Result<Provider, ProviderError> {
    let path = providers_path(&app)?;
    let mut providers = load(&path).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| ProviderError::NotFound(id.clone()))?;

    let detail = fetch_model_details(provider, &model_id).await;
    if let Some(existing) = provider.models.iter_mut().find(|m| m.id == model_id) {
        *existing = detail;
    } else {
        provider.models.push(detail);
    }

    let updated = provider.clone();
    save(&path, &providers).await?;
    Ok(updated)
}
