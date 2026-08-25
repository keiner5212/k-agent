use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

use crate::load_ui_settings;
use crate::lsp::{
    install_dir, resolve_for_path, LanguageServerSpec, LspError, ResolvedLanguageServer,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct LspHub {
    sessions: Mutex<HashMap<String, Arc<LspSession>>>,
}

struct LspSession {
    stdin: Mutex<ChildStdin>,
    pending: std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    next_id: AtomicU64,
    opened: Mutex<HashSet<String>>,
    language_id: String,
    _child: Mutex<Child>,
}

impl LspHub {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

fn lsp_enabled(app: &AppHandle) -> bool {
    load_ui_settings(app)
        .and_then(|value| value.get("lspEnabled").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn file_uri(path: &Path) -> String {
    let abs = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("file://{}", abs.display())
}

fn session_key(id: &str, root: &str) -> String {
    format!("{id}\0{root}")
}

fn rpc_id(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
}

async fn write_rpc(stdin: &mut ChildStdin, body: &Value) -> Result<(), LspError> {
    let encoded = serde_json::to_vec(body).map_err(|error| LspError::Message(error.to_string()))?;
    let header = format!("Content-Length: {}\r\n\r\n", encoded.len());
    stdin
        .write_all(header.as_bytes())
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    stdin
        .write_all(&encoded)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| LspError::Io(error.to_string()))
}

async fn read_rpc(reader: &mut BufReader<tokio::process::ChildStdout>) -> Result<Value, LspError> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|error| LspError::Io(error.to_string()))?;
        if n == 0 {
            return Err(LspError::Message("language server closed stdout".into()));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().ok();
        }
    }
    let n = content_length.ok_or_else(|| LspError::Message("missing Content-Length".into()))?;
    let mut buf = vec![0u8; n];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    serde_json::from_slice(&buf).map_err(|error| LspError::Message(error.to_string()))
}

fn spawn_server(spec: &LanguageServerSpec, dir: &Path, command: &str) -> Result<Child, LspError> {
    let mut cmd = Command::new(command);
    cmd.args(&spec.args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(parent) = Path::new(command).parent() {
        if let Some(old) = std::env::var_os("PATH") {
            let mut paths = vec![parent.to_path_buf()];
            paths.extend(std::env::split_paths(&old));
            if let Some(joined) = std::env::join_paths(paths).ok() {
                cmd.env("PATH", joined);
            }
        }
    }
    for (key, value) in &spec.env {
        cmd.env(key, value);
    }
    cmd.spawn().map_err(|error| LspError::Io(error.to_string()))
}

async fn start_session(
    spec: &LanguageServerSpec,
    dir: &Path,
    command: &str,
    root: &Path,
    language_id: String,
) -> Result<Arc<LspSession>, LspError> {
    let mut child = spawn_server(spec, dir, command)?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| LspError::Message("language server stdin missing".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LspError::Message("language server stdout missing".into()))?;
    let session = Arc::new(LspSession {
        stdin: Mutex::new(stdin),
        pending: std::sync::Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        opened: Mutex::new(HashSet::new()),
        language_id,
        _child: Mutex::new(child),
    });
    let reader_session = Arc::clone(&session);
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            let message = match read_rpc(&mut reader).await {
                Ok(value) => value,
                Err(_) => break,
            };
            if let Some(id) = rpc_id(message.get("id")) {
                let sender = reader_session
                    .pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(&id));
                if let Some(sender) = sender {
                    if let Some(error) = message.get("error") {
                        let _ = sender.send(Err(error.to_string()));
                    } else {
                        let _ =
                            sender.send(Ok(message.get("result").cloned().unwrap_or(Value::Null)));
                    }
                }
            }
        }
    });
    let root_uri = file_uri(root);
    let init = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "processId": null,
            "clientInfo": { "name": "k-agent", "version": env!("CARGO_PKG_VERSION") },
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "definition": {},
                    "rename": {},
                    "publishDiagnostics": {}
                }
            },
            "initializationOptions": spec.initialization_options.clone().unwrap_or(Value::Null)
        }
    });
    let (tx, rx) = oneshot::channel();
    if let Ok(mut pending) = session.pending.lock() {
        pending.insert(0, tx);
    }
    session.next_id.store(1, Ordering::Relaxed);
    {
        let mut stdin = session.stdin.lock().await;
        write_rpc(&mut stdin, &init).await?;
    }
    let _ = tokio::time::timeout(REQUEST_TIMEOUT, rx)
        .await
        .map_err(|_| LspError::Message("initialize timed out".into()))?
        .map_err(|_| LspError::Message("initialize canceled".into()))?
        .map_err(LspError::Message)?;
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    {
        let mut stdin = session.stdin.lock().await;
        write_rpc(&mut stdin, &initialized).await?;
    }
    Ok(session)
}

