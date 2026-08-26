use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

const SESSIONS_FILE: &str = "sessions.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionToolCall {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

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
    pub attachments: Vec<crate::attachments::ChatAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<SessionToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub updated_at: i64,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsSnapshot {
    pub active_session_id: String,
    pub sessions: Vec<SessionRecord>,
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

fn sessions_path(app: &AppHandle) -> Result<PathBuf, SessionError> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(SESSIONS_FILE))
        .map_err(|e| SessionError::Path(e.to_string()))
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

async fn load_snapshot(app: &AppHandle) -> Result<SessionsSnapshot, SessionError> {
    let path = sessions_path(app)?;
    if !path.exists() {
        return Ok(empty_snapshot());
    }
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| SessionError::Io(e.to_string()))?;
    let mut snapshot: SessionsSnapshot =
        serde_json::from_str(&raw).map_err(|e| SessionError::Parse(e.to_string()))?;
    if snapshot.sessions.is_empty() {
        return Ok(empty_snapshot());
    }
    if !snapshot
        .sessions
        .iter()
        .any(|session| session.id == snapshot.active_session_id)
    {
        snapshot.active_session_id = snapshot.sessions[0].id.clone();
    }
    Ok(snapshot)
}

async fn save_snapshot(app: &AppHandle, snapshot: &SessionsSnapshot) -> Result<(), SessionError> {
    let path = sessions_path(app)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(snapshot).map_err(|e| SessionError::Parse(e.to_string()))?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| SessionError::Io(e.to_string()))
}

#[tauri::command]
pub async fn load_sessions(app: AppHandle) -> Result<SessionsSnapshot, SessionError> {
    load_snapshot(&app).await
}

#[tauri::command]
pub async fn save_sessions(app: AppHandle, snapshot: SessionsSnapshot) -> Result<(), SessionError> {
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
    save_snapshot(&app, &snapshot).await
}
