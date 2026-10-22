use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use thiserror::Error;

const LEGACY_SERVERS_FILE: &str = "mcp-servers.json";
const MCP_SECRETS_FILE: &str = "mcp-secrets.json";
const MCP_FILE: &str = "mcp.json";
const RESERVED_SLUGS: &[&str] = &["skills", "agents", "cache"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum McpScope {
    #[default]
    Global,
    Local,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    Stdio,
    #[serde(alias = "sse")]
    Http,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerSecrets {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    #[serde(default)]
    pub scope: McpScope,
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing, skip_deserializing)]
    secrets: McpServerSecrets,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_env_secrets: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_header_secrets: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<McpToolSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_synced_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_probe_error: Option<String>,
    // Lets a crashed migration find this folder on the next load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legacy_id: Option<String>,
}

impl McpServer {
    pub(crate) fn probe_env(&self) -> &HashMap<String, String> {
        &self.secrets.env
    }

    pub(crate) fn probe_headers(&self) -> &HashMap<String, String> {
        &self.secrets.headers
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMcpServerInput {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub scope: McpScope,
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub transport: McpTransport,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub clear_secrets: bool,
}

fn default_enabled() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("path resolution failed: {0}")]
    Path(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("mcp server with id {0} not found")]
    NotFound(String),
    #[error("name is required")]
    NameRequired,
    #[error("name is invalid")]
    InvalidName,
    #[error("name is reserved")]
    ReservedName,
    #[error("name is already used")]
    NameTaken,
    #[error("no workspace")]
    NoWorkspace,
    #[error("command is required for stdio transport")]
    CommandRequired,
    #[error("url is required for remote transport")]
    UrlRequired,
    #[error("tool probe failed: {0}")]
    ProbeFailed(String),
    #[error("mcp tool call failed: {0}")]
    CallFailed(String),
    #[error("{0}")]
    Crypto(String),
}

impl Serialize for McpServerError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

impl From<crate::secret::SecretError> for McpServerError {
    fn from(error: crate::secret::SecretError) -> Self {
        Self::Crypto(error.to_string())
    }
}

struct LoadedServer {
    dir: PathBuf,
    folder_id: String,
    file_id: String,
    legacy_id: Option<String>,
    server: McpServer,
}

fn scope_key(scope: McpScope) -> &'static str {
    match scope {
        McpScope::Global => "global",
        McpScope::Local => "local",
    }
}

fn make_id(scope: McpScope, slug: &str) -> String {
    format!("{}/{}", scope_key(scope), slug)
}

fn valid_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 64 {
        return false;
    }
    let mut prev_dash = true;
    for ch in slug.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            prev_dash = false;
        } else if ch == '-' && !prev_dash {
            prev_dash = true;
        } else {
            return false;
        }
    }
    !prev_dash
}

fn is_reserved(slug: &str) -> bool {
    RESERVED_SLUGS.contains(&slug)
}

fn slugify(name: &str) -> Result<String, McpServerError> {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if !valid_slug(&out) {
        return Err(McpServerError::InvalidName);
    }
    Ok(out)
}

fn parse_id(id: &str) -> Option<(McpScope, String)> {
    let (scope, slug) = id.split_once('/')?;
    if !valid_slug(slug) {
        return None;
    }
    let scope = match scope {
        "global" => McpScope::Global,
        "local" => McpScope::Local,
        _ => return None,
    };
    Some((scope, slug.to_string()))
}

fn global_root(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    crate::paths::config_dir(app).map_err(McpServerError::Path)
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let left_key = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right_key = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    left_key == right_key
}

fn local_root(app: &AppHandle) -> Option<PathBuf> {
    let workspace = crate::pathutil::workspace_from_app(app)?;
    let root = workspace.join(crate::paths::APP_CONFIG_DIR);
    match global_root(app) {
        Ok(global) if paths_equal(&root, &global) => None,
        _ => Some(root),
    }
}

fn root_for(app: &AppHandle, scope: McpScope) -> Result<PathBuf, McpServerError> {
    match scope {
        McpScope::Global => global_root(app),
        McpScope::Local => local_root(app).ok_or(McpServerError::NoWorkspace),
    }
}

fn secrets_dir(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    crate::paths::app_data_dir(app).map_err(McpServerError::Path)
}

fn secrets_path(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    Ok(secrets_dir(app)?.join(MCP_SECRETS_FILE))
}

