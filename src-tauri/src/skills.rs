use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillContextKind {
    Global,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContext {
    pub kind: SkillContextKind,
    pub path: String,
    pub skills: Vec<SkillInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSkillsInput {
    pub global_path: String,
}

#[derive(Debug, Error, Serialize)]
pub enum SkillError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("io error: {0}")]
    Io(String),
}

fn expand_path(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "~" {
        return std::env::var_os("HOME").map(PathBuf::from);
    }
    if let Some(suffix) = trimmed.strip_prefix("~/") {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        return Some(home.join(suffix));
    }
    Some(PathBuf::from(trimmed))
}

fn has_skill_manifest(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file() || dir.join("skill.md").is_file()
}

fn list_skills_in(root: &Path) -> Result<Vec<SkillInfo>, SkillError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(SkillError::Io(error.to_string())),
    };
    let mut skills = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => return Err(SkillError::Io(error.to_string())),
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(kind) => kind,
            Err(error) => return Err(SkillError::Io(error.to_string())),
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if !has_skill_manifest(&path) {
            continue;
        }
        skills.push(SkillInfo {
            id: name.to_string(),
            path: path.display().to_string(),
        });
    }
    skills.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(skills)
}

fn absolute_root(expanded: PathBuf) -> Result<PathBuf, SkillError> {
    if expanded.is_absolute() {
        return Ok(expanded);
    }
    Ok(std::env::current_dir()
        .map_err(|error| SkillError::Io(error.to_string()))?
        .join(expanded))
}

fn normalize_global_root(path: PathBuf) -> PathBuf {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(".k-agent") => path.join("skills"),
        _ => path,
    }
}

fn local_skills_root(workspace: &Path) -> PathBuf {
    let agents = workspace.join(".agents").join("skills");
    if agents.is_dir() {
        return agents;
    }
    let kagent = workspace.join(".k-agent").join("skills");
    if kagent.is_dir() {
        return kagent;
    }
    agents
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.clone()
}

#[tauri::command]
pub async fn list_skills(
    app: AppHandle,
    input: ListSkillsInput,
) -> Result<Vec<SkillContext>, SkillError> {
    let global_expanded = expand_path(&input.global_path)
        .ok_or_else(|| SkillError::InvalidPath(input.global_path.clone()))?;
    let global_root = normalize_global_root(absolute_root(global_expanded)?);
    let global = SkillContext {
        kind: SkillContextKind::Global,
        path: global_root.display().to_string(),
        skills: list_skills_in(&global_root)?,
    };

    let mut contexts = vec![global];
    if let Some(workspace) = resolve_workspace(&app) {
        let local_root = local_skills_root(&workspace);
        let local = SkillContext {
            kind: SkillContextKind::Local,
            path: local_root.display().to_string(),
            skills: list_skills_in(&local_root)?,
        };
        contexts.push(local);
    }
    Ok(contexts)
}
