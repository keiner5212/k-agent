use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::providers::ModelInfo;

const CONFIG_DIR: &str = ".k-agent";
const CACHE_FILE: &str = "models-dev-cache.json";
const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const CACHE_TTL_SECS: i64 = 86_400;
const FETCH_TIMEOUT: Duration = Duration::from_secs(12);
const BUNDLED: &str = include_str!("../catalog/models.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub multimodal: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Catalog {
    by_id: HashMap<String, CatalogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogCache {
    fetched_at: i64,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevModel {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    limit: Option<ModelsDevLimit>,
    #[serde(default)]
    modalities: Option<ModelsDevModalities>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevLimit {
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevModalities {
    #[serde(default)]
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevProvider {
    #[serde(default)]
    models: HashMap<String, ModelsDevModel>,
}

impl Catalog {
    fn insert(&mut self, entry: CatalogEntry) {
        let mut keys = Vec::with_capacity(1 + entry.aliases.len());
        keys.push(normalize_id(&entry.id));
        for alias in &entry.aliases {
            keys.push(normalize_id(alias));
        }
        for key in keys {
            self.by_id.insert(key, entry.clone());
        }
    }

    fn extend(&mut self, entries: Vec<CatalogEntry>) {
        for entry in entries {
            self.insert(entry);
        }
    }

    pub fn lookup(&self, model_id: &str) -> Option<&CatalogEntry> {
        let normalized = normalize_id(model_id);
        if let Some(entry) = self.by_id.get(&normalized) {
            return Some(entry);
        }
        for suffix in ["-highspeed", "-highspeed"] {
            if let Some(base) = normalized.strip_suffix(suffix) {
                if let Some(entry) = self.by_id.get(base) {
                    return Some(entry);
                }
            }
        }
        None
    }

    pub fn apply(&self, model: &mut ModelInfo) {
        if model.user_edited {
            return;
        }
        let Some(entry) = self.lookup(&model.id) else {
            if model.family.is_none() {
                model.family = Some(id_family(&model.id));
            }
            return;
        };
        if model.context_window.is_none() {
            model.context_window = entry.context_window;
        }
        if model.max_output_tokens.is_none() {
            model.max_output_tokens = entry.max_output_tokens;
        }
        if model.display_name.is_none() {
            model.display_name = entry.display_name.clone();
        }
        if model.family.is_none() {
            model.family = Some(id_family(&model.id));
        }
        if !model.multimodal {
            model.multimodal = entry.multimodal;
        }
    }
}

pub async fn load(app: &AppHandle) -> Catalog {
    let mut catalog = Catalog::default();
    if let Ok(entries) = load_remote_or_cache(app).await {
        catalog.extend(entries);
    }
    catalog.extend(bundled_entries());
    catalog
}

fn bundled_entries() -> Vec<CatalogEntry> {
    serde_json::from_str(BUNDLED).unwrap_or_default()
}

fn cache_path(app: &AppHandle) -> Option<PathBuf> {
    let home = app.path().home_dir().ok()?;
    Some(home.join(CONFIG_DIR).join(CACHE_FILE))
}

async fn load_remote_or_cache(app: &AppHandle) -> Result<Vec<CatalogEntry>, String> {
    let path = cache_path(app).ok_or_else(|| "catalog cache path unavailable".to_string())?;
    let cached = read_cache(&path).await.ok();
    let now = chrono::Utc::now().timestamp();
    let fresh = cached
        .as_ref()
        .map(|cache| now.saturating_sub(cache.fetched_at) < CACHE_TTL_SECS)
        .unwrap_or(false);
    if fresh {
        return Ok(cached.unwrap().entries);
    }
    match fetch_models_dev().await {
        Ok(entries) => {
            let cache = CatalogCache {
                fetched_at: now,
                entries: entries.clone(),
            };
            if let Ok(json) = serde_json::to_string(&cache) {
                if let Some(parent) = path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::write(path, json).await;
            }
            Ok(entries)
        }
        Err(error) => cached
            .map(|cache| cache.entries)
            .ok_or(error),
    }
}

async fn read_cache(path: &PathBuf) -> Result<CatalogCache, String> {
    let data = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

async fn fetch_models_dev() -> Result<Vec<CatalogEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(MODELS_DEV_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("models.dev status {}", resp.status()));
    }
    let providers: HashMap<String, ModelsDevProvider> =
        resp.json().await.map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for provider in providers.into_values() {
        for (key, model) in provider.models {
            let id = model.id.unwrap_or(key);
            let multimodal = model
                .modalities
                .as_ref()
                .map(|mods| {
                    mods.input.iter().any(|item| {
                        matches!(item.as_str(), "image" | "audio" | "video" | "pdf")
                    })
                })
                .unwrap_or(false);
            entries.push(CatalogEntry {
                id,
                display_name: model.name,
                context_window: model.limit.as_ref().and_then(|limit| limit.context),
                max_output_tokens: model.limit.as_ref().and_then(|limit| limit.output),
                multimodal,
                aliases: Vec::new(),
            });
        }
    }
    Ok(entries)
}

fn normalize_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn id_family(id: &str) -> String {
    let lower = id.to_ascii_lowercase();
    for suffix in ["-highspeed", "-highspeed"] {
        if lower.ends_with(suffix) {
            return id[..id.len() - suffix.len()].to_string();
        }
    }
    id.to_string()
}
