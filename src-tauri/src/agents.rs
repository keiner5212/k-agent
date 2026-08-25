use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::skills::{
    estimate_tokens, global_skills_root, list_skills_in, local_skills_root, parse_yaml_scalar,
};

pub const MAX_AGENT_SKILLS: usize = 10;
pub const MAX_AGENT_PERSONALITY_LINES: usize = 200;
const PERSONA_MANIFEST: &str = "persona.md";
const LEGACY_MANIFESTS: &[&str] = &["PERSONA.md", "AGENT.md", "agent.md"];

const AGENT_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "delete_file",
    "run_command",
    "search_code",
    "web_fetch",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AgentContextKind {
    Global,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillRef {
    pub kind: AgentContextKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMeta {
    pub id: String,
    pub path: String,
    pub name: String,
    pub description: String,
    pub personality: String,
    pub estimated_tokens: u32,
    pub skills: Vec<AgentSkillRef>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContext {
    pub kind: AgentContextKind,
    pub path: String,
    pub agents: Vec<AgentMeta>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentInput {
    pub root_path: String,
    pub kind: AgentContextKind,
    pub name: String,
    pub description: String,
    pub personality: String,
    pub skills: Vec<AgentSkillRef>,
    pub tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPathInput {
    pub root_path: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentInput {
    pub path: String,
    pub kind: AgentContextKind,
    pub name: String,
    pub description: String,
    pub personality: String,
    pub skills: Vec<AgentSkillRef>,
    pub tools: Vec<String>,
}

#[derive(Debug, Error, Serialize)]
pub enum AgentError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: path escapes agents root")]
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

fn absolute_root(expanded: PathBuf) -> Result<PathBuf, AgentError> {
    if expanded.is_absolute() {
        return Ok(expanded);
    }
    Ok(std::env::current_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?
        .join(expanded))
}

fn global_agents_root(home: &Path) -> PathBuf {
    home.join(crate::APP_CONFIG_DIR).join("agents")
}

fn local_agents_root(workspace: &Path) -> PathBuf {
    workspace.join(crate::WORKSPACE_AGENTS_DIR).join("agents")
}

fn ensure_dir(path: &Path) -> Result<PathBuf, AgentError> {
    fs::create_dir_all(path).map_err(|error| AgentError::Io(error.to_string()))?;
    canonicalize_safe(path)
}

fn resolve_workspace(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<crate::LocalWorkspace>();
    state.path.lock().ok().and_then(|guard| guard.clone())
}

fn has_agent_manifest(dir: &Path) -> bool {
    find_agent_manifest(dir).is_some()
}

fn find_agent_manifest(dir: &Path) -> Option<PathBuf> {
    let preferred = dir.join(PERSONA_MANIFEST);
    if preferred.is_file() {
        return Some(preferred);
    }
    for name in LEGACY_MANIFESTS {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn agent_manifest_path(dir: &Path) -> PathBuf {
    dir.join(PERSONA_MANIFEST)
}

fn validate_agent_name(name: &str) -> Result<String, AgentError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AgentError::InvalidName("name is empty".into()));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(AgentError::InvalidName(trimmed.to_string()));
    }
    if trimmed.starts_with('.') {
        return Err(AgentError::InvalidName(trimmed.to_string()));
    }
    Ok(trimmed.to_string())
}

fn resolve_root_path(raw: &str) -> Result<PathBuf, AgentError> {
    let expanded = expand_path(raw).ok_or_else(|| AgentError::InvalidPath(raw.to_string()))?;
    absolute_root(expanded)
}

fn canonicalize_safe(p: &Path) -> Result<PathBuf, AgentError> {
    fs::canonicalize(p).map_err(|error| AgentError::Io(error.to_string()))
}

fn ensure_inside(child: &Path, root: &Path) -> Result<(), AgentError> {
    let child_canon = if child.exists() {
        canonicalize_safe(child)?
    } else {
        let parent = child
            .parent()
            .ok_or_else(|| AgentError::InvalidPath(child.display().to_string()))?;
        canonicalize_safe(parent)?.join(
            child
                .file_name()
                .ok_or_else(|| AgentError::InvalidPath(child.display().to_string()))?,
        )
    };
    let root_canon = canonicalize_safe(root)?;
    if !child_canon.starts_with(&root_canon) {
        return Err(AgentError::Forbidden);
    }
    Ok(())
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

fn clamp_personality(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut lines: Vec<&str> = raw.split('\n').collect();
    if lines.len() > MAX_AGENT_PERSONALITY_LINES {
        lines.truncate(MAX_AGENT_PERSONALITY_LINES);
        return lines.join("\n");
    }
    raw.to_string()
}

fn parse_skill_ref(raw: &str) -> Option<AgentSkillRef> {
    let (kind_raw, id_raw) = raw.split_once('/')?;
    let kind = match kind_raw {
        "global" => AgentContextKind::Global,
        "local" => AgentContextKind::Local,
        _ => return None,
    };
    let id = validate_agent_name(id_raw).ok()?;
    Some(AgentSkillRef { kind, id })
}

fn skill_ref_key(skill: &AgentSkillRef) -> String {
    let kind = match skill.kind {
        AgentContextKind::Global => "global",
        AgentContextKind::Local => "local",
    };
    format!("{kind}/{}", skill.id)
}

struct ParsedAgent {
    name: String,
    description: String,
    skills: Vec<AgentSkillRef>,
    tools: Vec<String>,
    body: String,
}

fn parse_agent_md(raw: &str) -> ParsedAgent {
    let normalized = raw.replace("\r\n", "\n");
    let trimmed = normalized.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return ParsedAgent {
            name: String::new(),
            description: String::new(),
            skills: Vec::new(),
            tools: Vec::new(),
            body: trimmed.to_string(),
        };
    }
    let after_first = trimmed.trim_start_matches('-').trim_start_matches('\n');
    let Some(end) = after_first.find("\n---") else {
        return ParsedAgent {
            name: String::new(),
            description: String::new(),
            skills: Vec::new(),
            tools: Vec::new(),
            body: trimmed.to_string(),
        };
    };
    let block = &after_first[..end];
    let rest = &after_first[end + 4..];
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();
    let mut name = String::new();
    let mut description = String::new();
    let mut skills = Vec::new();
    let mut tools = Vec::new();
    let mut list: Option<&str> = None;
    let lines: Vec<&str> = block.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let trimmed_line = line.trim();
        if trimmed_line == "skills:" || trimmed_line == "skills: []" {
            list = Some("skills");
            i += 1;
            continue;
        }
        if trimmed_line == "tools:" || trimmed_line == "tools: []" {
            list = Some("tools");
            i += 1;
            continue;
        }
        if let Some(item) = trimmed_line.strip_prefix("- ") {
            if list == Some("skills") {
                if let Some(skill) = parse_skill_ref(item.trim()) {
                    skills.push(skill);
                }
            } else if list == Some("tools") {
                let tool = item.trim().to_string();
                if !tool.is_empty() && !tools.contains(&tool) {
                    tools.push(tool);
                }
            }
            i += 1;
            continue;
        }
        list = None;
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
    ParsedAgent {
        name,
        description,
        skills,
        tools,
        body,
    }
}

fn build_agent_md(
    name: &str,
    description: &str,
    skills: &[AgentSkillRef],
    tools: &[String],
    body: &str,
) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", yaml_quote(name)));
    out.push_str(&format!("description: {}\n", yaml_quote(description)));
    if skills.is_empty() {
        out.push_str("skills: []\n");
    } else {
        out.push_str("skills:\n");
        for skill in skills {
            out.push_str("  - ");
            out.push_str(&skill_ref_key(skill));
            out.push('\n');
        }
    }
    if tools.is_empty() {
        out.push_str("tools: []\n");
    } else {
        out.push_str("tools:\n");
        for tool in tools {
            out.push_str("  - ");
            out.push_str(tool);
            out.push('\n');
        }
    }
    out.push_str("---\n");
    let trimmed_body = body.trim_end_matches('\n');
    if trimmed_body.is_empty() {
        out.push('\n');
    } else {
        out.push('\n');
        out.push_str(trimmed_body);
        out.push('\n');
    }
    out
}

fn live_skill_ids(
    home: &Path,
    workspace: Option<&Path>,
) -> Result<HashSet<AgentSkillRef>, AgentError> {
    let mut live = HashSet::new();
    for skill in list_skills_in(&global_skills_root(home)).map_err(skill_err)? {
        live.insert(AgentSkillRef {
            kind: AgentContextKind::Global,
            id: skill.id,
        });
    }
    if let Some(workspace) = workspace {
        for skill in list_skills_in(&local_skills_root(workspace)).map_err(skill_err)? {
            live.insert(AgentSkillRef {
                kind: AgentContextKind::Local,
                id: skill.id,
            });
        }
    }
    Ok(live)
}

fn skill_err(error: crate::skills::SkillError) -> AgentError {
    AgentError::Io(error.to_string())
}

fn sanitize_tools(tools: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for tool in tools {
        if AGENT_TOOLS.contains(&tool.as_str()) && !out.contains(tool) {
            out.push(tool.clone());
        }
    }
    out
}

fn sanitize_skills(
    skills: &[AgentSkillRef],
    agent_kind: AgentContextKind,
    live: &HashSet<AgentSkillRef>,
) -> Vec<AgentSkillRef> {
    let mut out = Vec::new();
    for skill in skills {
        if agent_kind == AgentContextKind::Global && skill.kind != AgentContextKind::Global {
            continue;
        }
        if !live.contains(skill) {
            continue;
        }
        if out.iter().any(|existing: &AgentSkillRef| existing == skill) {
            continue;
        }
        if out.len() >= MAX_AGENT_SKILLS {
            break;
        }
        out.push(skill.clone());
    }
    out
}

fn read_parsed_agent(dir: &Path) -> Result<ParsedAgent, AgentError> {
    let path =
        find_agent_manifest(dir).ok_or_else(|| AgentError::NotFound(dir.display().to_string()))?;
    let raw = fs::read_to_string(&path).map_err(|error| AgentError::Io(error.to_string()))?;
    Ok(parse_agent_md(&raw))
}

fn write_agent_md(dir: &Path, content: &str) -> Result<(), AgentError> {
    fs::write(agent_manifest_path(dir), content)
        .map_err(|error| AgentError::Io(error.to_string()))?;
    for name in LEGACY_MANIFESTS {
        let leftover = dir.join(name);
        if leftover.is_file() {
            let _ = fs::remove_file(leftover);
        }
    }
    Ok(())
}

fn agent_meta_from_dir(
    dir: &Path,
    id: String,
    agent_kind: AgentContextKind,
    live: &HashSet<AgentSkillRef>,
) -> Result<(AgentMeta, bool), AgentError> {
    let parsed = read_parsed_agent(dir)?;
    let skills = sanitize_skills(&parsed.skills, agent_kind, live);
    let tools = sanitize_tools(&parsed.tools);
    let changed = skills != parsed.skills || tools != parsed.tools;
    if changed {
        write_agent_md(
            dir,
            &build_agent_md(
                if parsed.name.is_empty() {
                    &id
                } else {
                    &parsed.name
                },
                &parsed.description,
                &skills,
                &tools,
                &parsed.body,
            ),
        )?;
    }
    let display_name = if parsed.name.is_empty() {
        id.clone()
    } else {
        parsed.name
    };
    let content = build_agent_md(
        &display_name,
        &parsed.description,
        &skills,
        &tools,
        &parsed.body,
    );
    Ok((
        AgentMeta {
            id,
            path: dir.display().to_string(),
            name: display_name,
            description: parsed.description,
            personality: parsed.body,
            estimated_tokens: estimate_tokens(&content),
            skills,
            tools,
        },
        changed,
    ))
}

fn list_agents_in(
    root: &Path,
    agent_kind: AgentContextKind,
    live: &HashSet<AgentSkillRef>,
) -> Result<Vec<AgentMeta>, AgentError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(AgentError::Io(error.to_string())),
    };
    let mut agents = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => return Err(AgentError::Io(error.to_string())),
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(kind) => kind,
            Err(error) => return Err(AgentError::Io(error.to_string())),
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
        if !has_agent_manifest(&path) {
            continue;
        }
        let (meta, _) = agent_meta_from_dir(&path, name.to_string(), agent_kind, live)?;
        agents.push(meta);
    }
    agents.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(agents)
}

fn rewrite_agents_dropping_skill(
    root: &Path,
    kind: AgentContextKind,
    skill_id: &str,
) -> Result<(), AgentError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(AgentError::Io(error.to_string())),
    };
    let target = AgentSkillRef {
        kind,
        id: skill_id.to_string(),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => return Err(AgentError::Io(error.to_string())),
        };
        let path = entry.path();
        if !path.is_dir() || !has_agent_manifest(&path) {
            continue;
        }
        let parsed = read_parsed_agent(&path)?;
        let next: Vec<AgentSkillRef> = parsed
            .skills
            .iter()
            .filter(|skill| *skill != &target)
            .cloned()
            .collect();
        if next.len() == parsed.skills.len() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        let name = if parsed.name.is_empty() {
            &id
        } else {
            &parsed.name
        };
        let tools = sanitize_tools(&parsed.tools);
        write_agent_md(
            &path,
            &build_agent_md(name, &parsed.description, &next, &tools, &parsed.body),
        )?;
    }
    Ok(())
}