async fn session_for(
    hub: &LspHub,
    app: &AppHandle,
    resolved: &ResolvedLanguageServer,
) -> Result<Arc<LspSession>, LspError> {
    if !resolved.row.installed {
        return Err(LspError::NotInstalled(resolved.row.spec.id.clone()));
    }
    let command = resolved
        .row
        .command_path
        .as_deref()
        .ok_or_else(|| LspError::NotInstalled(resolved.row.spec.id.clone()))?;
    let key = session_key(&resolved.row.spec.id, &resolved.root);
    {
        let sessions = hub.sessions.lock().await;
        if let Some(existing) = sessions.get(&key) {
            return Ok(Arc::clone(existing));
        }
    }
    let dir = install_dir(app, &resolved.row.spec)?;
    let session = start_session(
        &resolved.row.spec,
        &dir,
        command,
        Path::new(&resolved.root),
        resolved.language_id.clone(),
    )
    .await?;
    let mut sessions = hub.sessions.lock().await;
    if let Some(existing) = sessions.get(&key) {
        return Ok(Arc::clone(existing));
    }
    sessions.insert(key, Arc::clone(&session));
    Ok(session)
}

async fn request(session: &LspSession, method: &str, params: Value) -> Result<Value, LspError> {
    let id = session.next_id.fetch_add(1, Ordering::Relaxed);
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });
    let (tx, rx) = oneshot::channel();
    {
        let mut pending = session
            .pending
            .lock()
            .map_err(|error| LspError::Message(error.to_string()))?;
        pending.insert(id, tx);
    }
    {
        let mut stdin = session.stdin.lock().await;
        write_rpc(&mut stdin, &body).await?;
    }
    tokio::time::timeout(REQUEST_TIMEOUT, rx)
        .await
        .map_err(|_| LspError::Message(format!("{method} timed out")))?
        .map_err(|_| LspError::Message(format!("{method} canceled")))?
        .map_err(LspError::Message)
}

async fn notify(session: &LspSession, method: &str, params: Value) -> Result<(), LspError> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params
    });
    let mut stdin = session.stdin.lock().await;
    write_rpc(&mut stdin, &body).await
}

async fn ensure_open(session: &LspSession, path: &Path) -> Result<String, LspError> {
    let uri = file_uri(path);
    {
        let opened = session.opened.lock().await;
        if opened.contains(&uri) {
            return Ok(uri);
        }
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    notify(
        session,
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": session.language_id,
                "version": 1,
                "text": text
            }
        }),
    )
    .await?;
    session.opened.lock().await.insert(uri.clone());
    Ok(uri)
}

fn inject_uri(params: Value, uri: &str) -> Value {
    let mut params = params;
    if let Some(doc) = params
        .get_mut("textDocument")
        .and_then(Value::as_object_mut)
    {
        doc.entry("uri".to_string())
            .or_insert_with(|| Value::String(uri.to_string()));
    }
    params
}

#[tauri::command]
pub async fn lsp_request(
    app: AppHandle,
    hub: tauri::State<'_, LspHub>,
    path: String,
    method: String,
    params: Option<Value>,
) -> Result<Value, LspError> {
    if !lsp_enabled(&app) {
        return Err(LspError::Disabled);
    }
    let file = PathBuf::from(path.trim());
    if file.as_os_str().is_empty() {
        return Err(LspError::NoMatch);
    }
    let resolved = resolve_for_path(&app, &file)?.ok_or(LspError::NoMatch)?;
    let session = session_for(&hub, &app, &resolved).await?;
    let mut params = params.unwrap_or(json!({}));
    if method.starts_with("textDocument/") {
        let uri = ensure_open(&session, &file).await?;
        params = inject_uri(params, &uri);
    }
    request(&session, &method, params).await
}