fn allocate_slug(root: &Path, base: &str, global: bool) -> Result<String, McpServerError> {
    for n in 1..1000 {
        let candidate = if n == 1 {
            base.to_string()
        } else {
            format!("{base}-{n}")
        };
        if !valid_slug(&candidate) {
            continue;
        }
        if global && is_reserved(&candidate) {
            continue;
        }
        if root.join(&candidate).exists() {
            continue;
        }
        return Ok(candidate);
    }
    Err(McpServerError::NameTaken)
}

async fn read_text(path: &Path) -> Result<Option<String>, McpServerError> {
    match tokio::fs::read_to_string(path).await {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(McpServerError::Io(error.to_string())),
    }
}

async fn load_secret_blobs(path: &Path) -> Result<HashMap<String, String>, McpServerError> {
    let Some(data) = read_text(path).await? else {
        return Ok(HashMap::new());
    };
    if data.trim().is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str(&data).map_err(|error| McpServerError::Parse(error.to_string()))
}

async fn save_secret_blobs(
    path: &Path,
    blobs: &HashMap<String, String>,
) -> Result<(), McpServerError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| McpServerError::Io(error.to_string()))?;
    }
    let json = serde_json::to_string_pretty(blobs)
        .map_err(|error| McpServerError::Parse(error.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?;
    crate::paths::set_user_private(path);
    Ok(())
}

async fn write_server(dir: &Path, server: &McpServer) -> Result<(), McpServerError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?;
    let mut stored = server.clone();
    stored.has_env_secrets = false;
    stored.has_header_secrets = false;
    stored.secrets = McpServerSecrets::default();
    let json = serde_json::to_string_pretty(&stored)
        .map_err(|error| McpServerError::Parse(error.to_string()))?;
    tokio::fs::write(dir.join(MCP_FILE), json)
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))
}

async fn remove_server_dir(dir: &Path) -> Result<(), McpServerError> {
    match tokio::fs::remove_file(dir.join(MCP_FILE)).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(McpServerError::Io(error.to_string())),
    }
    match tokio::fs::remove_dir(dir).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) if matches!(error.raw_os_error(), Some(39 | 66)) => Ok(()),
        Err(error) => Err(McpServerError::Io(error.to_string())),
    }
}

fn rekey_blob(blobs: &mut HashMap<String, String>, from: &str, to: &str) -> bool {
    if from == to || blobs.contains_key(to) {
        return false;
    }
    let Some(blob) = blobs.remove(from) else {
        return false;
    };
    blobs.insert(to.to_string(), blob);
    true
}

fn find_legacy_slug(root: &Path, old_id: &str) -> Option<String> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(slug) = name.to_str() else {
            continue;
        };
        if !valid_slug(slug) {
            continue;
        }
        let path = entry.path().join(MCP_FILE);
        let Ok(data) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
            continue;
        };
        let legacy = value.get("legacyId").and_then(|item| item.as_str());
        let id = value.get("id").and_then(|item| item.as_str());
        if legacy == Some(old_id) || id == Some(old_id) {
            return Some(slug.to_string());
        }
    }
    None
}

async fn migrate_legacy_at(
    legacy: &Path,
    root: &Path,
    blobs: &mut HashMap<String, String>,
) -> Result<bool, McpServerError> {
    let Some(data) = read_text(legacy).await? else {
        return Ok(false);
    };
    if data.trim().is_empty() {
        return Ok(true);
    }
    let servers: Vec<McpServer> =
        serde_json::from_str(&data).map_err(|error| McpServerError::Parse(error.to_string()))?;
    for mut server in servers {
        let old_id = server.id.clone();
        if let Some(slug) = find_legacy_slug(root, &old_id) {
            let new_id = make_id(McpScope::Global, &slug);
            rekey_blob(blobs, &old_id, &new_id);
            server.id = new_id;
            server.scope = McpScope::Global;
            server.legacy_id = None;
            write_server(&root.join(slug), &server).await?;
            continue;
        }
        let base = slugify(&server.name).unwrap_or_else(|_| "server".to_string());
        let slug = allocate_slug(root, &base, true)?;
        let new_id = make_id(McpScope::Global, &slug);
        rekey_blob(blobs, &old_id, &new_id);
        server.legacy_id = Some(old_id);
        server.id = new_id;
        server.scope = McpScope::Global;
        write_server(&root.join(slug), &server).await?;
    }
    Ok(true)
}

