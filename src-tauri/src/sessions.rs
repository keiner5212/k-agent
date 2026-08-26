use std::collections::HashSet;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
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
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_ai_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_rounds: Vec<crate::chat::ToolRoundTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<SessionMessage>,
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
    app.path()
        .app_data_dir()
        .map_err(|e| SessionError::Path(e.to_string()))
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

fn session_dir(app: &AppHandle, session_id: &str) -> Result<PathBuf, SessionError> {
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
        }],
    }
}

fn index_from_snapshot(snapshot: &SessionsSnapshot) -> SessionsIndex {
    SessionsIndex {
        active_session_id: snapshot.active_session_id.clone(),
        sessions: snapshot
            .sessions
            .iter()
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
            !value.is_empty() && value.len() <= 8 && value.chars().all(|ch| ch.is_ascii_alphanumeric())
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

fn read_session_record(app: &AppHandle, id: &str) -> Result<SessionRecord, SessionError> {
    let path = session_dir(app, id)?.join(SESSION_FILE);
    if !path.exists() {
        return Ok(SessionRecord {
            id: id.to_string(),
            title: String::new(),
            preview: String::new(),
            updated_at: chrono::Utc::now().timestamp(),
            messages: Vec::new(),
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

fn save_snapshot_sync(app: &AppHandle, snapshot: &mut SessionsSnapshot) -> Result<(), SessionError> {
    if snapshot.sessions.is_empty() {
        return Err(SessionError::Parse("sessions must not be empty".into()));
    }
    if !snapshot
        .sessions
        .iter()
        .any(|session| session.id == snapshot.active_session_id)
    {
        return Err(SessionError::Parse("active session id not found".into()));
    }
    let keep: HashSet<String> = snapshot.sessions.iter().map(|session| session.id.clone()).collect();
    let previously_listed: HashSet<String> = read_thin_index(app)
        .map(|index| index.sessions.into_iter().map(|entry| entry.id).collect())
        .unwrap_or_default();
    for session in &mut snapshot.sessions {
        write_session_record(app, session)?;
    }
    write_index(app, snapshot)?;
    prune_removed_session_dirs(app, &keep, &previously_listed)?;
    Ok(())
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
        let file_type = entry.file_type().map_err(|e| SessionError::Io(e.to_string()))?;
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
    Ok(snapshot)
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

async fn save_snapshot(app: &AppHandle, mut snapshot: SessionsSnapshot) -> Result<(), SessionError> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || save_snapshot_sync(&handle, &mut snapshot))
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?
}

#[tauri::command]
pub async fn load_sessions(app: AppHandle) -> Result<SessionsSnapshot, SessionError> {
    load_snapshot(&app).await
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
