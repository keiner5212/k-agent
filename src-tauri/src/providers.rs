use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

use crate::APP_CONFIG_DIR;

const PROVIDERS_FILE: &str = "providers.json";
const PROVIDER_KEYS_FILE: &str = "provider-keys.json";
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ModelSource {
    #[default]
    Detected,
    Custom,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reasoning: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tool_call: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub structured_output: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub attachment: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub multimodal: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effort_levels: Vec<String>,
    #[serde(default, skip_serializing_if = "is_detected")]
    pub source: ModelSource,
    #[serde(default, skip_serializing_if = "is_false")]
    pub user_edited: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub favorite: bool,
}

impl ModelInfo {
    pub fn detected(id: &str) -> Self {
        Self {
            id: id.to_string(),
            context_window: None,
            max_output_tokens: None,
            display_name: None,
            family: None,
            knowledge: None,
            input: Vec::new(),
            output: Vec::new(),
            reasoning: false,
            tool_call: false,
            structured_output: false,
            attachment: false,
            multimodal: false,
            effort_levels: Vec::new(),
            source: ModelSource::Detected,
            user_edited: false,
            favorite: false,
        }
    }

    pub fn sync_multimodal(&mut self) {
        self.multimodal |= self.input.iter().any(|item| {
            matches!(
                item.to_ascii_lowercase().as_str(),
                "image" | "audio" | "video" | "pdf"
            )
        });
    }
}

fn keep_local(model: &ModelInfo) -> bool {
    model.user_edited || model.source == ModelSource::Custom
}

fn is_detected(value: &ModelSource) -> bool {
    matches!(value, ModelSource::Detected)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn deserialize_models<'de, D>(deserializer: D) -> Result<Vec<ModelInfo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|value| {
            if let Some(id) = value.as_str() {
                return Ok(ModelInfo::detected(id));
            }
            serde_json::from_value(value).map_err(serde::de::Error::custom)
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertModelInput {
    pub provider_id: String,
    #[serde(default)]
    pub original_id: Option<String>,
    pub id: String,
    pub display_name: Option<String>,
    pub family: Option<String>,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub knowledge: Option<String>,
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub structured_output: bool,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default)]
    pub multimodal: bool,
    #[serde(default)]
    pub effort_levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_api_key: bool,
    #[serde(default, deserialize_with = "deserialize_models")]
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
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub worker_cores: Option<u32>,
}

#[derive(Debug, Error)]
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
    #[error("model id already exists: {0}")]
    Duplicate(String),
    #[error("api responded with status {status}: {body}")]
    ApiStatus { status: u16, body: String },
    #[error("{0}")]
    Crypto(String),
}

impl Serialize for ProviderError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

impl From<crate::secret::SecretError> for ProviderError {
    fn from(error: crate::secret::SecretError) -> Self {
        Self::Crypto(error.to_string())
    }
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
    Ok(home.join(APP_CONFIG_DIR).join(PROVIDERS_FILE))
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
    let json =
        serde_json::to_string_pretty(providers).map_err(|e| ProviderError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| ProviderError::Io(e.to_string()))?;
    Ok(())
}

