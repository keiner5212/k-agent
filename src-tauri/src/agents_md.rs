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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsMdKindInput {
    pub kind: AgentsMdKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAgentsMdInput {
    pub kind: AgentsMdKind,
    pub content: String,
}

#[derive(Debug, Error)]
pub enum AgentsMdError {
    #[error("io error: {0}")]
    Io(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("no workspace")]
    NoWorkspace,
}

impl Serialize for AgentsMdError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.lock().ok().and_then(|guard| guard.clone())
}

fn resolve_dir(app: &AppHandle, kind: AgentsMdKind) -> Result<PathBuf, AgentsMdError> {
    match kind {
        AgentsMdKind::Global => {
            let home = app
                .path()
                .home_dir()
                .map_err(|error| AgentsMdError::Io(error.to_string()))?;
            Ok(home.join(crate::APP_CONFIG_DIR))
        }
        AgentsMdKind::Local => resolve_workspace(app).ok_or(AgentsMdError::NoWorkspace),
    }
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

fn write_target(dir: &Path) -> PathBuf {
    let located = locate_agents_md(dir);
    if located.is_file() {
        located
    } else {
        dir.join("AGENTS.md")
    }
}

fn file_from_disk(kind: AgentsMdKind, path: PathBuf) -> Result<AgentsMdFile, AgentsMdError> {
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

fn read_agents_md(kind: AgentsMdKind, dir: &Path) -> Result<AgentsMdFile, AgentsMdError> {
    file_from_disk(kind, locate_agents_md(dir))
}

fn write_agents_md_sync(
    app: &AppHandle,
    input: WriteAgentsMdInput,
) -> Result<AgentsMdFile, AgentsMdError> {
    let dir = resolve_dir(app, input.kind)?;
    fs::create_dir_all(&dir).map_err(|error| AgentsMdError::Io(error.to_string()))?;
    let path = write_target(&dir);
    fs::write(&path, &input.content).map_err(|error| AgentsMdError::Io(error.to_string()))?;
    file_from_disk(input.kind, path)
}

fn delete_agents_md_sync(
    app: &AppHandle,
    kind: AgentsMdKind,
) -> Result<(), AgentsMdError> {
    let dir = resolve_dir(app, kind)?;
    let path = locate_agents_md(&dir);
    if !path.is_file() {
        return Err(AgentsMdError::NotFound(path.display().to_string()));
    }
    fs::remove_file(&path).map_err(|error| AgentsMdError::Io(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn list_agents_md(app: AppHandle) -> Result<Vec<AgentsMdFile>, AgentsMdError> {
    tokio::task::spawn_blocking(move || collect_agents_md(&app))
        .await
        .map_err(|error| AgentsMdError::Io(error.to_string()))?
}

fn collect_agents_md(app: &AppHandle) -> Result<Vec<AgentsMdFile>, AgentsMdError> {
    let global_dir = resolve_dir(app, AgentsMdKind::Global)?;
    let mut files = vec![read_agents_md(AgentsMdKind::Global, &global_dir)?];
    if let Ok(local_dir) = resolve_dir(app, AgentsMdKind::Local) {
        files.push(read_agents_md(AgentsMdKind::Local, &local_dir)?);
    }
    Ok(files)
}

#[tauri::command]
pub async fn write_agents_md(
    app: AppHandle,
    input: WriteAgentsMdInput,
) -> Result<AgentsMdFile, AgentsMdError> {
    tokio::task::spawn_blocking(move || write_agents_md_sync(&app, input))
        .await
        .map_err(|error| AgentsMdError::Io(error.to_string()))?
}

#[tauri::command]
pub async fn delete_agents_md(app: AppHandle, input: AgentsMdKindInput) -> Result<(), AgentsMdError> {
    tokio::task::spawn_blocking(move || delete_agents_md_sync(&app, input.kind))
        .await
        .map_err(|error| AgentsMdError::Io(error.to_string()))?
}