async fn migrate_legacy(app: &AppHandle) -> Result<(), McpServerError> {
    let legacy =
        crate::paths::config_file(app, LEGACY_SERVERS_FILE).map_err(McpServerError::Path)?;
    if !legacy.is_file() {
        return Ok(());
    }
    let root = global_root(app)?;
    let path = secrets_path(app)?;
    let mut blobs = load_secret_blobs(&path).await?;
    if !migrate_legacy_at(&legacy, &root, &mut blobs).await? {
        return Ok(());
    }
    save_secret_blobs(&path, &blobs).await?;
    tokio::fs::remove_file(&legacy)
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?;
    Ok(())
}

async fn read_scope(root: &Path, scope: McpScope) -> Result<Vec<LoadedServer>, McpServerError> {
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(McpServerError::Io(error.to_string())),
    };
    let mut loaded = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|error| McpServerError::Io(error.to_string()))?;
        if !file_type.is_dir() {
            continue;
        }
        let Some(slug) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !valid_slug(&slug) {
            continue;
        }
        let dir = entry.path();
        let Some(data) = read_text(&dir.join(MCP_FILE)).await? else {
            continue;
        };
        if data.trim().is_empty() {
            continue;
        }
        // One bad file must not hide the other servers.
        let Ok(mut server) = serde_json::from_str::<McpServer>(&data) else {
            continue;
        };
        let file_id = server.id.clone();
        let legacy_id = server.legacy_id.clone();
        server.scope = scope;
        server.id = make_id(scope, &slug);
        server.legacy_id = None;
        loaded.push(LoadedServer {
            dir,
            folder_id: server.id.clone(),
            file_id,
            legacy_id,
            server,
        });
    }
    loaded.sort_by(|left, right| {
        left.server
            .name
            .to_lowercase()
            .cmp(&right.server.name.to_lowercase())
    });
    Ok(loaded)
}

fn apply_secret(
    server: &mut McpServer,
    cipher: &aes_gcm::Aes256Gcm,
    blobs: &HashMap<String, String>,
    file_id: &str,
) -> Result<(), McpServerError> {
    let stored = blobs
        .get(&server.id)
        .or_else(|| {
            if file_id == server.id {
                None
            } else {
                blobs.get(file_id)
            }
        })
        .map(String::as_str);
    if let Some(plain) = crate::secret::open(cipher, stored)? {
        if let Ok(secrets) = serde_json::from_str::<McpServerSecrets>(&plain) {
            server.secrets = secrets;
        }
    }
    Ok(())
}

pub(crate) async fn load_all(app: &AppHandle) -> Result<Vec<McpServer>, McpServerError> {
    migrate_legacy(app).await?;
    let mut loaded = read_scope(&global_root(app)?, McpScope::Global).await?;
    if let Some(root) = local_root(app) {
        loaded.extend(read_scope(&root, McpScope::Local).await?);
    }
    let dir = secrets_dir(app)?;
    let path = secrets_path(app)?;
    let mut blobs = load_secret_blobs(&path).await?;
    let cipher = crate::secret::cipher(&dir)?;
    let mut dirty = false;
    let mut servers = Vec::with_capacity(loaded.len());
    for item in &mut loaded {
        if rekey_blob(&mut blobs, &item.file_id, &item.folder_id) {
            dirty = true;
        }
        if let Some(legacy) = &item.legacy_id {
            if rekey_blob(&mut blobs, legacy, &item.folder_id) {
                dirty = true;
            }
        }
        apply_secret(&mut item.server, cipher, &blobs, &item.file_id)?;
        servers.push(item.server.clone());
    }
    if dirty {
        save_secret_blobs(&path, &blobs).await?;
    }
    for item in &loaded {
        if item.file_id != item.folder_id || item.legacy_id.is_some() {
            write_server(&item.dir, &item.server).await?;
        }
    }
    Ok(servers)
}

async fn load_one(app: &AppHandle, id: &str) -> Result<(PathBuf, McpServer), McpServerError> {
    let (scope, slug) = parse_id(id).ok_or_else(|| McpServerError::NotFound(id.to_string()))?;
    let root = root_for(app, scope)?;
    let dir = root.join(&slug);
    let Some(data) = read_text(&dir.join(MCP_FILE)).await? else {
        return Err(McpServerError::NotFound(id.to_string()));
    };
    let mut server: McpServer =
        serde_json::from_str(&data).map_err(|error| McpServerError::Parse(error.to_string()))?;
    let file_id = server.id.clone();
    server.scope = scope;
    server.id = make_id(scope, &slug);
    server.legacy_id = None;
    let secret_dir = secrets_dir(app)?;
    let blobs = load_secret_blobs(&secrets_path(app)?).await?;
    let cipher = crate::secret::cipher(&secret_dir)?;
    apply_secret(&mut server, cipher, &blobs, &file_id)?;
    Ok((dir, server))
}

