use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::skills::estimate_tokens;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentsMdKind {
    Global,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsMdFile {
    pub kind: AgentsMdKind,
    pub path: String,
    pub exists: bool,
    pub content: String,
    pub estimated_tokens: u32,
}

#[derive(Debug, Error, Serialize)]
pub enum AgentsMdError {
    #[error("io error: {0}")]
    Io(String),
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.lock().ok().and_then(|guard| guard.clone())
}

fn locate_agents_md(dir: &Path) -> PathBuf {
    let upper = dir.join("AGENTS.md");
    if upper.is_file() {
        return upper;
    }
    let lower = dir.join("agents.md");
    if lower.is_file() {
        return lower;
    }
    upper
}

fn read_agents_md(kind: AgentsMdKind, dir: &Path) -> Result<AgentsMdFile, AgentsMdError> {
    let path = locate_agents_md(dir);
    if !path.is_file() {
        return Ok(AgentsMdFile {
            kind,
            path: path.display().to_string(),
            exists: false,
            content: String::new(),
            estimated_tokens: 0,
        });
    }
    let content = fs::read_to_string(&path).map_err(|error| AgentsMdError::Io(error.to_string()))?;
    let estimated_tokens = estimate_tokens(&content);
    Ok(AgentsMdFile {
        kind,
        path: path.display().to_string(),
        exists: true,
        content,
        estimated_tokens,
    })
}

#[tauri::command]
pub async fn list_agents_md(app: AppHandle) -> Result<Vec<AgentsMdFile>, AgentsMdError> {
    tokio::task::spawn_blocking(move || collect_agents_md(&app))
        .await
        .map_err(|error| AgentsMdError::Io(error.to_string()))?
}

fn collect_agents_md(app: &AppHandle) -> Result<Vec<AgentsMdFile>, AgentsMdError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentsMdError::Io(error.to_string()))?;
    let global_dir = home.join(crate::APP_CONFIG_DIR);
    let mut files = vec![read_agents_md(AgentsMdKind::Global, &global_dir)?];
    if let Some(workspace) = resolve_workspace(app) {
        files.push(read_agents_md(AgentsMdKind::Local, &workspace)?);
    }
    Ok(files)
}