async fn load_keys(path: &Path) -> Result<HashMap<String, String>, ProviderError> {
    match tokio::fs::read_to_string(path).await {
        Ok(data) if data.trim().is_empty() => Ok(HashMap::new()),
        Ok(data) => serde_json::from_str(&data).map_err(|e| ProviderError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(ProviderError::Io(e.to_string())),
    }
}

async fn save_keys(path: &Path, keys: &HashMap<String, String>) -> Result<(), ProviderError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ProviderError::Io(e.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(keys).map_err(|e| ProviderError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| ProviderError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn secrets_dir(app: &AppHandle) -> Result<PathBuf, ProviderError> {
    app.path()
        .app_data_dir()
        .map_err(|e| ProviderError::Path(e.to_string()))
}

fn keys_path(app: &AppHandle) -> Result<PathBuf, ProviderError> {
    Ok(secrets_dir(app)?.join(PROVIDER_KEYS_FILE))
}

fn redact(mut provider: Provider) -> Provider {
    provider.has_api_key = provider
        .api_key
        .as_ref()
        .is_some_and(|key| !key.trim().is_empty());
    provider.api_key = None;
    provider
}

pub(crate) async fn load_all(app: &AppHandle) -> Result<Vec<Provider>, ProviderError> {
    let path = providers_path(app)?;
    let mut providers = load(&path).await?;
    let dir = secrets_dir(app)?;
    let key_blobs = load_keys(&keys_path(app)?).await?;
    let cipher = crate::secret::cipher(&dir)?;
    for provider in &mut providers {
        provider.api_key =
            crate::secret::open(cipher, key_blobs.get(&provider.id).map(String::as_str))?;
    }
    Ok(providers)
}

async fn save_all(app: &AppHandle, providers: &[Provider]) -> Result<(), ProviderError> {
    let path = providers_path(app)?;
    let dir = secrets_dir(app)?;
    let cipher = crate::secret::cipher(&dir)?;
    let mut stored = providers.to_vec();
    let mut keys = HashMap::new();
    for provider in &mut stored {
        provider.has_api_key = false;
        let sealed = match provider
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
        {
            None => None,
            Some(plain) => Some(crate::secret::seal(cipher, plain)?),
        };
        if let Some(blob) = sealed {
            keys.insert(provider.id.clone(), blob);
        }
        provider.api_key = None;
    }
    save_keys(&keys_path(app)?, &keys).await?;
    save(&path, &stored).await
}

pub(crate) fn http_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ProviderError::Http(e.to_string()))
}

pub(crate) fn attach_auth(req: reqwest::RequestBuilder, provider: &Provider) -> reqwest::RequestBuilder {
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
                .map(|m| {
                    m.name
                        .strip_prefix("models/")
                        .unwrap_or(&m.name)
                        .to_string()
                })
                .collect())
        }
    }
}

async fn fetch_model_details(provider: &Provider, model_id: &str) -> ModelInfo {
    let client = match http_client() {
        Ok(c) => c,
        Err(_) => return ModelInfo::detected(model_id),
    };
    let base = provider.base_url.trim_end_matches('/').to_string();

    match provider.kind {
        ProviderKind::OpenAiLike => {
            let url = format!("{base}/models/{model_id}");
            let resp = attach_auth(client.get(&url), provider).send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(detail) = resp.json::<OpenAiSingleModelResponse>().await {
                        let mut info = ModelInfo::detected(&detail.id);
                        info.context_window = detail.context_window.or(detail.max_tokens);
                        info.display_name = detail.display_name;
                        info.family = detail.family;
                        info.multimodal = detail.multimodal;
                        return info;
                    }
                } else if resp.status().as_u16() == 404 {
                    let _ = resp.bytes().await;
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo::detected(model_id)
        }
        ProviderKind::AnthropicLike => {
            let url = format!("{base}/v1/models/{model_id}");
            let resp = attach_auth(client.get(&url), provider).send().await;
            if let Ok(resp) = resp {
                if resp.status().is_success() {
                    if let Ok(detail) = resp.json::<AnthropicModel>().await {
                        let mut info = ModelInfo::detected(&detail.id);
                        info.context_window = detail.context_window;
                        info.max_output_tokens = detail.max_tokens;
                        info.display_name = detail.display_name;
                        info.family = detail.family;
                        return info;
                    }
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo::detected(model_id)
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
                            .map(|methods| {
                                methods
                                    .iter()
                                    .any(|m| m.contains("image") || m.contains("vision"))
                            })
                            .unwrap_or(false);
                        let mut info = ModelInfo::detected(model_id);
                        info.context_window = detail.input_token_limit;
                        info.max_output_tokens = detail.output_token_limit;
                        info.display_name = detail.display_name;
                        info.multimodal = multimodal;
                        return info;
                    }
                } else {
                    let _ = resp.bytes().await;
                }
            }
            ModelInfo::detected(model_id)
        }
    }
}

fn detail_concurrency(worker_cores: Option<u32>) -> usize {
    (worker_cores.unwrap_or(1) as usize).clamp(1, 64)
}

async fn fetch_all_model_details(
    provider: &Provider,
    model_ids: &[String],
    worker_cores: usize,
) -> Vec<ModelInfo> {
    let mut details = Vec::with_capacity(model_ids.len());
    let concurrency = worker_cores.max(1);
    for chunk in model_ids.chunks(concurrency) {
        let mut handles = Vec::with_capacity(chunk.len());
        for id in chunk {
            let provider = provider.clone();
            let model_id = id.clone();
            handles.push(tokio::spawn(async move {
                fetch_model_details(&provider, &model_id).await
            }));
        }
        for (id, handle) in chunk.iter().zip(handles) {
            match handle.await {
                Ok(info) => details.push(info),
                Err(_) => details.push(ModelInfo::detected(id)),
            }
        }
    }
    details
}

