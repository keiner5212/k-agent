use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

const CONFIG_DIR: &str = ".k-agent";
const PROVIDERS_FILE: &str = "providers.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    OpenAiLike,
    AnthropicLike,
    GeminiLike,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
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
struct OpenAiModel {
    id: String,
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
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

async fn fetch_models(provider: &Provider) -> Result<Vec<String>, ProviderError> {
    let client = http_client()?;
    let base = provider.base_url.trim_end_matches('/').to_string();

    match provider.kind {
        ProviderKind::OpenAiLike => {
            let url = format!("{base}/models");
            let mut req = client.get(&url);
            if let Some(key) = provider.api_key.as_deref() {
                req = req.bearer_auth(key);
            }
            let resp = req.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;
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
            let mut req = client.get(&url);
            if let Some(key) = provider.api_key.as_deref() {
                req = req
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01");
            }
            let resp = req.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;
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

    let stored = Provider {
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
        models: existing_idx
            .and_then(|i| providers.get(i).map(|p| p.models.clone()))
            .unwrap_or_default(),
        last_synced_at: existing_idx.and_then(|i| {
            providers.get(i).and_then(|p| p.last_synced_at)
        }),
    };

    // Probe models on first save; skip if user is just editing existing fields.
    let mut stored = stored;
    let is_new = existing_idx.is_none();
    if is_new {
        match fetch_models(&stored).await {
            Ok(models) => {
                stored.models = models;
                stored.last_synced_at = Some(now);
            }
            Err(_) => {
                // Leave models empty; user can retry via refresh.
                stored.last_synced_at = None;
            }
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

    let models = fetch_models(provider).await?;
    provider.models = models;
    provider.last_synced_at = Some(Utc::now().timestamp());

    let updated = provider.clone();
    save(&path, &providers).await?;
    Ok(updated)
}