async fn write_secrets(
    app: &AppHandle,
    id: &str,
    secrets: &McpServerSecrets,
    drop_id: Option<&str>,
) -> Result<(), McpServerError> {
    let path = secrets_path(app)?;
    let dir = secrets_dir(app)?;
    let cipher = crate::secret::cipher(&dir)?;
    let mut blobs = load_secret_blobs(&path).await?;
    if let Some(old) = drop_id {
        if old != id {
            blobs.remove(old);
        }
    }
    if secrets.env.is_empty() && secrets.headers.is_empty() {
        blobs.remove(id);
    } else {
        let plain = serde_json::to_string(secrets)
            .map_err(|error| McpServerError::Parse(error.to_string()))?;
        blobs.insert(id.to_string(), crate::secret::seal(cipher, &plain)?);
    }
    save_secret_blobs(&path, &blobs).await
}

fn redact(mut server: McpServer) -> McpServer {
    server.has_env_secrets = !server.secrets.env.is_empty();
    server.has_header_secrets = !server.secrets.headers.is_empty();
    server.secrets = McpServerSecrets::default();
    server.legacy_id = None;
    server
}

pub(crate) fn stored_ids(app: &AppHandle) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Ok(root) = global_root(app) {
        collect_ids(&root, McpScope::Global, &mut ids);
    }
    if let Some(root) = local_root(app) {
        collect_ids(&root, McpScope::Local, &mut ids);
    }
    ids
}

fn collect_ids(root: &Path, scope: McpScope, ids: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(slug) = name.to_str() else {
            continue;
        };
        if !valid_slug(slug) {
            continue;
        }
        let path = entry.path().join(MCP_FILE);
        if !path.is_file() {
            continue;
        }
        ids.insert(make_id(scope, slug));
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(id) = value.get("id").and_then(|item| item.as_str()) {
                    ids.insert(id.to_string());
                }
                if let Some(id) = value.get("legacyId").and_then(|item| item.as_str()) {
                    ids.insert(id.to_string());
                }
            }
        }
    }
}

