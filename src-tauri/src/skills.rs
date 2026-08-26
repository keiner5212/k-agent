use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::pathutil;

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
    pub name: String,
    pub description: String,
    pub estimated_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContext {
    pub kind: SkillContextKind,
    pub path: String,
    pub skills: Vec<SkillInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMeta {
    id: String,
    path: String,
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillInput {
    pub root_path: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPathInput {
    pub root_path: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillInput {
    pub path: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillContentInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPathOnlyInput {
    pub path: String,
}

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: path escapes skills root")]
    Forbidden,
}

impl Serialize for SkillError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
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

fn absolute_root(expanded: PathBuf) -> Result<PathBuf, SkillError> {
    if expanded.is_absolute() {
        return Ok(expanded);
    }
    Ok(std::env::current_dir()
        .map_err(|error| SkillError::Io(error.to_string()))?
        .join(expanded))
}

pub(crate) fn global_skills_root(home: &Path) -> PathBuf {
    home.join(crate::APP_CONFIG_DIR).join("skills")
}

pub(crate) fn local_skills_root(workspace: &Path) -> PathBuf {
    workspace.join(crate::WORKSPACE_AGENTS_DIR).join("skills")
}

fn ensure_dir(path: &Path) -> Result<PathBuf, SkillError> {
    fs::create_dir_all(path).map_err(|error| SkillError::Io(error.to_string()))?;
    canonicalize_safe(path)
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.lock().ok().and_then(|guard| guard.clone())
}

pub(crate) fn has_skill_manifest(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file() || dir.join("skill.md").is_file()
}

pub(crate) fn list_skills_in(root: &Path) -> Result<Vec<SkillInfo>, SkillError> {
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
        skills.push(skill_info_from_dir(&path, name));
    }
    skills.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(skills)
}

pub(crate) fn estimate_tokens(content: &str) -> u32 {
    let chars = content.chars().count();
    u32::try_from((chars + 3) / 4).unwrap_or(u32::MAX)
}

fn skill_info_from_dir(path: &Path, id: &str) -> SkillInfo {
    let content = read_skill_raw(path).unwrap_or_default();
    let (name, description) = parse_frontmatter(&content);
    SkillInfo {
        id: id.to_string(),
        path: path.display().to_string(),
        name: if name.is_empty() {
            id.to_string()
        } else {
            name
        },
        description,
        estimated_tokens: estimate_tokens(&content),
    }
}

fn read_skill_raw(path: &Path) -> Result<String, SkillError> {
    let skill_md = if path.join("SKILL.md").is_file() {
        path.join("SKILL.md")
    } else {
        path.join("skill.md")
    };
    fs::read_to_string(&skill_md).map_err(|error| SkillError::Io(error.to_string()))
}

fn validate_skill_name(name: &str) -> Result<String, SkillError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(SkillError::InvalidName("name is empty".into()));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(SkillError::InvalidName(trimmed.to_string()));
    }
    if trimmed.starts_with('.') {
        return Err(SkillError::InvalidName(trimmed.to_string()));
    }
    Ok(trimmed.to_string())
}

fn resolve_root_path(raw: &str) -> Result<PathBuf, SkillError> {
    let expanded = expand_path(raw).ok_or_else(|| SkillError::InvalidPath(raw.to_string()))?;
    absolute_root(expanded)
}

fn canonicalize_safe(p: &Path) -> Result<PathBuf, SkillError> {
    pathutil::canonicalize_path(p).map_err(|error| SkillError::Io(error.to_string()))
}

fn ensure_inside(child: &Path, root: &Path) -> Result<(), SkillError> {
    let child_canon = if child.exists() {
        canonicalize_safe(child)?
    } else {
        // For not-yet-existing child paths, canonicalize parent and append component.
        let parent = child
            .parent()
            .ok_or_else(|| SkillError::InvalidPath(child.display().to_string()))?;
        canonicalize_safe(parent)?.join(
            child
                .file_name()
                .ok_or_else(|| SkillError::InvalidPath(child.display().to_string()))?,
        )
    };
    let root_canon = canonicalize_safe(root)?;
    if !child_canon.starts_with(&root_canon) {
        return Err(SkillError::Forbidden);
    }
    Ok(())
}

fn read_skill_md(path: &Path) -> Result<(String, String), SkillError> {
    let raw = read_skill_raw(path)?;
    Ok(parse_frontmatter(&raw))
}

fn parse_frontmatter(raw: &str) -> (String, String) {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return (String::new(), String::new());
    }
    let after_first = trimmed.trim_start_matches('-').trim_start_matches('\n');
    let Some(end) = after_first.find("\n---") else {
        return (String::new(), String::new());
    };
    let block = &after_first[..end];
    let lines: Vec<&str> = block.lines().collect();
    let mut name = String::new();
    let mut description = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        if let Some(rest) = line.strip_prefix("name:") {
            let (value, next) = parse_yaml_scalar(rest, &lines, i + 1);
            name = value;
            i = next;
            continue;
        }
        if let Some(rest) = line.strip_prefix("description:") {
            let (value, next) = parse_yaml_scalar(rest, &lines, i + 1);
            description = value;
            i = next;
            continue;
        }
        i += 1;
    }
    (name, description)
}

