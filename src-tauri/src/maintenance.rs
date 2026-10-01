use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use tauri::AppHandle;

struct Sweep {
    id: &'static str,
    run: fn(&AppHandle) -> Result<(), String>,
}

const SWEEPS: &[Sweep] = &[
    Sweep {
        id: "tool-cache",
        run: sweep_tool_cache,
    },
    Sweep {
        id: "legacy-home-cache",
        run: sweep_legacy_home_cache,
    },
    Sweep {
        id: "catalog-cache",
        run: sweep_catalog_cache,
    },
    Sweep {
        id: "provider-key-orphans",
        run: sweep_provider_key_orphans,
    },
    Sweep {
        id: "mcp-secret-orphans",
        run: sweep_mcp_secret_orphans,
    },
    Sweep {
        id: "settings-orphans",
        run: sweep_settings_orphans,
    },
];

#[tauri::command]
pub fn clear_app_cache(app: AppHandle) -> Result<(), String> {
    let mut errors = Vec::new();
    for sweep in SWEEPS {
        if let Err(error) = (sweep.run)(&app) {
            errors.push(format!("{}: {error}", sweep.id));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn sweep_tool_cache(app: &AppHandle) -> Result<(), String> {
    let dir = crate::paths::app_data_dir(app)?.join("cache");
    remove_dir(&dir)
}

fn sweep_legacy_home_cache(app: &AppHandle) -> Result<(), String> {
    let Some(home) = crate::paths::home_dir(app).ok() else {
        return Ok(());
    };
    remove_dir(&crate::paths::config_root(&home).join("cache"))
}

fn sweep_catalog_cache(app: &AppHandle) -> Result<(), String> {
    remove_file(&crate::paths::config_file(app, "models-dev-cache.json")?)
}

fn sweep_provider_key_orphans(app: &AppHandle) -> Result<(), String> {
    let ids = json_ids(&crate::paths::config_file(app, "providers.json")?)?;
    retain_secret_map(
        &crate::paths::app_data_file(app, "provider-keys.json")?,
        &ids,
    )
}

fn sweep_mcp_secret_orphans(app: &AppHandle) -> Result<(), String> {
    let ids = json_ids(&crate::paths::config_file(app, "mcp-servers.json")?)?;
    retain_secret_map(&crate::paths::app_data_file(app, "mcp-secrets.json")?, &ids)
}

fn sweep_settings_orphans(app: &AppHandle) -> Result<(), String> {
    let path = crate::paths::app_data_file(app, "settings.json")?;
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut store: Map<String, Value> =
        serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let ids = provider_ids(app);
    let agent_root = crate::paths::home_dir(app)
        .ok()
        .map(|home| crate::paths::config_root(&home).join("agents"));
    let drop_keys: Vec<String> = store
        .keys()
        .filter(|key| orphan_setting(key, &store, &ids, agent_root.as_deref()))
        .cloned()
        .collect();
    if drop_keys.is_empty() {
        return Ok(());
    }
    for key in drop_keys {
        store.remove(&key);
    }
    write_json(&path, &Value::Object(store), false)
}

fn orphan_setting(
    key: &str,
    store: &Map<String, Value>,
    provider_ids: &HashSet<String>,
    agent_root: Option<&Path>,
) -> bool {
    if let Some(rest) = key
        .strip_prefix("modelEffort:")
        .or_else(|| key.strip_prefix("modelRequest:"))
    {
        let provider_id = rest.split(':').next().unwrap_or("");
        return !provider_ids.contains(provider_id);
    }
    if key == "selectedModel" {
        let provider_id = store
            .get(key)
            .and_then(|value| value.get("providerId"))
            .and_then(Value::as_str)
            .unwrap_or("");
        return !provider_ids.contains(provider_id);
    }
    if key == "selectedAgent" {
        let Some(value) = store.get(key).and_then(Value::as_str) else {
            return false;
        };
        return orphan_agent(value, agent_root);
    }
    false
}

fn orphan_agent(value: &str, agent_root: Option<&Path>) -> bool {
    let Some(id) = value.strip_prefix("global:") else {
        return false;
    };
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return true;
    }
    let Some(root) = agent_root else {
        return false;
    };
    let dir = root.join(id);
    !dir.join("persona.md").is_file()
        && !dir.join("PERSONA.md").is_file()
        && !dir.join("AGENT.md").is_file()
        && !dir.join("agent.md").is_file()
}

fn provider_ids(app: &AppHandle) -> HashSet<String> {
    crate::paths::config_file(app, "providers.json")
        .and_then(|path| json_ids(&path))
        .unwrap_or_default()
}

fn json_ids(path: &Path) -> Result<HashSet<String>, String> {
    if !path.is_file() {
        return Ok(HashSet::new());
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if raw.trim().is_empty() {
        return Ok(HashSet::new());
    }
    let items: Vec<Value> = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    Ok(items
        .into_iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
        .collect())
}

fn retain_secret_map(path: &Path, keep: &HashSet<String>) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if raw.trim().is_empty() {
        return Ok(());
    }
    let mut blobs: HashMap<String, Value> =
        serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let before = blobs.len();
    blobs.retain(|id, _| keep.contains(id));
    if blobs.len() == before {
        return Ok(());
    }
    write_json(
        path,
        &serde_json::to_value(&blobs).map_err(|error| error.to_string())?,
        true,
    )
}

fn write_json(path: &Path, value: &Value, secret: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())?;
    if secret {
        crate::paths::set_user_private(path);
    }
    Ok(())
}

fn remove_file(path: &Path) -> Result<(), String> {
    if path.is_file() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_prefs_for_missing_providers() {
        let mut store = Map::new();
        store.insert("modelEffort:gone:gpt".into(), Value::String("high".into()));
        store.insert("modelRequest:keep:gpt".into(), Value::String("low".into()));
        let ids = HashSet::from(["keep".to_string()]);
        assert!(orphan_setting("modelEffort:gone:gpt", &store, &ids, None));
        assert!(!orphan_setting("modelRequest:keep:gpt", &store, &ids, None));
        assert!(orphan_agent("global:../x", None));
        assert!(!orphan_agent("builtin:build", None));
    }
}