fn trim_opt(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn merge_secrets(previous: &McpServerSecrets, input: &SaveMcpServerInput) -> McpServerSecrets {
    let mut env = if input.env.is_empty() {
        previous.env.clone()
    } else {
        input.env.clone()
    };
    let mut headers = if input.headers.is_empty() {
        previous.headers.clone()
    } else {
        input.headers.clone()
    };
    if input.clear_secrets {
        env.clear();
        headers.clear();
    }
    McpServerSecrets { env, headers }
}

fn connection_changed(previous: &McpServer, next: &McpServer) -> bool {
    previous.transport != next.transport
        || previous.command != next.command
        || previous.args != next.args
        || previous.cwd != next.cwd
        || previous.url != next.url
        || previous.secrets.env != next.secrets.env
        || previous.secrets.headers != next.secrets.headers
}

fn validate_input(input: &SaveMcpServerInput) -> Result<(), McpServerError> {
    if input.name.trim().is_empty() {
        return Err(McpServerError::NameRequired);
    }
    match input.transport {
        McpTransport::Stdio => {
            if trim_opt(input.command.clone()).is_none() {
                return Err(McpServerError::CommandRequired);
            }
        }
        McpTransport::Http => {
            if trim_opt(input.url.clone()).is_none() {
                return Err(McpServerError::UrlRequired);
            }
        }
    }
    Ok(())
}

fn target_dir_free(dir: &Path, previous: Option<&Path>) -> Result<(), McpServerError> {
    if previous.is_some_and(|current| current == dir) {
        return Ok(());
    }
    if dir.exists() {
        return Err(McpServerError::NameTaken);
    }
    Ok(())
}

async fn apply_tool_probe(server: &mut McpServer) {
    match crate::mcp_client::probe_tools(server).await {
        Ok(tools) => {
            server.tools = tools;
            server.tools_synced_at = Some(chrono::Utc::now().timestamp());
            server.tools_probe_error = None;
        }
        Err(error) => {
            server.tools.clear();
            server.tools_synced_at = None;
            server.tools_probe_error = Some(error.to_string());
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerList {
    pub servers: Vec<McpServer>,
    pub workspace_ready: bool,
}

#[tauri::command]
pub async fn list_mcp_servers(app: AppHandle) -> Result<McpServerList, McpServerError> {
    let servers = load_all(&app).await?;
    Ok(McpServerList {
        workspace_ready: local_root(&app).is_some(),
        servers: servers.into_iter().map(redact).collect(),
    })
}

#[tauri::command]
pub async fn save_mcp_server(
    app: AppHandle,
    input: SaveMcpServerInput,
) -> Result<McpServer, McpServerError> {
    validate_input(&input)?;
    migrate_legacy(&app).await?;
    let slug = slugify(&input.name)?;
    if input.scope == McpScope::Global && is_reserved(&slug) {
        return Err(McpServerError::ReservedName);
    }
    let new_root = root_for(&app, input.scope)?;
    let new_dir = new_root.join(&slug);
    let new_id = make_id(input.scope, &slug);
    let previous = if let Some(id) = input.id.as_deref() {
        Some(load_one(&app, id).await?)
    } else {
        None
    };
    target_dir_free(&new_dir, previous.as_ref().map(|(dir, _)| dir.as_path()))?;
    let secrets = merge_secrets(
        previous
            .as_ref()
            .map(|(_, server)| &server.secrets)
            .unwrap_or(&McpServerSecrets::default()),
        &input,
    );
    let mut stored = McpServer {
        id: new_id.clone(),
        scope: input.scope,
        name: input.name.trim().to_string(),
        enabled: input.enabled,
        transport: input.transport,
        command: trim_opt(input.command.clone()),
        args: input
            .args
            .into_iter()
            .map(|arg| arg.trim().to_string())
            .filter(|arg| !arg.is_empty())
            .collect(),
        cwd: trim_opt(input.cwd.clone()),
        url: trim_opt(input.url.clone()),
        secrets,
        has_env_secrets: false,
        has_header_secrets: false,
        tools: previous
            .as_ref()
            .map(|(_, server)| server.tools.clone())
            .unwrap_or_default(),
        tools_synced_at: previous
            .as_ref()
            .and_then(|(_, server)| server.tools_synced_at),
        tools_probe_error: previous
            .as_ref()
            .and_then(|(_, server)| server.tools_probe_error.clone()),
        legacy_id: None,
    };
    let needs_probe = previous
        .as_ref()
        .map(|(_, server)| connection_changed(server, &stored))
        .unwrap_or(true);
    if needs_probe {
        apply_tool_probe(&mut stored).await;
    }
    write_server(&new_dir, &stored).await?;
    write_secrets(
        &app,
        &stored.id,
        &stored.secrets,
        previous.as_ref().map(|(_, server)| server.id.as_str()),
    )
    .await?;
    if let Some((dir, _)) = previous.as_ref() {
        if dir != &new_dir {
            remove_server_dir(dir).await?;
        }
    }
    Ok(redact(stored))
}

async fn forget_secret_keys(app: &AppHandle, keys: &[String]) -> Result<(), McpServerError> {
    if keys.is_empty() {
        return Ok(());
    }
    let path = secrets_path(app)?;
    let mut blobs = load_secret_blobs(&path).await?;
    let before = blobs.len();
    for key in keys {
        blobs.remove(key);
    }
    if blobs.len() == before {
        return Ok(());
    }
    save_secret_blobs(&path, &blobs).await
}

#[tauri::command]
pub async fn delete_mcp_server(app: AppHandle, id: String) -> Result<(), McpServerError> {
    migrate_legacy(&app).await?;
    let (dir, server) = load_one(&app, &id).await?;
    let mut keys = vec![server.id.clone()];
    if let Some(data) = read_text(&dir.join(MCP_FILE)).await? {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(stored) = value.get("id").and_then(|item| item.as_str()) {
                keys.push(stored.to_string());
            }
            if let Some(stored) = value.get("legacyId").and_then(|item| item.as_str()) {
                keys.push(stored.to_string());
            }
        }
    }
    remove_server_dir(&dir).await?;
    forget_secret_keys(&app, &keys).await
}

#[tauri::command]
pub async fn refresh_mcp_server_tools(
    app: AppHandle,
    id: String,
) -> Result<McpServer, McpServerError> {
    migrate_legacy(&app).await?;
    let (dir, mut server) = load_one(&app, &id).await?;
    apply_tool_probe(&mut server).await;
    write_server(&dir, &server).await?;
    Ok(redact(server))
}

#[tauri::command]
pub async fn set_mcp_server_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<McpServer, McpServerError> {
    migrate_legacy(&app).await?;
    let (dir, mut server) = load_one(&app, &id).await?;
    server.enabled = enabled;
    write_server(&dir, &server).await?;
    Ok(redact(server))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("k-agent-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn slug_rejects_empty_and_reserved_shape() {
        assert_eq!(slugify("Git Hub").unwrap(), "git-hub");
        assert!(slugify("...").is_err());
        assert!(is_reserved("skills"));
        assert!(!valid_slug("Git"));
        assert!(parse_id("global/git-hub").is_some());
        assert!(parse_id("../x").is_none());
        assert!(parse_id("global/skills/nope").is_none());
    }

    #[test]
    fn allocate_skips_reserved_and_existing() {
        let root = temp_root("allocate");
        std::fs::create_dir_all(root.join("git-hub")).unwrap();
        assert_eq!(allocate_slug(&root, "skills", true).unwrap(), "skills-2");
        assert_eq!(allocate_slug(&root, "git-hub", true).unwrap(), "git-hub-2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn migrates_legacy_list_without_dropping_secret_keys() {
        let root = temp_root("migrate");
        let legacy = root.join(LEGACY_SERVERS_FILE);
        let raw = r#"[
          {"id":"uuid-a","name":"Git Hub","enabled":true,"transport":"stdio","command":"echo"},
          {"id":"uuid-b","name":"skills","enabled":true,"transport":"http","url":"https://example.test"}
        ]"#;
        std::fs::write(&legacy, raw).unwrap();
        let mut blobs = HashMap::from([
            ("uuid-a".to_string(), "blob-a".to_string()),
            ("uuid-b".to_string(), "blob-b".to_string()),
        ]);
        migrate_legacy_at(&legacy, &root, &mut blobs).await.unwrap();
        assert!(legacy.exists());
        assert!(root.join("git-hub").join(MCP_FILE).is_file());
        let migrated = std::fs::read_to_string(root.join("git-hub").join(MCP_FILE)).unwrap();
        assert!(migrated.contains("uuid-a"));
        assert!(root.join("skills-2").join(MCP_FILE).is_file());
        assert_eq!(
            blobs.get("global/git-hub").map(String::as_str),
            Some("blob-a")
        );
        assert_eq!(
            blobs.get("global/skills-2").map(String::as_str),
            Some("blob-b")
        );
        assert!(!blobs.contains_key("uuid-a"));
        let again = root.join(LEGACY_SERVERS_FILE);
        std::fs::write(
            &again,
            r#"[{"id":"uuid-a","name":"Git Hub","enabled":true,"transport":"stdio","command":"echo"}]"#,
        )
        .unwrap();
        migrate_legacy_at(&again, &root, &mut blobs).await.unwrap();
        assert!(!root.join("git-hub-2").exists());
        assert_eq!(
            blobs.get("global/git-hub").map(String::as_str),
            Some("blob-a")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn home_workspace_matches_global_root() {
        let root = temp_root("same");
        assert!(paths_equal(&root, &root));
        let project = root.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        assert!(!paths_equal(&root, &project.join(".k-agent")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn lists_only_dirs_with_mcp_json_and_strips_secrets() {
        let root = temp_root("list");
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::write(root.join("skills").join("SKILL.md"), "name: x").unwrap();
        let server = McpServer {
            id: "global/demo".into(),
            scope: McpScope::Global,
            name: "Demo".into(),
            enabled: true,
            transport: McpTransport::Stdio,
            command: Some("echo".into()),
            args: Vec::new(),
            cwd: None,
            url: None,
            secrets: McpServerSecrets {
                env: HashMap::from([("TOKEN".into(), "secret".into())]),
                headers: HashMap::new(),
            },
            has_env_secrets: true,
            has_header_secrets: false,
            tools: Vec::new(),
            tools_synced_at: None,
            tools_probe_error: None,
            legacy_id: None,
        };
        write_server(&root.join("demo"), &server).await.unwrap();
        let text = std::fs::read_to_string(root.join("demo").join(MCP_FILE)).unwrap();
        assert!(!text.contains("TOKEN"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("legacyId"));
        let loaded = read_scope(&root, McpScope::Global).await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].folder_id, "global/demo");
        assert!(loaded[0].server.secrets.env.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