fn build_skill_md(name: &str, description: &str) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", yaml_quote(name)));
    out.push_str(&format!("description: {}\n", yaml_quote(description)));
    out.push_str("---\n\n");
    out.push_str("# ");
    out.push_str(name);
    out.push_str("\n\n");
    out.push_str(description);
    out.push('\n');
    out
}

fn yaml_quote(value: &str) -> String {
    let needs_quote = value.contains(':')
        || value.contains('#')
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.is_empty();
    if needs_quote {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn unquote_yaml(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && *bytes.last().unwrap() == b'"')
            || (bytes[0] == b'\'' && *bytes.last().unwrap() == b'\''))
    {
        return trimmed[1..trimmed.len() - 1].replace("\\\"", "\"");
    }
    trimmed.to_string()
}

enum YamlBlockKind {
    Fold,
    Literal,
}

fn yaml_block_kind(value: &str) -> Option<(YamlBlockKind, bool)> {
    match value {
        ">" | ">+" => Some((YamlBlockKind::Fold, false)),
        ">-" => Some((YamlBlockKind::Fold, true)),
        "|" | "|+" => Some((YamlBlockKind::Literal, false)),
        "|-" => Some((YamlBlockKind::Literal, true)),
        _ => None,
    }
}

fn fold_yaml_lines(parts: &[String]) -> String {
    let mut out = String::new();
    let mut gap = false;
    for part in parts {
        if part.is_empty() {
            gap = true;
            continue;
        }
        if !out.is_empty() {
            out.push(if gap { '\n' } else { ' ' });
        }
        gap = false;
        out.push_str(part);
    }
    out
}

fn read_yaml_block(
    lines: &[&str],
    start: usize,
    kind: YamlBlockKind,
    clip: bool,
) -> (String, usize) {
    let mut i = start;
    let mut indent: Option<usize> = None;
    let mut parts: Vec<String> = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        let spaces = line.chars().take_while(|ch| *ch == ' ').count();
        if line.trim().is_empty() {
            if indent.is_some() {
                parts.push(String::new());
            }
            i += 1;
            continue;
        }
        if indent.is_none() {
            if spaces == 0 {
                break;
            }
            indent = Some(spaces);
        }
        let min = indent.unwrap_or(spaces);
        if spaces < min {
            break;
        }
        let content = if line.len() >= min { &line[min..] } else { "" };
        parts.push(content.trim_end().to_string());
        i += 1;
    }
    if clip {
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
    }
    let value = match kind {
        YamlBlockKind::Fold => fold_yaml_lines(&parts),
        YamlBlockKind::Literal => {
            let mut joined = parts.join("\n");
            if !clip && !joined.is_empty() {
                joined.push('\n');
            }
            joined
        }
    };
    (value, i)
}

pub(crate) fn parse_yaml_scalar(
    after_colon: &str,
    lines: &[&str],
    next_index: usize,
) -> (String, usize) {
    let trimmed = after_colon.trim();
    match yaml_block_kind(trimmed) {
        Some((kind, clip)) => read_yaml_block(lines, next_index, kind, clip),
        None => (unquote_yaml(trimmed), next_index),
    }
}

#[tauri::command]
pub async fn list_skills(app: AppHandle) -> Result<Vec<SkillContext>, SkillError> {
    tokio::task::spawn_blocking(move || collect_skill_contexts(&app))
        .await
        .map_err(|error| SkillError::Io(error.to_string()))?
}

fn collect_skill_contexts(app: &AppHandle) -> Result<Vec<SkillContext>, SkillError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| SkillError::Io(error.to_string()))?;
    let global_root = global_skills_root(&home);
    let global = SkillContext {
        kind: SkillContextKind::Global,
        path: global_root.display().to_string(),
        skills: list_skills_in(&global_root)?,
    };
    let mut contexts = vec![global];
    if let Some(workspace) = resolve_workspace(app) {
        let local_root = local_skills_root(&workspace);
        contexts.push(SkillContext {
            kind: SkillContextKind::Local,
            path: local_root.display().to_string(),
            skills: list_skills_in(&local_root)?,
        });
    }
    Ok(contexts)
}

pub(crate) struct LoadedSkill {
    pub path: String,
    pub body: String,
}

fn skill_markdown_body(raw: &str) -> String {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return raw.to_string();
    }
    let after_first = trimmed.trim_start_matches('-').trim_start_matches('\n');
    let Some(end) = after_first.find("\n---") else {
        return raw.to_string();
    };
    after_first[end + 4..].trim_start_matches('\n').to_string()
}