fn merge_models(previous: Vec<ModelInfo>, fetched: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut previous_by_id: HashMap<String, ModelInfo> = previous
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect();
    let mut merged = Vec::with_capacity(fetched.len());
    for mut model in fetched {
        if let Some(old) = previous_by_id.remove(&model.id) {
            if keep_local(&old) {
                merged.push(old);
                continue;
            }
            model.favorite = old.favorite;
        }
        merged.push(model);
    }
    for leftover in previous_by_id.into_values() {
        if keep_local(&leftover) {
            merged.push(leftover);
        }
    }
    merged
}

async fn enrich_models(
    app: &AppHandle,
    provider: &Provider,
    model_ids: &[String],
    previous: Vec<ModelInfo>,
    worker_cores: usize,
) -> Vec<ModelInfo> {
    let catalog = crate::catalog::load(app).await;
    let mut by_id: HashMap<String, ModelInfo> = HashMap::new();
    let mut missing = Vec::new();
    for id in model_ids {
        if catalog.lookup(id).is_some() {
            let mut model = ModelInfo::detected(id);
            catalog.apply(&mut model);
            by_id.insert(id.clone(), model);
        } else {
            missing.push(id.clone());
        }
    }
    if !missing.is_empty() {
        for mut model in fetch_all_model_details(provider, &missing, worker_cores).await {
            catalog.apply(&mut model);
            by_id.insert(model.id.clone(), model);
        }
    }
    let fetched = model_ids.iter().filter_map(|id| by_id.remove(id)).collect();
    merge_models(previous, fetched)
}

#[tauri::command]
pub async fn list_providers(app: AppHandle) -> Result<Vec<Provider>, ProviderError> {
    let mut providers = load_all(&app).await?;
    let catalog = crate::catalog::load(&app).await;
    for provider in &mut providers {
        for model in &mut provider.models {
            catalog.apply(model);
        }
    }
    Ok(providers.into_iter().map(redact).collect())
}

#[tauri::command]
pub async fn save_provider(
    app: AppHandle,
    input: SaveProviderInput,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;

    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let existing_idx = providers.iter().position(|p| p.id == id);
    let now = Utc::now().timestamp();

    let previous_models = existing_idx
        .and_then(|idx| providers.get(idx).map(|provider| provider.models.clone()))
        .unwrap_or_default();
    let previous_key = existing_idx.and_then(|idx| providers[idx].api_key.clone());
    let api_key = if input.clear_api_key {
        None
    } else if let Some(key) = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        Some(key.to_string())
    } else {
        previous_key
    };

    let mut stored = Provider {
        id: id.clone(),
        name: input.name.trim().to_string(),
        kind: input.kind,
        base_url: input.base_url.trim().to_string(),
        api_key,
        has_api_key: false,
        models: previous_models.clone(),
        last_synced_at: None,
    };

    match fetch_models(&stored).await {
        Ok(model_ids) => {
            stored.models = enrich_models(
                &app,
                &stored,
                &model_ids,
                previous_models,
                detail_concurrency(input.worker_cores),
            )
            .await;
            stored.last_synced_at = Some(now);
        }
        Err(_) => {
            stored.models = previous_models;
            stored.last_synced_at = None;
        }
    }

    if let Some(idx) = existing_idx {
        providers[idx] = stored.clone();
    } else {
        providers.push(stored.clone());
    }
    save_all(&app, &providers).await?;
    Ok(redact(stored))
}

#[tauri::command]
pub async fn delete_provider(app: AppHandle, id: String) -> Result<(), ProviderError> {
    let mut providers = load_all(&app).await?;
    let before = providers.len();
    providers.retain(|p| p.id != id);
    if providers.len() == before {
        return Err(ProviderError::NotFound(id));
    }
    save_all(&app, &providers).await
}

#[tauri::command]
pub async fn refresh_provider_models(
    app: AppHandle,
    id: String,
    worker_cores: Option<u32>,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| ProviderError::NotFound(id.clone()))?;

    let snapshot = provider.clone();
    let previous = snapshot.models.clone();
    let model_ids = fetch_models(&snapshot).await?;
    let models = enrich_models(
        &app,
        &snapshot,
        &model_ids,
        previous,
        detail_concurrency(worker_cores),
    )
    .await;
    provider.models = models;
    provider.last_synced_at = Some(Utc::now().timestamp());

    let updated = provider.clone();
    save_all(&app, &providers).await?;
    Ok(redact(updated))
}

