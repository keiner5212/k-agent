use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};

use crate::load_ui_settings;
use crate::lsp::{
    install_dir, resolve_for_path, LanguageServerSpec, LspError, ResolvedLanguageServer,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const DIAG_WAIT: Duration = Duration::from_millis(700);
const MAX_DIAGS: usize = 8;

pub struct LspHub {
    sessions: Mutex<HashMap<String, Arc<LspSession>>>,
}

struct StoredDiag {
    fresh: bool,
    notes: Vec<RawDiag>,
}

struct LspSession {
    stdin: Mutex<ChildStdin>,
    pending: std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    next_id: AtomicU64,
    opened: Mutex<HashMap<String, i32>>,
    diagnostics: Mutex<HashMap<String, StoredDiag>>,
    language_id: String,
    _child: Mutex<Child>,
}

#[derive(Clone)]
pub struct RawDiag {
    pub line: u32,
    pub severity: String,
    pub message: String,
}

pub struct PathDiagnostics {
    pub path: PathBuf,
    pub notes: Vec<RawDiag>,
}

struct Touch {
    session: Arc<LspSession>,
    uri: String,
}

impl LspHub {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn drop_sessions_for_spec(&self, spec_id: &str) {
        let prefix = format!("{spec_id}\0");
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|key, _| !key.starts_with(&prefix));
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
        opened: Mutex::new(HashMap::new()),
        diagnostics: Mutex::new(HashMap::new()),
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
            if let Some((uri, notes)) = published_diagnostics(&message) {
                reader_session
                    .diagnostics
                    .lock()
                    .await
                    .insert(uri, StoredDiag { fresh: true, notes });
                continue;
            }
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
                    "synchronization": { "didSave": true },
                    "definition": { "linkSupport": false },
                    "references": {},
                    "implementation": {},
                    "documentSymbol": {},
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "callHierarchy": {},
                    "rename": {},
                    "publishDiagnostics": {}
                },
                "workspace": { "symbol": {} }
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
        if opened.contains_key(&uri) {
            return Ok(uri);
        }
    }
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    open_document(session, &uri, &text).await?;
    Ok(uri)
}

async fn open_document(session: &LspSession, uri: &str, text: &str) -> Result<(), LspError> {
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
    session.opened.lock().await.insert(uri.to_string(), 1);
    Ok(())
}

pub async fn sync_disk_change(app: &AppHandle, path: &Path, deleted: bool) {
    let _ = push_change(app, path, deleted).await;
}

pub async fn format_files(app: &AppHandle, paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut changed = Vec::new();
    if !lsp_enabled(app) || paths.is_empty() {
        return changed;
    }
    for path in paths {
        if path.is_file() && format_one(app, path).await {
            changed.push(path.clone());
        }
    }
    changed
}