pub(crate) fn drop_skill_ref(
    app: &AppHandle,
    kind: AgentContextKind,
    skill_id: &str,
) -> Result<(), AgentError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?;
    rewrite_agents_dropping_skill(&global_agents_root(&home), kind, skill_id)?;
    if let Some(workspace) = resolve_workspace(app) {
        rewrite_agents_dropping_skill(&local_agents_root(&workspace), kind, skill_id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_agents(app: AppHandle) -> Result<Vec<AgentContext>, AgentError> {
    tokio::task::spawn_blocking(move || collect_agent_contexts(&app))
        .await
        .map_err(|error| AgentError::Io(error.to_string()))?
}

fn collect_agent_contexts(app: &AppHandle) -> Result<Vec<AgentContext>, AgentError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?;
    let workspace = resolve_workspace(app);
    let live = live_skill_ids(&home, workspace.as_deref())?;
    let global_root = global_agents_root(&home);
    let global = AgentContext {
        kind: AgentContextKind::Global,
        path: global_root.display().to_string(),
        agents: list_agents_in(&global_root, AgentContextKind::Global, &live)?,
    };
    let mut contexts = vec![global];
    if let Some(workspace) = workspace {
        let local_root = local_agents_root(&workspace);
        contexts.push(AgentContext {
            kind: AgentContextKind::Local,
            path: local_root.display().to_string(),
            agents: list_agents_in(&local_root, AgentContextKind::Local, &live)?,
        });
    }
    Ok(contexts)
}

#[tauri::command]
pub async fn read_agent_meta(
    app: AppHandle,
    input: AgentPathInput,
) -> Result<AgentMeta, AgentError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?;
    let workspace = resolve_workspace(&app);
    let live = live_skill_ids(&home, workspace.as_deref())?;
    let root = resolve_root_path(&input.root_path)?;
    if !root.exists() {
        return Err(AgentError::NotFound(input.root_path));
    }
    let validated = validate_agent_name(&input.name)?;
    let agent_path = root.join(&validated);
    if !agent_path.is_dir() {
        return Err(AgentError::NotFound(validated));
    }
    ensure_inside(&agent_path, &root)?;
    let kind = agent_kind_for_root(&root, &home, workspace.as_deref());
    let (meta, _) = agent_meta_from_dir(&agent_path, validated, kind, &live)?;
    Ok(meta)
}