#[tauri::command]
pub async fn upsert_provider_model(
    app: AppHandle,
    input: UpsertModelInput,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == input.provider_id)
        .ok_or_else(|| ProviderError::NotFound(input.provider_id.clone()))?;

    let id = input.id.trim().to_string();
    if id.is_empty() {
        return Err(ProviderError::Parse("model id is required".to_string()));
    }

    let original_id = input
        .original_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.clone());

    if provider
        .models
        .iter()
        .any(|model| model.id == id && model.id != original_id)
    {
        return Err(ProviderError::Duplicate(id));
    }

    let family = input
        .family
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let display_name = input
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let favorite = provider
        .models
        .iter()
        .find(|model| model.id == original_id)
        .map(|model| model.favorite)
        .unwrap_or(false);

    let mut next = provider
        .models
        .iter()
        .find(|model| model.id == original_id)
        .cloned()
        .unwrap_or_else(|| ModelInfo::detected(&id));
    next.id = id;
    next.context_window = input.context_window.filter(|value| *value > 0);
    next.max_output_tokens = input.max_output_tokens.filter(|value| *value > 0);
    next.display_name = display_name;
    next.family = family;
    next.multimodal = input.multimodal;
    next.source = ModelSource::Custom;
    next.user_edited = true;
    next.favorite = favorite;
    if let Some(knowledge) = input
        .knowledge
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        next.knowledge = Some(knowledge.to_string());
    }
    if !input.input.is_empty() {
        next.input = input.input;
    }
    if !input.output.is_empty() {
        next.output = input.output;
    }
    if !input.effort_levels.is_empty() {
        next.effort_levels = input.effort_levels;
    }
    next.reasoning |= input.reasoning;
    next.tool_call |= input.tool_call;
    next.structured_output |= input.structured_output;
    next.attachment |= input.attachment;
    next.sync_multimodal();

    if let Some(existing) = provider
        .models
        .iter_mut()
        .find(|model| model.id == original_id)
    {
        *existing = next;
    } else {
        provider.models.push(next);
    }

    let updated = provider.clone();
    save_all(&app, &providers).await?;
    Ok(redact(updated))
}

#[tauri::command]
pub async fn delete_provider_model(
    app: AppHandle,
    id: String,
    model_id: String,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| ProviderError::NotFound(id.clone()))?;

    let before = provider.models.len();
    provider.models.retain(|model| model.id != model_id);
    if provider.models.len() == before {
        return Err(ProviderError::NotFound(model_id));
    }

    let updated = provider.clone();
    save_all(&app, &providers).await?;
    Ok(redact(updated))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSingleModelInput {
    pub id: String,
    pub model_id: String,
}

#[tauri::command]
pub async fn refresh_single_model(
    app: AppHandle,
    input: RefreshSingleModelInput,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == input.id)
        .ok_or_else(|| ProviderError::NotFound(input.id.clone()))?;

    let favorite = provider
        .models
        .iter()
        .find(|model| model.id == input.model_id)
        .map(|model| {
            if model.user_edited {
                None
            } else {
                Some(model.favorite)
            }
        });
    if matches!(favorite, Some(None)) {
        return Ok(redact(provider.clone()));
    }
    let favorite = favorite.flatten().unwrap_or(false);

    let mut detail = fetch_model_details(provider, &input.model_id).await;
    crate::catalog::load(&app).await.apply(&mut detail);
    detail.favorite = favorite;
    if let Some(existing) = provider.models.iter_mut().find(|m| m.id == input.model_id) {
        *existing = detail;
    } else {
        provider.models.push(detail);
    }

    let updated = provider.clone();
    save_all(&app, &providers).await?;
    Ok(redact(updated))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFavoriteInput {
    pub id: String,
    pub model_id: String,
    pub favorite: bool,
}

#[tauri::command]
pub async fn set_model_favorite(
    app: AppHandle,
    input: SetFavoriteInput,
) -> Result<Provider, ProviderError> {
    let mut providers = load_all(&app).await?;
    let provider = providers
        .iter_mut()
        .find(|p| p.id == input.id)
        .ok_or_else(|| ProviderError::NotFound(input.id.clone()))?;

    let model = provider
        .models
        .iter_mut()
        .find(|model| model.id == input.model_id)
        .ok_or_else(|| ProviderError::NotFound(input.model_id))?;
    model.favorite = input.favorite;

    let updated = provider.clone();
    save_all(&app, &providers).await?;
    Ok(redact(updated))
}
