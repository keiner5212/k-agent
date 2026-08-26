use std::io::{Cursor, Read};
use std::path::Path;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zip::ZipArchive;

const MAX_ATTACHMENTS: usize = 8;
const IMAGE_MAX: u64 = 20 * 1024 * 1024;
const PDF_MAX: u64 = 32 * 1024 * 1024;
const VIDEO_MAX: u64 = 20 * 1024 * 1024;
const AUDIO_MAX: u64 = 20 * 1024 * 1024;
const TEXT_MAX: u64 = 2 * 1024 * 1024;
const DOCUMENT_MAX: u64 = 15 * 1024 * 1024;
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAttachmentsInput {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub blobs: Vec<AttachmentBlob>,
    #[serde(default)]
    pub allowed_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentBlob {
    pub name: String,
    #[serde(default)]
    pub mime: Option<String>,
    pub data: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedAttachments {
    pub attachments: Vec<ChatAttachment>,
    pub errors: Vec<AttachmentPrepareError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPrepareError {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Error)]
pub enum AttachmentError {
    #[error("{0}")]
    Message(String),
}

impl Serialize for AttachmentError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

#[tauri::command]
pub async fn prepare_chat_attachments(
    input: PrepareAttachmentsInput,
) -> Result<PreparedAttachments, AttachmentError> {
    tauri::async_runtime::spawn_blocking(move || prepare_sync(input))
        .await
        .map_err(|error| AttachmentError::Message(error.to_string()))?
}

fn prepare_sync(input: PrepareAttachmentsInput) -> Result<PreparedAttachments, AttachmentError> {
    let allowed: Vec<String> = input
        .allowed_types
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect();
    if allowed.is_empty() {
        return Err(AttachmentError::Message("unsupported".into()));
    }
    let mut attachments = Vec::new();
    let mut errors = Vec::new();
    for path in input.paths {
        if attachments.len() >= MAX_ATTACHMENTS {
            errors.push(error_for(&file_name(&path), "tooMany"));
            continue;
        }
        match read_path(&path, &allowed) {
            Ok(item) => attachments.push(item),
            Err(code) => errors.push(error_for(&file_name(&path), code)),
        }
    }
    for blob in input.blobs {
        if attachments.len() >= MAX_ATTACHMENTS {
            errors.push(error_for(&blob.name, "tooMany"));
            continue;
        }
        match read_blob(&blob, &allowed) {
            Ok(item) => attachments.push(item),
            Err(code) => errors.push(error_for(&blob.name, code)),
        }
    }
    Ok(PreparedAttachments {
        attachments,
        errors,
    })
}

fn error_for(name: &str, code: &str) -> AttachmentPrepareError {
    AttachmentPrepareError {
        name: name.to_string(),
        code: code.to_string(),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn read_path(path: &str, allowed: &[String]) -> Result<ChatAttachment, &'static str> {
    let name = file_name(path);
    let bytes = std::fs::read(path).map_err(|_| "unreadable")?;
    build_attachment(&name, None, bytes, allowed)
}

fn read_blob(blob: &AttachmentBlob, allowed: &[String]) -> Result<ChatAttachment, &'static str> {
    let bytes = BASE64.decode(blob.data.trim()).map_err(|_| "unreadable")?;
    build_attachment(&blob.name, blob.mime.as_deref(), bytes, allowed)
}

fn build_attachment(
    name: &str,
    mime_hint: Option<&str>,
    bytes: Vec<u8>,
    allowed: &[String],
) -> Result<ChatAttachment, &'static str> {
    let fallback = if name.trim().is_empty() {
        "clipboard.png"
    } else {
        name
    };
    let (kind, mime) = classify(fallback, mime_hint).ok_or("unsupported")?;
    if !allowed.iter().any(|item| item == kind) {
        return Err("unsupported");
    }
    if bytes.len() as u64 > max_bytes(kind) {
        return Err("tooLarge");
    }
    let text = match kind {
        "text" => Some(String::from_utf8_lossy(&bytes).into_owned()),
        "document" => Some(extract_docx_text(&bytes).ok_or("extractFailed")?),
        _ => None,
    };
    let data = if kind == "text" || kind == "document" {
        String::new()
    } else {
        BASE64.encode(&bytes)
    };
    Ok(ChatAttachment {
        id: Uuid::new_v4().to_string(),
        name: fallback.to_string(),
        mime: mime.to_string(),
        kind: kind.to_string(),
        data,
        text,
        file: None,
    })
}

fn max_bytes(kind: &str) -> u64 {
    match kind {
        "image" => IMAGE_MAX,
        "pdf" => PDF_MAX,
        "video" => VIDEO_MAX,
        "audio" => AUDIO_MAX,
        "text" => TEXT_MAX,
        "document" => DOCUMENT_MAX,
        _ => TEXT_MAX,
    }
}

fn classify(name: &str, mime_hint: Option<&str>) -> Option<(&'static str, &'static str)> {
    if let Some(mime) = mime_hint.map(str::trim).filter(|item| !item.is_empty()) {
        if let Some(pair) = kind_from_mime(mime) {
            return Some(pair);
        }
    }
    kind_from_ext(name)
}

fn kind_from_mime(mime: &str) -> Option<(&'static str, &'static str)> {
    let lower = mime.to_ascii_lowercase();
    match lower.as_str() {
        "image/png" => Some(("image", "image/png")),
        "image/jpeg" | "image/jpg" => Some(("image", "image/jpeg")),
        "image/gif" => Some(("image", "image/gif")),
        "image/webp" => Some(("image", "image/webp")),
        "application/pdf" => Some(("pdf", "application/pdf")),
        "video/mp4" => Some(("video", "video/mp4")),
        "video/webm" => Some(("video", "video/webm")),
        "video/quicktime" => Some(("video", "video/quicktime")),
        "video/mpeg" | "video/mpg" => Some(("video", "video/mpeg")),
        "video/x-msvideo" | "video/avi" => Some(("video", "video/x-msvideo")),
        "video/x-ms-wmv" | "video/wmv" => Some(("video", "video/x-ms-wmv")),
        "video/3gpp" => Some(("video", "video/3gpp")),
        "audio/mpeg" | "audio/mp3" => Some(("audio", "audio/mpeg")),
        "audio/wav" | "audio/x-wav" => Some(("audio", "audio/wav")),
        "audio/webm" => Some(("audio", "audio/webm")),
        "audio/mp4" | "audio/aac" => Some(("audio", "audio/mp4")),
        "text/plain" => Some(("text", "text/plain")),
        "text/markdown" => Some(("text", "text/markdown")),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some(("document", DOCX_MIME))
        }
        _ if lower.starts_with("image/") => None,
        _ => None,
    }
}