fn agent_kind_for_root(root: &Path, home: &Path, workspace: Option<&Path>) -> AgentContextKind {
    if let Ok(root_canon) = canonicalize_safe(root) {
        if canonicalize_safe(&global_agents_root(home))
            .ok()
            .is_some_and(|global| global == root_canon)
        {
            return AgentContextKind::Global;
        }
        if let Some(workspace) = workspace {
            if canonicalize_safe(&local_agents_root(workspace))
                .ok()
                .is_some_and(|local| local == root_canon)
            {
                return AgentContextKind::Local;
            }
        }
    }
    AgentContextKind::Local
}

#[tauri::command]
pub async fn create_agent(
    app: AppHandle,
    input: CreateAgentInput,
) -> Result<AgentMeta, AgentError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?;
    let workspace = resolve_workspace(&app);
    let live = live_skill_ids(&home, workspace.as_deref())?;
    let root = ensure_dir(&resolve_root_path(&input.root_path)?)?;
    let validated = validate_agent_name(&input.name)?;
    let agent_path = root.join(&validated);
    if agent_path.exists() {
        return Err(AgentError::InvalidName(format!(
            "{validated} already exists"
        )));
    }
    ensure_inside(&agent_path, &root)?;
    let skills = sanitize_skills(&input.skills, input.kind, &live);
    let tools = sanitize_tools(&input.tools);
    let personality = clamp_personality(&input.personality);
    fs::create_dir_all(&agent_path).map_err(|error| AgentError::Io(error.to_string()))?;
    let content = build_agent_md(
        &validated,
        &input.description,
        &skills,
        &tools,
        &personality,
    );
    write_agent_md(&agent_path, &content)?;
    Ok(AgentMeta {
        id: validated.clone(),
        path: agent_path.display().to_string(),
        name: validated,
        description: input.description,
        personality,
        estimated_tokens: estimate_tokens(&content),
        skills,
        tools,
    })
}

