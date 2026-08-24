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
pub struct ListSkillsInput {
    pub global_path: String,
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

#[derive(Debug, Error, Serialize)]
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

fn normalize_global_root(path: PathBuf) -> PathBuf {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(".k-agent") => path.join("skills"),
        _ => path,
    }
}

fn local_skills_root(workspace: &Path) -> PathBuf {
    for program in LOCAL_PROGRAMS {
        let candidate = workspace.join(program).join("skills");
        if candidate.is_dir() {
            return candidate;
        }
    }
    workspace.join(".k-agent").join("skills")
}

const LOCAL_PROGRAMS: &[&str] = &[
    ".agents",
    ".k-agent",
    ".agent",
    ".opencode",
    ".claude",
    ".codex",
    ".cursor",
    ".gemini",
    ".github",
    ".continue",
    ".aider",
];

fn scan_local_skills(workspace: &Path) -> (Vec<PathBuf>, Vec<SkillInfo>) {
    let mut roots = Vec::new();
    let mut skills = Vec::new();
    for program in LOCAL_PROGRAMS {
        let candidate = workspace.join(program).join("skills");
        if candidate.is_dir() {
            roots.push(candidate.clone());
            if let Ok(items) = list_skills_in(&candidate) {
                skills.extend(items);
            }
        }
    }
    skills.sort_by(|left, right| left.id.cmp(&right.id));
    (roots, skills)
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.clone()
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
    fs::canonicalize(p).map_err(|error| SkillError::Io(error.to_string()))
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
    let skill_md = if path.join("SKILL.md").is_file() {
        path.join("SKILL.md")
    } else {
        path.join("skill.md")
    };
    let raw = fs::read_to_string(&skill_md).map_err(|error| SkillError::Io(error.to_string()))?;
    let (name, description) = parse_frontmatter(&raw);
    Ok((name, description))
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
    let mut name = String::new();
    let mut description = String::new();
    for line in block.lines() {
        let line = line.trim_end();
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().trim_matches('"').to_string();
        } else if let Some(value) = line.strip_prefix("description:") {
            description = value.trim().trim_matches('"').to_string();
        }
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

#[tauri::command]
pub async fn list_skills(
    app: AppHandle,
    input: ListSkillsInput,
) -> Result<Vec<SkillContext>, SkillError> {
    let expanded = expand_path(&input.global_path)
        .ok_or_else(|| SkillError::InvalidPath(input.global_path.clone()))?;
    let global_root = normalize_global_root(absolute_root(expanded)?);
    let global_canon = fs::canonicalize(&global_root).unwrap_or_else(|_| global_root.clone());
    let global = SkillContext {
        kind: SkillContextKind::Global,
        path: global_root.display().to_string(),
        skills: list_skills_in(&global_root)?,
    };
    let mut contexts = vec![global];
    if let Some(workspace) = resolve_workspace(&app) {
        let (roots, _) = scan_local_skills(&workspace);
        let mut deduped_roots = Vec::new();
        for root in roots {
            let canon = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
            if canon == global_canon {
                continue;
            }
            deduped_roots.push(root);
        }
        let mut deduped_skills: Vec<SkillInfo> = Vec::new();
        for root in &deduped_roots {
            if let Ok(items) = list_skills_in(root) {
                deduped_skills.extend(items);
            }
        }
        deduped_skills.sort_by(|left, right| left.id.cmp(&right.id));
        if !deduped_roots.is_empty() {
            let local_path = deduped_roots
                .first()
                .cloned()
                .unwrap_or_else(|| local_skills_root(&workspace));
            let local = SkillContext {
                kind: SkillContextKind::Local,
                path: local_path.display().to_string(),
                skills: deduped_skills,
            };
            contexts.push(local);
        }
    }
    Ok(contexts)
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
    let skill_md = if skill_path.join("SKILL.md").is_file() || !skill_path.join("skill.md").is_file() {
        skill_path.join("SKILL.md")
    } else {
        skill_path.join("skill.md")
    };
    fs::write(&skill_md, input.content).map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn create_skill(input: CreateSkillInput) -> Result<SkillInfo, SkillError> {
    let root = resolve_root_path(&input.root_path)?;
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
    fs::write(skill_path.join("SKILL.md"), content)
        .map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(SkillInfo {
        id: validated,
        path: skill_path.display().to_string(),
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
    if validated != skill_path.file_name().and_then(|n| n.to_str()).unwrap_or("") {
        if new_path.exists() {
            return Err(SkillError::InvalidName(format!(
                "{validated} already exists"
            )));
        }
        fs::rename(&skill_path, &new_path).map_err(|error| SkillError::Io(error.to_string()))?;
    }
    let content = build_skill_md(&validated, &input.description);
    fs::write(new_path.join("SKILL.md"), content)
        .map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(SkillInfo {
        id: validated,
        path: new_path.display().to_string(),
    })
}

#[tauri::command]
pub async fn delete_skill(input: SkillPathInput) -> Result<(), SkillError> {
    let root = resolve_root_path(&input.root_path)?;
    let validated = validate_skill_name(&input.name)?;
    let skill_path = root.join(&validated);
    if !skill_path.is_dir() {
        return Err(SkillError::NotFound(validated));
    }
    ensure_inside(&skill_path, &root)?;
    fs::remove_dir_all(&skill_path).map_err(|error| SkillError::Io(error.to_string()))?;
    Ok(())
}