async fn format_one(app: &AppHandle, path: &Path) -> bool {
    let Some(touch) = push_change(app, path, false).await else {
        return false;
    };
    let Ok(result) = request(
        &touch.session,
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": touch.uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    )
    .await
    else {
        return false;
    };
    let Some(edits) = result.as_array() else {
        return false;
    };
    if edits.is_empty() {
        return false;
    }
    let Ok(original) = tokio::fs::read_to_string(path).await else {
        return false;
    };
    let Some(next) = apply_text_edits(&original, edits) else {
        return false;
    };
    if next == original {
        return false;
    }
    tokio::fs::write(path, next).await.is_ok()
}

fn apply_text_edits(text: &str, edits: &[Value]) -> Option<String> {
    let mut spans = Vec::new();
    for edit in edits {
        let range = edit.get("range")?;
        let new_text = edit.get("newText")?.as_str()?.to_string();
        let start = lsp_offset(text, range.get("start")?)?;
        let end = lsp_offset(text, range.get("end")?)?;
        if end < start || end > text.len() {
            return None;
        }
        spans.push((start, end, new_text));
    }
    spans.sort_by(|left, right| right.0.cmp(&left.0));
    let mut out = text.to_string();
    for (start, end, new_text) in spans {
        out.replace_range(start..end, &new_text);
    }
    Some(out)
}

fn lsp_offset(text: &str, pos: &Value) -> Option<usize> {
    let line = pos.get("line")?.as_u64()? as usize;
    let character = pos.get("character")?.as_u64()? as usize;
    let mut byte = 0usize;
    let mut index = 0usize;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if line > lines.len() {
        return None;
    }
    if line == lines.len() {
        return if character == 0 {
            Some(text.len())
        } else {
            None
        };
    }
    for row in &lines {
        if index == line {
            let body = row.trim_end_matches(['\n', '\r']);
            return Some(byte + utf16_to_byte(body, character)?);
        }
        byte += row.len();
        index += 1;
    }
    None
}

fn utf16_to_byte(line: &str, character: usize) -> Option<usize> {
    let mut units = 0usize;
    for (offset, ch) in line.char_indices() {
        if units == character {
            return Some(offset);
        }
        units += ch.len_utf16();
        if units > character {
            return None;
        }
    }
    if units == character {
        Some(line.len())
    } else {
        None
    }
}

pub async fn diagnostics_after_write(
    app: &AppHandle,
    paths: &[PathBuf],
) -> Option<Vec<PathDiagnostics>> {
    if !lsp_enabled(app) || paths.is_empty() {
        return None;
    }
    let mut touches = Vec::new();
    for path in paths {
        let deleted = !path.exists();
        if let Some(touch) = push_change(app, path, deleted).await {
            touches.push((path.clone(), touch));
        }
    }
    if touches.is_empty() {
        return None;
    }
    let deadline = tokio::time::Instant::now() + DIAG_WAIT;
    loop {
        if touches_fresh(&touches).await {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let mut found = Vec::new();
    for (path, touch) in touches {
        let map = touch.session.diagnostics.lock().await;
        let Some(slot) = map.get(&touch.uri) else {
            continue;
        };
        if !slot.fresh {
            continue;
        }
        found.push(PathDiagnostics {
            path,
            notes: slot.notes.clone(),
        });
    }
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

async fn touches_fresh(touches: &[(PathBuf, Touch)]) -> bool {
    for (_, touch) in touches {
        let map = touch.session.diagnostics.lock().await;
        if !map.get(&touch.uri).is_some_and(|slot| slot.fresh) {
            return false;
        }
    }
    true
}

async fn push_change(app: &AppHandle, path: &Path, deleted: bool) -> Option<Touch> {
    if !lsp_enabled(app) {
        return None;
    }
    let hub = app.try_state::<LspHub>()?;
    let resolved = resolve_for_path(app, path).ok()??;
    let session = session_for(hub.inner(), app, &resolved).await.ok()?;
    let uri = file_uri(path);
    if deleted {
        let mut opened = session.opened.lock().await;
        if opened.remove(&uri).is_none() {
            return None;
        }
        drop(opened);
        let _ = notify(
            &session,
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        )
        .await;
        return None;
    }
    let text = tokio::fs::read_to_string(path).await.ok()?;
    session.diagnostics.lock().await.insert(
        uri.clone(),
        StoredDiag {
            fresh: false,
            notes: Vec::new(),
        },
    );
    let version = {
        let mut opened = session.opened.lock().await;
        if let Some(version) = opened.get_mut(&uri) {
            *version += 1;
            *version
        } else {
            0
        }
    };
    let sent = if version == 0 {
        open_document(&session, &uri, &text).await.is_ok()
    } else {
        notify(
            &session,
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
        .await
        .is_ok()
    };
    if !sent {
        return None;
    }
    Some(Touch { session, uri })
}

fn published_diagnostics(message: &Value) -> Option<(String, Vec<RawDiag>)> {
    if message.get("method")?.as_str()? != "textDocument/publishDiagnostics" {
        return None;
    }
    let params = message.get("params")?;
    let uri = params.get("uri")?.as_str()?.to_string();
    let items = params
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut notes = Vec::new();
    for item in items {
        let severity = match item.get("severity").and_then(Value::as_u64).unwrap_or(1) {
            1 => "error",
            2 => "warning",
            3 => "info",
            _ => "hint",
        };
        let line = item
            .get("range")
            .and_then(|range| range.get("start"))
            .and_then(|start| start.get("line"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32
            + 1;
        let message = item
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if message.is_empty() {
            continue;
        }
        let message = message.chars().take(180).collect::<String>();
        notes.push(RawDiag {
            line,
            severity: severity.to_string(),
            message,
        });
    }
    notes.sort_by_key(|note| match note.severity.as_str() {
        "error" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 3,
    });
    notes.truncate(MAX_DIAGS);
    Some((uri, notes))
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

#[tauri::command]
pub async fn uninstall_language_server(
    app: AppHandle,
    hub: tauri::State<'_, LspHub>,
    id: String,
) -> Result<crate::lsp::LanguageServerRow, LspError> {
    hub.drop_sessions_for_spec(&id).await;
    crate::lsp::uninstall_managed(&app, &id).await
}