#[tauri::command]
pub async fn update_agent(
    app: AppHandle,
    input: UpdateAgentInput,
) -> Result<AgentMeta, AgentError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AgentError::Io(error.to_string()))?;
    let workspace = resolve_workspace(&app);
    let live = live_skill_ids(&home, workspace.as_deref())?;
    let root_canon = canonicalize_safe(Path::new(&input.path))
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .ok_or_else(|| AgentError::InvalidPath(input.path.clone()))?;
    let agent_path = PathBuf::from(&input.path);
    if !agent_path.is_dir() {
        return Err(AgentError::NotFound(input.path.clone()));
    }
    ensure_inside(&agent_path, &root_canon)?;
    let validated = validate_agent_name(&input.name)?;
    let mut new_path = agent_path.clone();
    if validated
        != agent_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    {
        new_path = root_canon.join(&validated);
        if new_path.exists() {
            return Err(AgentError::InvalidName(format!(
                "{validated} already exists"
            )));
        }
        fs::rename(&agent_path, &new_path).map_err(|error| AgentError::Io(error.to_string()))?;
    }
    let skills = sanitize_skills(&input.skills, input.kind, &live);
    let tools = sanitize_tools(&input.tools);
    let personality = clamp_personality(&input.personality);
    let content = build_agent_md(
        &validated,
        &input.description,
        &skills,
        &tools,
        &personality,
    );
    write_agent_md(&new_path, &content)?;
    Ok(AgentMeta {
        id: validated.clone(),
        path: new_path.display().to_string(),
        name: validated,
        description: input.description,
        personality,
        estimated_tokens: estimate_tokens(&content),
        skills,
        tools,
    })
}

#[tauri::command]
pub async fn delete_agent(input: AgentPathInput) -> Result<(), AgentError> {
    let root = resolve_root_path(&input.root_path)?;
    let validated = validate_agent_name(&input.name)?;
    let agent_path = root.join(&validated);
    if !agent_path.is_dir() {
        return Err(AgentError::NotFound(validated));
    }
    ensure_inside(&agent_path, &root)?;
    fs::remove_dir_all(&agent_path).map_err(|error| AgentError::Io(error.to_string()))?;
    Ok(())
}
