use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::providers::{ModelCost, ModelInfo};
use crate::APP_CONFIG_DIR;

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
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<ModelCost>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
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
    family: Option<String>,
    #[serde(default)]
    knowledge: Option<String>,
    #[serde(default)]
    limit: Option<ModelsDevLimit>,
    #[serde(default)]
    modalities: Option<ModelsDevModalities>,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    structured_output: bool,
    #[serde(default)]
    attachment: bool,
    #[serde(default)]
    reasoning_options: Vec<ModelsDevReasoningOption>,
    #[serde(default)]
    cost: Option<ModelsDevCost>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevCost {
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
    #[serde(default)]
    reasoning: Option<f64>,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevLimit {
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
    #[serde(default)]
    input: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevModalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevReasoningOption {
    #[serde(default, rename = "type")]
    option_type: Option<String>,
    #[serde(default)]
    values: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevProvider {
    #[serde(default)]
    models: HashMap<String, ModelsDevModel>,
}

impl Catalog {
    fn insert(&mut self, mut entry: CatalogEntry) {
        let mut keys = Vec::with_capacity(1 + entry.aliases.len());
        keys.push(normalize_id(&entry.id));
        for alias in &entry.aliases {
            keys.push(normalize_id(alias));
        }
        if entry.cost.is_none() {
            for key in &keys {
                if let Some(current) = self.by_id.get(key) {
                    if current.cost.is_some() {
                        entry.cost = current.cost.clone();
                        break;
                    }
                }
            }
        }
        for key in keys {
            self.by_id.insert(key, entry.clone());
        }
    }

    fn extend(&mut self, entries: impl IntoIterator<Item = CatalogEntry>) {
        for entry in entries {
            self.insert(entry);
        }
    }

    pub fn lookup(&self, model_id: &str) -> Option<&CatalogEntry> {
        let normalized = normalize_id(model_id);
        if let Some(entry) = self.by_id.get(&normalized) {
            return Some(entry);
        }
        for suffix in ["-highspeed"] {
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
            model.sync_multimodal();
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
            model.family = entry.family.clone().or_else(|| Some(id_family(&model.id)));
        }
        if model.knowledge.is_none() {
            model.knowledge = entry.knowledge.clone();
        }
        if model.input.is_empty() {
            model.input = entry.input.clone();
        }
        if model.output.is_empty() {
            model.output = entry.output.clone();
        }
        if model.effort_levels.is_empty() {
            model.effort_levels = entry.effort_levels.clone();
        }
        if model.cost.is_none() {
            model.cost = entry.cost.clone();
        }
        model.reasoning |= entry.reasoning;
        model.tool_call |= entry.tool_call;
        model.structured_output |= entry.structured_output;
        model.attachment |= entry.attachment;
        model.multimodal |= entry.multimodal;
        model.sync_multimodal();
    }
}

pub async fn load(app: &AppHandle) -> Catalog {
    let mut catalog = Catalog::default();
    if let Ok(entries) = load_remote_or_cache(app).await {
        catalog.extend(entries);
    }
    catalog.extend(bundled_entries().iter().cloned());
    catalog
}

fn bundled_entries() -> &'static [CatalogEntry] {
    static ENTRIES: OnceLock<Vec<CatalogEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| serde_json::from_str(BUNDLED).unwrap_or_default())
        .as_slice()
}

fn cache_path(app: &AppHandle) -> Option<PathBuf> {
    let home = app.path().home_dir().ok()?;
    Some(home.join(APP_CONFIG_DIR).join(CACHE_FILE))
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
        Err(error) => cached.map(|cache| cache.entries).ok_or(error),
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
    let mut by_id: HashMap<String, CatalogEntry> = HashMap::new();
    for provider in providers.into_values() {
        for (key, model) in provider.models {
            let entry = catalog_from_models_dev(key, model);
            let key = normalize_id(&entry.id);
            by_id
                .entry(key)
                .and_modify(|current| {
                    if richer_than(current, &entry) {
                        let mut next = entry.clone();
                        if next.cost.is_none() {
                            next.cost = current.cost.clone();
                        }
                        *current = next;
                    } else if current.cost.is_none() {
                        current.cost = entry.cost.clone();
                    }
                })
                .or_insert(entry);
        }
    }
    Ok(by_id.into_values().collect())
}

fn catalog_from_models_dev(key: String, model: ModelsDevModel) -> CatalogEntry {
    let input = unique_nonempty(
        model
            .modalities
            .as_ref()
            .map(|m| m.input.clone())
            .unwrap_or_default(),
    );
    let output = unique_nonempty(
        model
            .modalities
            .as_ref()
            .map(|m| m.output.clone())
            .unwrap_or_default(),
    );
    let mut effort_levels = effort_from_options(&model.reasoning_options);
    if model.reasoning && effort_levels.is_empty() {
        effort_levels = vec!["low".into(), "medium".into(), "high".into()];
    }
    CatalogEntry {
        id: model.id.unwrap_or(key),
        display_name: model.name,
        family: empty_to_none(model.family),
        knowledge: empty_to_none(model.knowledge),
        context_window: model
            .limit
            .as_ref()
            .and_then(|limit| limit.context.or(limit.input)),
        max_output_tokens: model.limit.as_ref().and_then(|limit| limit.output),
        multimodal: is_media_input(&input),
        reasoning: model.reasoning,
        tool_call: model.tool_call,
        structured_output: model.structured_output,
        attachment: model.attachment,
        effort_levels,
        input,
        output,
        cost: model.cost.and_then(model_cost_from_dev),
        aliases: Vec::new(),
    }
}

fn model_cost_from_dev(cost: ModelsDevCost) -> Option<ModelCost> {
    let input = cost.input?;
    let output = cost.output?;
    Some(ModelCost {
        input,
        output,
        reasoning: cost.reasoning,
        cache_read: cost.cache_read,
        cache_write: cost.cache_write,
    })
}

fn effort_from_options(options: &[ModelsDevReasoningOption]) -> Vec<String> {
    let mut levels = Vec::new();
    for option in options {
        let values = unique_nonempty(
            option
                .values
                .iter()
                .filter_map(|value| {
                    value
                        .as_str()
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                })
                .map(str::to_string),
        );
        if !values.is_empty() {
            levels.extend(values);
        } else if option.option_type.as_deref() == Some("toggle") {
            levels.extend(["off".into(), "on".into()]);
        }
    }
    unique_nonempty(levels)
}

fn unique_nonempty(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.contains_key(&key) {
            continue;
        }
        seen.insert(key, ());
        out.push(trimmed.to_string());
    }
    out
}

fn richer_than(current: &CatalogEntry, candidate: &CatalogEntry) -> bool {
    score(candidate) > score(current)
}

fn score(entry: &CatalogEntry) -> (usize, usize, u8, u8, u8, u8) {
    (
        entry.effort_levels.len(),
        entry.input.len(),
        u8::from(entry.context_window.is_some()),
        u8::from(entry.cost.is_some()),
        u8::from(entry.reasoning),
        u8::from(entry.tool_call),
    )
}

fn is_media_input(input: &[String]) -> bool {
    input.iter().any(|item| {
        matches!(
            item.to_ascii_lowercase().as_str(),
            "image" | "audio" | "video" | "pdf"
        )
    })
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn id_family(id: &str) -> String {
    let lower = id.to_ascii_lowercase();
    if let Some(stripped) = lower.strip_suffix("-highspeed") {
        return id[..stripped.len()].to_string();
    }
    id.to_string()
}