pub(crate) fn find_skill_by_name(
    app: &AppHandle,
    query: &str,
) -> Result<Option<LoadedSkill>, SkillError> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return Ok(None);
    }
    for context in collect_skill_contexts(app)? {
        for skill in context.skills {
            if skill.name.to_ascii_lowercase() != needle && skill.id.to_ascii_lowercase() != needle
            {
                continue;
            }
            let raw = read_skill_raw(Path::new(&skill.path))?;
            return Ok(Some(LoadedSkill {
                path: skill.path,
                body: skill_markdown_body(&raw),
            }));
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn read_skill_meta(input: SkillPathInput) -> Result<SkillMeta, SkillError> {
    let root = resolve_root_path(&input.root_path)?;
    if !root.exists() {
        return Err(SkillError::NotFound(input.root_path));
    }
    let validated = validate_skill_name(&input.name)?;
    let skill_path = root.join(&validated);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(validated));
    }
    ensure_inside(&skill_path, &root)?;
    let (name, description) = read_skill_md(&skill_path)?;
    let display_name = if name.is_empty() {
        validated.clone()
    } else {
        name
    };
    Ok(SkillMeta {
        id: validated,
        path: skill_path.display().to_string(),
        name: display_name,
        description,
    })
}

#[tauri::command]
pub async fn read_skill_file(input: SkillPathOnlyInput) -> Result<String, SkillError> {
    let skill_path = PathBuf::from(&input.path);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(input.path));
    }
    let skill_md = if skill_path.join("SKILL.md").is_file() {
        skill_path.join("SKILL.md")
    } else {
        skill_path.join("skill.md")
    };
    fs::read_to_string(&skill_md).map_err(|error| SkillError::Io(error.to_string()))
}

#[tauri::command]
pub async fn update_skill_content(input: UpdateSkillContentInput) -> Result<(), SkillError> {
    let skill_path = PathBuf::from(&input.path);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(input.path));
    }
    let skill_md =
        if skill_path.join("SKILL.md").is_file() || !skill_path.join("skill.md").is_file() {
            skill_path.join("SKILL.md")
        } else {
            skill_path.join("skill.md")
        };
    fs::write(&skill_md, input.content).map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn create_skill(input: CreateSkillInput) -> Result<SkillInfo, SkillError> {
    let root = ensure_dir(&resolve_root_path(&input.root_path)?)?;
    let validated = validate_skill_name(&input.name)?;
    let skill_path = root.join(&validated);
    if skill_path.exists() {
        return Err(SkillError::InvalidName(format!(
            "{validated} already exists"
        )));
    }
    ensure_inside(&skill_path, &root)?;
    fs::create_dir_all(&skill_path).map_err(|error| SkillError::Io(error.to_string()))?;
    let content = build_skill_md(&validated, &input.description);
    let estimated_tokens = estimate_tokens(&content);
    fs::write(skill_path.join("SKILL.md"), &content)
        .map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(SkillInfo {
        id: validated.clone(),
        path: skill_path.display().to_string(),
        name: validated,
        description: input.description,
        estimated_tokens,
    })
}

#[tauri::command]
pub async fn update_skill(input: UpdateSkillInput) -> Result<SkillInfo, SkillError> {
    let root_canon = canonicalize_safe(Path::new(&input.path))
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .ok_or_else(|| SkillError::InvalidPath(input.path.clone()))?;
    let skill_path = PathBuf::from(&input.path);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(input.path.clone()));
    }
    ensure_inside(&skill_path, &root_canon)?;
    let validated = validate_skill_name(&input.name)?;
    let new_path = root_canon.join(&validated);
    if validated
        != skill_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    {
        if new_path.exists() {
            return Err(SkillError::InvalidName(format!(
                "{validated} already exists"
            )));
        }
        fs::rename(&skill_path, &new_path).map_err(|error| SkillError::Io(error.to_string()))?;
    }
    let content = build_skill_md(&validated, &input.description);
    let estimated_tokens = estimate_tokens(&content);
    fs::write(new_path.join("SKILL.md"), &content)
        .map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(SkillInfo {
        id: validated.clone(),
        path: new_path.display().to_string(),
        name: validated,
        description: input.description,
        estimated_tokens,
    })
}

#[tauri::command]
pub async fn delete_skill(app: AppHandle, input: SkillPathInput) -> Result<(), SkillError> {
    let root = resolve_root_path(&input.root_path)?;
    let validated = validate_skill_name(&input.name)?;
    let skill_path = root.join(&validated);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(validated));
    }
    ensure_inside(&skill_path, &root)?;
    let kind = skill_context_kind(&app, &root);
    fs::remove_dir_all(&skill_path).map_err(|error| SkillError::Io(error.to_string()))?;
    if let Some(kind) = kind {
        let _ = crate::agents::drop_skill_ref(&app, kind, &validated);
    }
    Ok(())
}

fn skill_context_kind(app: &AppHandle, root: &Path) -> Option<crate::agents::AgentContextKind> {
    let home = app.path().home_dir().ok()?;
    let root_canon = canonicalize_safe(root).ok()?;
    if canonicalize_safe(&global_skills_root(&home))
        .ok()
        .is_some_and(|global| global == root_canon)
    {
        return Some(crate::agents::AgentContextKind::Global);
    }
    None
}
