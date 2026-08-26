use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

use crate::APP_CONFIG_DIR;

const MCP_SERVERS_FILE: &str = "mcp-servers.json";
const MCP_SECRETS_FILE: &str = "mcp-secrets.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    Stdio,
    Sse,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
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
    #[error("command is required for stdio transport")]
    CommandRequired,
    #[error("url is required for remote transport")]
    UrlRequired,
    #[error("tool probe failed: {0}")]
    ProbeFailed(String),
    #[error("tool probe is not supported for SSE transport")]
    ProbeUnsupported,
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

fn servers_path(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| McpServerError::Path(e.to_string()))?;
    Ok(home.join(APP_CONFIG_DIR).join(MCP_SERVERS_FILE))
}

fn secrets_dir(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    app.path()
        .app_data_dir()
        .map_err(|e| McpServerError::Path(e.to_string()))
}

fn secrets_path(app: &AppHandle) -> Result<PathBuf, McpServerError> {
    Ok(secrets_dir(app)?.join(MCP_SECRETS_FILE))
}

async fn load_servers(path: &Path) -> Result<Vec<McpServer>, McpServerError> {
    match tokio::fs::read_to_string(path).await {
        Ok(data) if data.trim().is_empty() => Ok(Vec::new()),
        Ok(data) => serde_json::from_str(&data).map_err(|e| McpServerError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(McpServerError::Io(e.to_string())),
    }
}

async fn save_servers(path: &Path, servers: &[McpServer]) -> Result<(), McpServerError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| McpServerError::Io(e.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(servers).map_err(|e| McpServerError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| McpServerError::Io(e.to_string()))
}

async fn load_secret_blobs(path: &Path) -> Result<HashMap<String, String>, McpServerError> {
    match tokio::fs::read_to_string(path).await {
        Ok(data) if data.trim().is_empty() => Ok(HashMap::new()),
        Ok(data) => serde_json::from_str(&data).map_err(|e| McpServerError::Parse(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(McpServerError::Io(e.to_string())),
    }
}

async fn save_secret_blobs(
    path: &Path,
    blobs: &HashMap<String, String>,
) -> Result<(), McpServerError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| McpServerError::Io(e.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(blobs).map_err(|e| McpServerError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| McpServerError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn redact(mut server: McpServer) -> McpServer {
    server.has_env_secrets = !server.secrets.env.is_empty();
    server.has_header_secrets = !server.secrets.headers.is_empty();
    server.secrets = McpServerSecrets {
        env: HashMap::new(),
        headers: HashMap::new(),
    };
    server
}

async fn load_all(app: &AppHandle) -> Result<Vec<McpServer>, McpServerError> {
    let path = servers_path(app)?;
    let mut servers = load_servers(&path).await?;
    let dir = secrets_dir(app)?;
    let blobs = load_secret_blobs(&secrets_path(app)?).await?;
    let cipher = crate::secret::cipher(&dir)?;
    for server in &mut servers {
        if let Some(plain) = crate::secret::open(cipher, blobs.get(&server.id).map(String::as_str))?
        {
            if let Ok(secrets) = serde_json::from_str::<McpServerSecrets>(&plain) {
                server.secrets = secrets;
            }
        }
    }
    Ok(servers)
}

async fn save_all(app: &AppHandle, servers: &[McpServer]) -> Result<(), McpServerError> {
    let path = servers_path(app)?;
    let dir = secrets_dir(app)?;
    let cipher = crate::secret::cipher(&dir)?;
    let mut stored = servers.to_vec();
    let mut blobs = HashMap::new();
    for server in &mut stored {
        server.has_env_secrets = false;
        server.has_header_secrets = false;
        let has_secrets = !server.secrets.env.is_empty() || !server.secrets.headers.is_empty();
        if has_secrets {
            let plain = serde_json::to_string(&server.secrets)
                .map_err(|e| McpServerError::Parse(e.to_string()))?;
            blobs.insert(server.id.clone(), crate::secret::seal(cipher, &plain)?);
        }
        server.secrets = McpServerSecrets {
            env: HashMap::new(),
            headers: HashMap::new(),
        };
    }
    save_secret_blobs(&secrets_path(app)?, &blobs).await?;
    save_servers(&path, &stored).await
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
        McpTransport::Sse | McpTransport::Http => {
            if trim_opt(input.url.clone()).is_none() {
                return Err(McpServerError::UrlRequired);
            }
        }
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

#[tauri::command]
pub async fn list_mcp_servers(app: AppHandle) -> Result<Vec<McpServer>, McpServerError> {
    let servers = load_all(&app).await?;
    Ok(servers.into_iter().map(redact).collect())
}

#[tauri::command]
pub async fn save_mcp_server(
    app: AppHandle,
    input: SaveMcpServerInput,
) -> Result<McpServer, McpServerError> {
    validate_input(&input)?;
    let mut servers = load_all(&app).await?;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let previous = servers.iter().find(|server| server.id == id).cloned();
    let secrets = merge_secrets(
        &previous
            .as_ref()
            .map(|server| server.secrets.clone())
            .unwrap_or_default(),
        &input,
    );
    let mut stored = McpServer {
        id: id.clone(),
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
            .map(|server| server.tools.clone())
            .unwrap_or_default(),
        tools_synced_at: previous.as_ref().and_then(|server| server.tools_synced_at),
        tools_probe_error: previous
            .as_ref()
            .and_then(|server| server.tools_probe_error.clone()),
    };
    let needs_probe = previous
        .as_ref()
        .map(|server| connection_changed(server, &stored))
        .unwrap_or(true);
    if needs_probe {
        apply_tool_probe(&mut stored).await;
    }
    if let Some(idx) = servers.iter().position(|server| server.id == id) {
        servers[idx] = stored.clone();
    } else {
        servers.push(stored.clone());
    }
    save_all(&app, &servers).await?;
    Ok(redact(stored))
}

#[tauri::command]
pub async fn delete_mcp_server(app: AppHandle, id: String) -> Result<(), McpServerError> {
    let mut servers = load_all(&app).await?;
    let before = servers.len();
    servers.retain(|server| server.id != id);
    if servers.len() == before {
        return Err(McpServerError::NotFound(id));
    }
    save_all(&app, &servers).await
}

#[tauri::command]
pub async fn refresh_mcp_server_tools(
    app: AppHandle,
    id: String,
) -> Result<McpServer, McpServerError> {
    let mut servers = load_all(&app).await?;
    let server = servers
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| McpServerError::NotFound(id.clone()))?;
    apply_tool_probe(server).await;
    let updated = server.clone();
    save_all(&app, &servers).await?;
    Ok(redact(updated))
}

#[tauri::command]
pub async fn set_mcp_server_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<McpServer, McpServerError> {
    let mut servers = load_all(&app).await?;
    let server = servers
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| McpServerError::NotFound(id.clone()))?;
    server.enabled = enabled;
    let updated = server.clone();
    save_all(&app, &servers).await?;
    Ok(redact(updated))
}
