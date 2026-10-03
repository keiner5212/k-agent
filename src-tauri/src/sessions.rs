use std::collections::HashSet;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use thiserror::Error;
use uuid::Uuid;

use crate::attachments::ChatAttachment;

const SESSIONS_FILE: &str = "sessions.json";
const SESSIONS_DIR: &str = "sessions";
const SESSION_FILE: &str = "session.json";
const INDEX_MAX_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_ai_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_rounds: Vec<crate::chat::ToolRoundTrace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_ask: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resume_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: i64,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outside_workspace_allowed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub http_write_allowed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todos: Vec<crate::tools::todo::TodoItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todos_history: Vec<crate::tools::todo::TodoHistoryEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_checkpoints: Vec<TurnCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redo: Option<RedoRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTouch {
    pub path: String,
    pub before_hash: String,
    pub after_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCheckpoint {
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub checkpoint_id: String,
    pub files: Vec<FileTouch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedoRecord {
    pub messages: Vec<SessionMessage>,
    #[serde(default)]
    pub checkpoints: Vec<TurnCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub todos: Option<Vec<crate::tools::todo::TodoItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsSnapshot {
    pub active_session_id: String,
    pub sessions: Vec<SessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionIndexEntry {
    id: String,
    title: String,
    preview: String,
    updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionsIndex {
    active_session_id: String,
    sessions: Vec<SessionIndexEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSessionAttachmentInput {
    pub session_id: String,
    pub attachment_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSessionFileRevisionInput {
    pub session_id: String,
    pub call_id: String,
    pub side: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFileRevision {
    pub content: String,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("path resolution failed: {0}")]
    Path(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
}

impl Serialize for SessionError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, SessionError> {
    crate::paths::app_data_dir(app).map_err(SessionError::Path)
}

fn sessions_index_path(app: &AppHandle) -> Result<PathBuf, SessionError> {
    Ok(app_data_dir(app)?.join(SESSIONS_FILE))
}

fn sessions_root(app: &AppHandle) -> Result<PathBuf, SessionError> {
    Ok(app_data_dir(app)?.join(SESSIONS_DIR))
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn file_call_id(id: &str) -> Result<String, SessionError> {
    let cleaned: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if cleaned.is_empty() {
        Err(SessionError::Path("invalid call id".into()))
    } else {
        Ok(cleaned)
    }
}

pub(crate) fn session_dir(app: &AppHandle, session_id: &str) -> Result<PathBuf, SessionError> {
    if !is_safe_id(session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    Ok(sessions_root(app)?.join(session_id))
}

fn empty_snapshot() -> SessionsSnapshot {
    let id = Uuid::new_v4().to_string();
    SessionsSnapshot {
        active_session_id: id.clone(),
        sessions: vec![SessionRecord {
            id,
            title: String::new(),
            preview: String::new(),
            updated_at: chrono::Utc::now().timestamp(),
            messages: Vec::new(),
            outside_workspace_allowed: false,
            http_write_allowed: false,
            todos: Vec::new(),
            todos_history: Vec::new(),
            workspace_path: None,
            parent_session_id: None,
            file_checkpoints: Vec::new(),
            redo: None,
        }],
    }
}

fn index_from_snapshot(snapshot: &SessionsSnapshot) -> SessionsIndex {
    SessionsIndex {
        active_session_id: snapshot.active_session_id.clone(),
        sessions: snapshot
            .sessions
            .iter()
            .filter(|session| session.parent_session_id.is_none())
            .map(|session| SessionIndexEntry {
                id: session.id.clone(),
                title: session.title.clone(),
                preview: session.preview.clone(),
                updated_at: session.updated_at,
            })
            .collect(),
    }
}

fn attachment_ext(name: &str, mime: &str) -> String {
    if let Some(ext) = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 8
                && value.chars().all(|ch| ch.is_ascii_alphanumeric())
        })
    {
        return format!(".{ext}");
    }
    match mime {
        "image/png" => ".png".into(),
        "image/jpeg" => ".jpg".into(),
        "image/gif" => ".gif".into(),
        "image/webp" => ".webp".into(),
        "application/pdf" => ".pdf".into(),
        "video/mp4" => ".mp4".into(),
        "video/webm" => ".webm".into(),
        "audio/mpeg" => ".mp3".into(),
        "audio/wav" => ".wav".into(),
        _ => String::new(),
    }
}

fn persist_attachment(dir: &Path, item: &mut ChatAttachment) -> Result<(), SessionError> {
    if item.data.is_empty() {
        return Ok(());
    }
    if !is_safe_id(&item.id) {
        return Err(SessionError::Path("invalid attachment id".into()));
    }
    let ext = attachment_ext(&item.name, &item.mime);
    let rel = format!("attachments/{}{ext}", item.id);
    let path = dir.join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SessionError::Io(e.to_string()))?;
    }
    let bytes = BASE64
        .decode(item.data.trim())
        .map_err(|e| SessionError::Io(e.to_string()))?;
    std::fs::write(&path, bytes).map_err(|e| SessionError::Io(e.to_string()))?;
    item.file = Some(rel);
    item.data.clear();
    Ok(())
}

fn strip_message_blobs(dir: &Path, message: &mut SessionMessage) -> Result<(), SessionError> {
    for item in &mut message.attachments {
        persist_attachment(dir, item)?;
    }
    Ok(())
}

fn write_session_record(app: &AppHandle, session: &mut SessionRecord) -> Result<(), SessionError> {
    if !is_safe_id(&session.id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let dir = session_dir(app, &session.id)?;
    std::fs::create_dir_all(&dir).map_err(|e| SessionError::Io(e.to_string()))?;
    for message in &mut session.messages {
        strip_message_blobs(&dir, message)?;
    }
    let path = dir.join(SESSION_FILE);
    let json =
        serde_json::to_string_pretty(session).map_err(|e| SessionError::Parse(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| SessionError::Io(e.to_string()))
}

pub fn begin_child_session(
    app: &AppHandle,
    parent_session_id: &str,
    title: &str,
    prompt: &str,
    workspace_path: Option<String>,
) -> Result<String, SessionError> {
    write_child_session(
        app,
        parent_session_id,
        title,
        prompt,
        "",
        "",
        "",
        Vec::new(),
        workspace_path,
    )
}

pub fn update_child_session(
    app: &AppHandle,
    id: &str,
    content: &str,
    reasoning: &str,
    reasoning_signature: &str,
    tool_rounds: Vec<crate::chat::ToolRoundTrace>,
) -> Result<(), SessionError> {
    let mut session = read_session_record(app, id)?;
    if let Some(assistant) = session
        .messages
        .iter_mut()
        .rev()
        .find(|message| message.role == "assistant")
    {
        assistant.content = content.to_string();
        assistant.reasoning = nonempty(reasoning.to_string());
        assistant.reasoning_signature = nonempty(reasoning_signature.to_string());
        assistant.tool_rounds = tool_rounds;
    }
    session.preview = clip_preview(content);
    session.updated_at = chrono::Utc::now().timestamp();
    write_session_record(app, &mut session)
}

pub fn write_child_session(
    app: &AppHandle,
    parent_session_id: &str,
    title: &str,
    prompt: &str,
    content: &str,
    reasoning: &str,
    reasoning_signature: &str,
    tool_rounds: Vec<crate::chat::ToolRoundTrace>,
    workspace_path: Option<String>,
) -> Result<String, SessionError> {
    if !is_safe_id(parent_session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let id = Uuid::new_v4().to_string();
    let preview = clip_preview(content);
    let user = empty_message(Uuid::new_v4().to_string(), "user", prompt.to_string());
    let mut assistant = empty_message(Uuid::new_v4().to_string(), "assistant", content.to_string());
    assistant.reasoning = nonempty(reasoning.to_string());
    assistant.reasoning_signature = nonempty(reasoning_signature.to_string());
    assistant.tool_rounds = tool_rounds;
    let mut session = SessionRecord {
        id: id.clone(),
        title: clip_title(title),
        preview,
        updated_at: chrono::Utc::now().timestamp(),
        messages: vec![user, assistant],
        outside_workspace_allowed: false,
        http_write_allowed: false,
        todos: Vec::new(),
        todos_history: Vec::new(),
        workspace_path,
        parent_session_id: Some(parent_session_id.to_string()),
        file_checkpoints: Vec::new(),
        redo: None,
    };
    write_session_record(app, &mut session)?;
    Ok(id)
}

fn nonempty(text: String) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn clip_title(title: &str) -> String {
    let trimmed = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= 60 {
        return trimmed;
    }
    let clipped: String = trimmed.chars().take(59).collect();
    format!("{clipped}...")
}

fn clip_preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 140 {
        return flat;
    }
    let clipped: String = flat.chars().take(139).collect();
    format!("{clipped}...")
}

fn empty_message(id: String, role: &str, content: String) -> SessionMessage {
    SessionMessage {
        id,
        role: role.to_string(),
        content,
        reasoning: None,
        reasoning_signature: None,
        thinking_ms: None,
        turn_ms: None,
        kind: None,
        shell_ai_summary: None,
        attachments: Vec::new(),
        tool_rounds: Vec::new(),
        pending_ask: None,
        resume_tools: false,
    }
}

pub(crate) fn read_session_record(
    app: &AppHandle,
    id: &str,
) -> Result<SessionRecord, SessionError> {
    let path = session_dir(app, id)?.join(SESSION_FILE);
    if !path.exists() {
        return Ok(SessionRecord {
            id: id.to_string(),
            title: String::new(),
            preview: String::new(),
            updated_at: chrono::Utc::now().timestamp(),
            messages: Vec::new(),
            outside_workspace_allowed: false,
            http_write_allowed: false,
            todos: Vec::new(),
            todos_history: Vec::new(),
            workspace_path: None,
            parent_session_id: None,
            file_checkpoints: Vec::new(),
            redo: None,
        });
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| SessionError::Io(e.to_string()))?;
    let mut session: SessionRecord =
        serde_json::from_str(&raw).map_err(|e| SessionError::Parse(e.to_string()))?;
    session.id = id.to_string();
    for message in &mut session.messages {
        for item in &mut message.attachments {
            item.data.clear();
        }
    }
    Ok(session)
}

fn write_index(app: &AppHandle, snapshot: &SessionsSnapshot) -> Result<(), SessionError> {
    let path = sessions_index_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SessionError::Io(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(&index_from_snapshot(snapshot))
        .map_err(|e| SessionError::Parse(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| SessionError::Io(e.to_string()))
}

fn prune_removed_session_dirs(
    app: &AppHandle,
    keep: &HashSet<String>,
    previously_listed: &HashSet<String>,
) -> Result<(), SessionError> {
    for id in previously_listed {
        if keep.contains(id) || !is_safe_id(id) {
            continue;
        }
        let path = session_dir(app, id)?;
        let _ = std::fs::remove_dir_all(path);
    }
    Ok(())
}

fn save_snapshot_sync(
    app: &AppHandle,
    snapshot: &mut SessionsSnapshot,
) -> Result<(), SessionError> {
    if snapshot.sessions.is_empty() {
        return Err(SessionError::Parse("sessions must not be empty".into()));
    }
    prefer_root(snapshot);
    if !snapshot
        .sessions
        .iter()
        .any(|session| session.id == snapshot.active_session_id)
    {
        return Err(SessionError::Parse("active session id not found".into()));
    }
    let keep: HashSet<String> = snapshot
        .sessions
        .iter()
        .map(|session| session.id.clone())
        .collect();
    let previously_listed: HashSet<String> = read_thin_index(app)
        .map(|index| index.sessions.into_iter().map(|entry| entry.id).collect())
        .unwrap_or_default();
    for session in &mut snapshot.sessions {
        write_session_record(app, session)?;
    }
    write_index(app, snapshot)?;
    prune_removed_session_dirs(app, &keep, &previously_listed)?;
    delete_orphan_children(app, &keep)?;
    Ok(())
}

fn delete_orphan_children(app: &AppHandle, live_ids: &HashSet<String>) -> Result<(), SessionError> {
    let root = sessions_root(app)?;
    if !root.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(&root).map_err(|e| SessionError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| SessionError::Io(e.to_string()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_safe_id(&name) {
            continue;
        }
        let Ok(session) = read_session_record(app, &name) else {
            continue;
        };
        let Some(parent) = session.parent_session_id else {
            continue;
        };
        if live_ids.contains(&parent) {
            continue;
        }
        let _ = std::fs::remove_dir_all(entry.path());
    }
    Ok(())
}

fn prefer_root(snapshot: &mut SessionsSnapshot) {
    let active_is_child = snapshot.sessions.iter().any(|session| {
        session.id == snapshot.active_session_id && session.parent_session_id.is_some()
    });
    if !active_is_child {
        return;
    }
    if let Some(root) = snapshot
        .sessions
        .iter()
        .find(|session| session.parent_session_id.is_none())
    {
        snapshot.active_session_id = root.id.clone();
    }
}

fn load_from_session_dirs(app: &AppHandle) -> Result<SessionsSnapshot, SessionError> {
    let root = sessions_root(app)?;
    if !root.exists() {
        return Ok(empty_snapshot());
    }
    let entries = std::fs::read_dir(&root).map_err(|e| SessionError::Io(e.to_string()))?;
    let mut sessions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| SessionError::Io(e.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|e| SessionError::Io(e.to_string()))?;
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_safe_id(&name) {
            continue;
        }
        match read_session_record(app, &name) {
            Ok(session) => sessions.push(session),
            Err(_) => continue,
        }
    }
    if sessions.is_empty() {
        return Ok(empty_snapshot());
    }
    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    let active_session_id = sessions[0].id.clone();
    Ok(SessionsSnapshot {
        active_session_id,
        sessions,
    })
}

fn read_thin_index(app: &AppHandle) -> Option<SessionsIndex> {
    let path = sessions_index_path(app).ok()?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > INDEX_MAX_BYTES {
        return None;
    }
    let raw = std::fs::read_to_string(&path).ok()?;
    if raw.contains("\"messages\"") {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

fn load_snapshot_sync(app: &AppHandle) -> Result<SessionsSnapshot, SessionError> {
    let mut snapshot = load_from_session_dirs(app)?;
    if snapshot.sessions.is_empty() {
        return Ok(snapshot);
    }
    if let Some(index) = read_thin_index(app) {
        if snapshot
            .sessions
            .iter()
            .any(|session| session.id == index.active_session_id)
        {
            snapshot.active_session_id = index.active_session_id;
        }
    }
    prefer_root(&mut snapshot);
    Ok(snapshot)
}

fn session_is_blank(session: &SessionRecord) -> bool {
    session.messages.is_empty() && session.title.trim().is_empty()
}

pub(crate) fn drop_blank_sessions(app: &AppHandle) -> Result<(), String> {
    let mut snapshot = load_snapshot_sync(app).map_err(|error| error.to_string())?;
    let active = snapshot.active_session_id.clone();
    let removed: Vec<String> = snapshot
        .sessions
        .iter()
        .filter(|session| session.id != active && session_is_blank(session))
        .map(|session| session.id.clone())
        .collect();
    if removed.is_empty() {
        return Ok(());
    }
    snapshot
        .sessions
        .retain(|session| !removed.iter().any(|id| id == &session.id));
    if snapshot.sessions.is_empty() || !snapshot.sessions.iter().any(|session| session.id == active)
    {
        return Ok(());
    }
    save_snapshot_sync(app, &mut snapshot).map_err(|error| error.to_string())?;
    for id in removed {
        if !is_safe_id(&id) {
            continue;
        }
        if let Ok(path) = session_dir(app, &id) {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn read_session(
    app: AppHandle,
    session_id: String,
) -> Result<SessionRecord, SessionError> {
    if !is_safe_id(&session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || read_session_record(&handle, &session_id))
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?
}

fn safe_rel_path(rel: &str) -> Result<&str, SessionError> {
    let unified = rel.replace('\\', "/");
    if unified.is_empty()
        || unified.starts_with('/')
        || unified.contains("..")
        || unified.starts_with("../")
    {
        return Err(SessionError::Path("invalid session file path".into()));
    }
    if !(unified.starts_with("attachments/") || unified.starts_with("files/")) {
        return Err(SessionError::Path("invalid session file path".into()));
    }
    Ok(rel)
}

pub fn write_file_revision(
    app: &AppHandle,
    session_id: &str,
    call_id: &str,
    before: &str,
    after: &str,
) -> Result<(), SessionError> {
    if !is_safe_id(session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let call_id = file_call_id(call_id)?;
    let dir = session_dir(app, session_id)?.join("files");
    std::fs::create_dir_all(&dir).map_err(|e| SessionError::Io(e.to_string()))?;
    std::fs::write(dir.join(format!("{call_id}.before")), before)
        .map_err(|e| SessionError::Io(e.to_string()))?;
    std::fs::write(dir.join(format!("{call_id}.after")), after)
        .map_err(|e| SessionError::Io(e.to_string()))?;
    Ok(())
}

pub fn hydrate_attachment(
    app: &AppHandle,
    session_id: &str,
    item: &mut ChatAttachment,
) -> Result<(), SessionError> {
    if !item.data.is_empty() {
        return Ok(());
    }
    let Some(rel) = item.file.as_deref().filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let path = session_dir(app, session_id)?.join(safe_rel_path(rel)?);
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| SessionError::Io(e.to_string()))?;
    item.data = BASE64.encode(bytes);
    Ok(())
}

pub fn hydrate_attachments(
    app: &AppHandle,
    session_id: Option<&str>,
    items: &mut [ChatAttachment],
) -> Result<(), SessionError> {
    let Some(session_id) = session_id.filter(|id| is_safe_id(id)) else {
        return Ok(());
    };
    for item in items {
        hydrate_attachment(app, session_id, item)?;
    }
    Ok(())
}

async fn load_snapshot(app: &AppHandle) -> Result<SessionsSnapshot, SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || load_snapshot_sync(&handle))
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?
}

async fn save_snapshot(
    app: &AppHandle,
    mut snapshot: SessionsSnapshot,
) -> Result<(), SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || save_snapshot_sync(&handle, &mut snapshot))
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?
}

#[tauri::command]
pub async fn load_sessions(app: AppHandle) -> Result<SessionsSnapshot, SessionError> {
    load_snapshot(&app).await
}

pub fn read_session_todos(
    app: &AppHandle,
    session_id: &str,
) -> Result<Vec<crate::tools::todo::TodoItem>, SessionError> {
    if !is_safe_id(session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let session = read_session_record(app, session_id)?;
    Ok(session.todos)
}

pub fn append_session_todo_event(
    app: &AppHandle,
    session_id: &str,
    todos: Vec<crate::tools::todo::TodoItem>,
    event: crate::tools::todo::TodoHistoryEvent,
) -> Result<(), SessionError> {
    if !is_safe_id(session_id) {
        return Err(SessionError::Path("invalid session id".into()));
    }
    let dir = session_dir(app, session_id)?;
    std::fs::create_dir_all(&dir).map_err(|e| SessionError::Io(e.to_string()))?;
    let mut session = read_session_record(app, session_id)?;
    session.todos = todos;
    session.todos_history.push(event);
    let cap = crate::tools::todo::HISTORY_CAP;
    if session.todos_history.len() > cap {
        let drop = session.todos_history.len() - cap;
        session.todos_history.drain(0..drop);
    }
    session.updated_at = chrono::Utc::now().timestamp();
    write_session_record(app, &mut session)?;
    refresh_index_entry(app, &session)
}

fn refresh_index_entry(app: &AppHandle, session: &SessionRecord) -> Result<(), SessionError> {
    let path = sessions_index_path(app)?;
    let mut index = match std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<SessionsIndex>(&raw).ok())
    {
        Some(existing) => existing,
        None => SessionsIndex {
            active_session_id: session.id.clone(),
            sessions: Vec::new(),
        },
    };
    let mut replaced = false;
    for entry in index.sessions.iter_mut() {
        if entry.id == session.id {
            entry.title = session.title.clone();
            entry.preview = session.preview.clone();
            entry.updated_at = session.updated_at;
            replaced = true;
            break;
        }
    }
    if !replaced {
        index.sessions.push(SessionIndexEntry {
            id: session.id.clone(),
            title: session.title.clone(),
            preview: session.preview.clone(),
            updated_at: session.updated_at,
        });
    }
    let json =
        serde_json::to_string_pretty(&index).map_err(|e| SessionError::Parse(e.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| SessionError::Io(e.to_string()))?;
    }
    std::fs::write(&path, json).map_err(|e| SessionError::Io(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn save_sessions(app: AppHandle, snapshot: SessionsSnapshot) -> Result<(), SessionError> {
    save_snapshot(&app, snapshot).await
}

#[tauri::command]
pub async fn read_session_attachment(
    app: AppHandle,
    input: ReadSessionAttachmentInput,
) -> Result<ChatAttachment, SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = read_session_record(&handle, &input.session_id)?;
        let mut item = session
            .messages
            .iter()
            .find_map(|message| {
                message
                    .attachments
                    .iter()
                    .find(|item| item.id == input.attachment_id)
            })
            .cloned()
            .ok_or_else(|| SessionError::Path("attachment not found".into()))?;
        hydrate_attachment(&handle, &input.session_id, &mut item)?;
        Ok(item)
    })
    .await
    .map_err(|e| SessionError::Io(e.to_string()))?
}

#[tauri::command]
pub async fn read_session_file_revision(
    app: AppHandle,
    input: ReadSessionFileRevisionInput,
) -> Result<SessionFileRevision, SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let call_id = file_call_id(&input.call_id)?;
        let side = input.side.trim().to_ascii_lowercase();
        if side != "before" && side != "after" {
            return Err(SessionError::Parse("side must be before or after".into()));
        }
        let rel = format!("files/{call_id}.{side}");
        let path = session_dir(&handle, &input.session_id)?.join(safe_rel_path(&rel)?);
        let content = if path.exists() {
            std::fs::read_to_string(&path).map_err(|e| SessionError::Io(e.to_string()))?
        } else {
            String::new()
        };
        Ok(SessionFileRevision { content })
    })
    .await
    .map_err(|e| SessionError::Io(e.to_string()))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackFileInput {
    pub path: String,
    pub checkpoint_id: String,
    pub side: String,
    pub expect_hash: String,
    pub restore_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackFilesInput {
    pub session_id: String,
    pub files: Vec<RollbackFileInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackFileOutcome {
    pub path: String,
    pub restored: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackFilesResult {
    pub files: Vec<RollbackFileOutcome>,
}

#[tauri::command]
pub async fn rollback_files(
    app: AppHandle,
    input: RollbackFilesInput,
) -> Result<RollbackFilesResult, SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || rollback_files_sync(&handle, input))
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?
}

pub(crate) fn seal_turn(
    app: &AppHandle,
    session_id: &str,
    noted: Vec<crate::checkpoints::NotedFile>,
) -> Result<Option<(String, Vec<FileTouch>)>, SessionError> {
    let Some(sealed) = crate::checkpoints::seal(&session_dir(app, session_id)?, noted)
        .map_err(SessionError::Io)?
    else {
        return Ok(None);
    };
    let files = sealed
        .files
        .into_iter()
        .map(|file| FileTouch {
            path: file.path,
            before_hash: file.before_hash,
            after_hash: file.after_hash,
        })
        .collect();
    Ok(Some((sealed.checkpoint_id, files)))
}

fn rollback_files_sync(
    app: &AppHandle,
    input: RollbackFilesInput,
) -> Result<RollbackFilesResult, SessionError> {
    let workspace = crate::pathutil::workspace_from_app(app);
    let mut files = Vec::with_capacity(input.files.len());
    for item in input.files {
        let restored = restore_one(app, &input.session_id, workspace.as_deref(), &item).is_ok();
        files.push(RollbackFileOutcome {
            path: item.path,
            restored,
        });
    }
    Ok(RollbackFilesResult { files })
}

fn restore_one(
    app: &AppHandle,
    session_id: &str,
    workspace: Option<&Path>,
    item: &RollbackFileInput,
) -> Result<(), SessionError> {
    let resolved =
        crate::pathutil::resolve_tool_path(&item.path, workspace).map_err(SessionError::Path)?;
    let session = session_dir(app, session_id)?;
    crate::checkpoints::restore_at(
        &session,
        &resolved,
        &item.checkpoint_id,
        &item.path,
        &item.side,
        &item.expect_hash,
        &item.restore_hash,
    )
    .map_err(SessionError::Path)
}