fn kind_from_ext(name: &str) -> Option<(&'static str, &'static str)> {
    let ext = Path::new(name)
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(("image", "image/png")),
        "jpg" | "jpeg" => Some(("image", "image/jpeg")),
        "gif" => Some(("image", "image/gif")),
        "webp" => Some(("image", "image/webp")),
        "pdf" => Some(("pdf", "application/pdf")),
        "mp4" => Some(("video", "video/mp4")),
        "webm" => Some(("video", "video/webm")),
        "mov" => Some(("video", "video/quicktime")),
        "mpeg" | "mpg" => Some(("video", "video/mpeg")),
        "avi" => Some(("video", "video/x-msvideo")),
        "wmv" => Some(("video", "video/x-ms-wmv")),
        "3gp" => Some(("video", "video/3gpp")),
        "mp3" => Some(("audio", "audio/mpeg")),
        "wav" => Some(("audio", "audio/wav")),
        "m4a" | "aac" => Some(("audio", "audio/mp4")),
        "txt" => Some(("text", "text/plain")),
        "md" => Some(("text", "text/markdown")),
        "docx" => Some(("document", DOCX_MIME)),
        _ => None,
    }
}

fn extract_docx_text(bytes: &[u8]) -> Option<String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut file = archive.by_name("word/document.xml").ok()?;
    let mut xml = String::new();
    file.read_to_string(&mut xml).ok()?;
    Some(docx_plain_text(&xml))
}

fn docx_plain_text(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:t") {
        rest = &rest[start + 4..];
        let Some(gt) = rest.find('>') else { break };
        rest = &rest[gt + 1..];
        let Some(end) = rest.find("</w:t>") else {
            break;
        };
        let chunk = decode_xml(&rest[..end]);
        if !chunk.is_empty() {
            if !out.is_empty() && !out.ends_with([' ', '\n']) {
                out.push(' ');
            }
            out.push_str(&chunk);
        }
        rest = &rest[end + 6..];
    }
    out
}

fn decode_xml(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
