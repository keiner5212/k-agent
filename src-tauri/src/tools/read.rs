use std::fs;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::ImageFormat;
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "read";

const DESCRIPTION: &str = "Read a file. Text stays line-numbered text. An image is attached only when the model accepts image input. A PDF is attached only when the model accepts pdf input. A docx is extracted to text when the model accepts documents. Path is absolute or workspace-relative. Paths outside the workspace wait for the user. Optional offset and limit for text. Text capped at 50 KB. Images capped at 20 MB.";

const DEFAULT_LIMIT: usize = 2000;
const MAX_LINE_LENGTH: usize = 2000;
const MAX_LINE_SUFFIX: &str = "... (line truncated to 2000 chars)";
const MAX_BYTES: usize = 50 * 1024;
const MAX_BYTES_LABEL: &str = "50 KB";

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_BYTES_LABEL: &str = "20 MB";
const MAX_IMAGE_DIMENSION: u32 = 1440;
const IMAGE_PROBE_BYTES: usize = 16;

pub struct ReadTool;

impl Tool for ReadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path"
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "1-based start line. Text files only."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Max lines (default 2000). Text files only."
                    }
                },
                "required": ["filePath"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(raw_path) = args.get("filePath").and_then(Value::as_str) else {
            return super::context_error(None, "read tool requires a string `filePath`.");
        };
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return super::context_error(None, "read tool `filePath` is empty.");
        }
        let offset = match parse_usize_arg(args, "offset", 1, 1) {
            Ok(value) => value,
            Err(message) => return super::context_error(Some(trimmed), &message),
        };
        let limit = match parse_usize_arg(args, "limit", DEFAULT_LIMIT, 1) {
            Ok(value) => value,
            Err(message) => return super::context_error(Some(trimmed), &message),
        };
        let page_requested = args.get("offset").is_some() || args.get("limit").is_some();

        let resolved = match resolve_path(ctx, trimmed) {
            Ok(value) => value,
            Err(message) => return super::context_error(Some(trimmed), &message),
        };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &resolved,
            ctx.workspace_path().as_deref(),
        ) {
            return super::context_error(
                Some(trimmed),
                "read outside the workspace must run on the async dispatch path.",
            );
        }
        let rel = ctx.relative_path(&resolved);
        let metadata = match fs::metadata(&resolved) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let suggestions = fuzzy_sibling_suggestions(&resolved);
                let body = if suggestions.is_empty() {
                    format!("File not found: {}", resolved.display())
                } else {
                    format!(
                        "File not found: {}\nDid you mean one of these?\n{}",
                        resolved.display(),
                        suggestions.join("\n")
                    )
                };
                return super::context_error(Some(&rel), &body);
            }
            Err(error) => {
                return super::context_error(
                    Some(&rel),
                    &format!("Unable to stat `{}`: {error}", resolved.display()),
                );
            }
        };

        let file_type = metadata.file_type();
        if file_type.is_dir() {
            return super::context_error(Some(&rel), "Path is a directory. Use list_directory.");
        }
        if !file_type.is_file() {
            return super::context_error(
                Some(&rel),
                &format!(
                    "Cannot read `{}`: not a regular file or directory.",
                    resolved.display()
                ),
            );
        }

        if !page_requested {
            if let Some(image_kind) = detect_image(&resolved) {
                if model_accepts(ctx, "image") {
                    return render_image(&resolved, &rel, image_kind);
                }
                return super::context_error(
                    Some(&rel),
                    "This file is an image. The selected model has no image input, so the pixels were not attached.",
                );
            }
            if is_pdf(&resolved) {
                if model_accepts(ctx, "pdf") {
                    return render_pdf(&resolved, &rel);
                }
                return super::context_error(
                    Some(&rel),
                    "This file is a PDF. The selected model has no pdf input, so the file was not attached.",
                );
            }
            if is_docx(&resolved) {
                if model_accepts(ctx, "document") {
                    return render_docx(&resolved, &rel);
                }
                return super::context_error(
                    Some(&rel),
                    "This file is a document. The selected model has no document input.",
                );
            }
        }

        if is_disguised_binary_extension(&resolved) {
            return super::context_error(
                Some(&rel),
                &format!("Cannot read binary file: {}", resolved.display()),
            );
        }

        render_text(&resolved, &rel, offset, limit)
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let raw = args
        .get("filePath")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    super::tool_utils::workspace::guard(ctx, raw, "Read", false, || ReadTool.execute(&args, ctx))
        .await
}

fn parse_usize_arg(
    args: &Value,
    key: &str,
    default_value: usize,
    min_value: usize,
) -> Result<usize, String> {
    let Some(value) = args.get(key) else {
        return Ok(default_value);
    };
    let Some(number) = value.as_u64() else {
        return Err(format!("read tool `{key}` must be a non-negative integer."));
    };
    let parsed =
        usize::try_from(number).map_err(|_| format!("read tool `{key}` is out of range."))?;
    if parsed < min_value {
        return Err(format!("read tool `{key}` must be at least {min_value}."));
    }
    Ok(parsed)
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

fn fuzzy_sibling_suggestions(path: &Path) -> Vec<String> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let Some(target) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let lower_target = target.to_ascii_lowercase();
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut hits: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower_name = name.to_ascii_lowercase();
            if lower_name.contains(&lower_target) || lower_target.contains(&lower_name) {
                Some(parent.join(name).to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .take(3)
        .collect();
    hits.sort();
    hits
}

fn render_text(path: &Path, rel: &str, offset: usize, limit: usize) -> ToolOutcome {
    let outcome = read_windowed(path, offset, limit);
    let result = match outcome {
        Ok(value) => value,
        Err(error) => {
            let message = if error.kind() == std::io::ErrorKind::InvalidData {
                format!("Cannot read binary file: {}", path.display())
            } else {
                format!("Unable to read `{}`: {error}", path.display())
            };
            return super::context_error(Some(rel), &message);
        }
    };
    if result.raw.is_empty() && offset > 1 && result.total_lines < offset {
        return super::context_error(
            Some(rel),
            &format!(
                "Offset {offset} is out of range for this file ({} lines).",
                result.total_lines
            ),
        );
    }
    let start_line = offset as u32;
    let end_line = if result.raw.is_empty() {
        start_line
    } else {
        (offset + result.raw.len() - 1) as u32
    };
    let mut content = String::new();
    for (index, line) in result.raw.iter().enumerate() {
        let line_number = offset + index;
        content.push_str(&format!("{line_number}: {line}\n"));
    }
    let last = offset + result.raw.len().saturating_sub(1);
    if result.capped {
        let next = last + 1;
        content.push_str(&format!(
            "\n(Output capped at {MAX_BYTES_LABEL}. Showing lines {offset}-{last}. Use offset={next} to continue.)"
        ));
    } else if result.more {
        let next = last + 1;
        content.push_str(&format!(
            "\n(Showing lines {offset}-{last} of {}. Use offset={next} to continue.)",
            result.total_lines
        ));
    } else {
        content.push_str(&format!(
            "\n(End of file - total {} lines)",
            result.total_lines
        ));
    }
    let content = content.trim_end();
    ToolOutcome {
        text: toon_doc(&[
            ("path", ToonValue::Str(rel)),
            ("startLine", ToonValue::Int(start_line as i64)),
            ("endLine", ToonValue::Int(end_line as i64)),
            ("content", ToonValue::Block(content)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(rel.to_string()),
            start_line: Some(start_line),
            end_line: Some(end_line),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn render_image(path: &Path, rel: &str, kind: ImageKind) -> ToolOutcome {
    let metadata = match fs::metadata(path) {
        Ok(value) => value,
        Err(error) => {
            return super::context_error(
                Some(rel),
                &format!("Unable to stat `{}`: {error}", path.display()),
            );
        }
    };
    if metadata.len() > MAX_IMAGE_BYTES {
        return super::context_error(
            Some(rel),
            &format!(
                "Image exceeds {MAX_IMAGE_BYTES_LABEL} ingestion limit ({} bytes): {}",
                metadata.len(),
                path.display()
            ),
        );
    }
    let bytes = match fs::read(path) {
        Ok(value) => value,
        Err(error) => {
            return super::context_error(
                Some(rel),
                &format!("Unable to read `{}`: {error}", path.display()),
            );
        }
    };
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return super::context_error(
            Some(rel),
            &format!(
                "Image exceeds {MAX_IMAGE_BYTES_LABEL} ingestion limit ({} bytes): {}",
                bytes.len(),
                path.display()
            ),
        );
    }
    let image = match image::load_from_memory(&bytes) {
        Ok(value) => value,
        Err(error) => {
            let text = format!(
                "Image could not be decoded as {} ({}): {}",
                kind.label(),
                error,
                path.display()
            );
            return super::context_error(Some(rel), &text);
        }
    };
    let original = (image.width(), image.height());
    let thumb = image.thumbnail(MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION);
    let (scaled_w, scaled_h) = (thumb.width(), thumb.height());
    let mut encoded = Vec::with_capacity(bytes.len().min(64 * 1024));
    if let Err(error) = thumb.write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png) {
        return super::context_error(
            Some(rel),
            &format!("Failed to encode image as PNG: {error}"),
        );
    }
    let encoded_b64 = BASE64.encode(&encoded);
    let mime = kind.mime();
    ToolOutcome {
        text: toon_doc(&[
            ("path", ToonValue::Str(rel)),
            ("mime", ToonValue::Str(mime)),
            ("bytes", ToonValue::Int(encoded.len() as i64)),
            ("width", ToonValue::Int(scaled_w as i64)),
            ("height", ToonValue::Int(scaled_h as i64)),
            ("originalWidth", ToonValue::Int(original.0 as i64)),
            ("originalHeight", ToonValue::Int(original.1 as i64)),
            ("image", ToonValue::Str("png attached")),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(rel.to_string()),
            status: Some("ok".into()),
            image_data: Some(encoded_b64),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: Some(encoded),
        file: None,
    }
}

fn is_disguised_binary_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext.is_empty() {
        return false;
    }
    matches!(
        ext.as_str(),
        "zip"
            | "tar"
            | "gz"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "class"
            | "jar"
            | "war"
            | "7z"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "odt"
            | "ods"
            | "odp"
            | "bin"
            | "dat"
            | "obj"
            | "o"
            | "a"
            | "lib"
            | "wasm"
            | "pyc"
            | "pyo"
    )
}

#[derive(Debug, Clone, Copy)]
enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageKind {
    fn label(self) -> &'static str {
        match self {
            ImageKind::Png => "PNG",
            ImageKind::Jpeg => "JPEG",
            ImageKind::Gif => "GIF",
            ImageKind::Webp => "WebP",
        }
    }

    fn mime(self) -> &'static str {
        match self {
            ImageKind::Png => "image/png",
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Gif => "image/gif",
            ImageKind::Webp => "image/webp",
        }
    }
}

fn model_accepts(ctx: &ToolContext<'_>, kind: &str) -> bool {
    ctx.input_modalities
        .iter()
        .chain(ctx.attachment_types.iter())
        .any(|item| item.eq_ignore_ascii_case(kind))
}

fn is_pdf(path: &Path) -> bool {
    let mut buf = [0u8; 5];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(read) = std::io::Read::read(&mut file, &mut buf) else {
        return false;
    };
    read >= 4 && &buf[..4] == b"%PDF"
}

fn is_docx(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("docx"))
}

fn render_pdf(path: &Path, rel: &str) -> ToolOutcome {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return super::context_error(Some(rel), &format!("Unable to read `{rel}`: {error}"));
        }
    };
    if bytes.len() as u64 > 32 * 1024 * 1024 {
        return super::context_error(Some(rel), "PDF exceeds 32 MB.");
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file.pdf".into());
    ToolOutcome {
        text: toon_doc(&[
            ("path", ToonValue::Str(rel)),
            ("kind", ToonValue::Str("pdf")),
            ("mime", ToonValue::Str("application/pdf")),
            ("status", ToonValue::Str("attached")),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(rel.to_string()),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: Some(super::ToolFile {
            name,
            mime: "application/pdf".into(),
            bytes,
        }),
    }
}

fn render_docx(path: &Path, rel: &str) -> ToolOutcome {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return super::context_error(Some(rel), &format!("Unable to read `{rel}`: {error}"));
        }
    };
    let Some(text) = crate::attachments::extract_docx_text(&bytes) else {
        return super::context_error(Some(rel), "Could not extract text from the document.");
    };
    ToolOutcome {
        text: toon_doc(&[
            ("path", ToonValue::Str(rel)),
            ("kind", ToonValue::Str("document")),
            ("content", ToonValue::Block(&text)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(rel.to_string()),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn detect_image(path: &Path) -> Option<ImageKind> {
    let mut buf = [0u8; IMAGE_PROBE_BYTES];
    let mut file = fs::File::open(path).ok()?;
    let read = std::io::Read::read(&mut file, &mut buf).ok()?;
    if read < 8 {
        return None;
    }
    detect_image_from_bytes(&buf[..read])
}

fn detect_image_from_bytes(bytes: &[u8]) -> Option<ImageKind> {
    if starts_with(bytes, b"\x89PNG\r\n\x1a\n") {
        return Some(ImageKind::Png);
    }
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return Some(ImageKind::Jpeg);
    }
    if starts_with(bytes, b"GIF87a") || starts_with(bytes, b"GIF89a") {
        return Some(ImageKind::Gif);
    }
    if bytes.len() >= 12 && starts_with(bytes, b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageKind::Webp);
    }
    None
}

fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix
}

struct ReadResult {
    raw: Vec<String>,
    total_lines: usize,
    more: bool,
    capped: bool,
}

fn read_windowed(path: &Path, offset: usize, limit: usize) -> std::io::Result<ReadResult> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let start = offset.saturating_sub(1);
    let mut raw: Vec<String> = Vec::with_capacity(limit.min(DEFAULT_LIMIT));
    let mut bytes_used: usize = 0;
    let mut total_lines: usize = 0;
    let mut more = false;
    let mut capped = false;
    let mut buffer = String::new();
    loop {
        buffer.clear();
        let read = reader.read_line(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_lines += 1;
        if total_lines <= start {
            continue;
        }
        if raw.len() >= limit {
            more = true;
            continue;
        }
        let mut line = buffer.trim_end_matches(['\n', '\r']).to_string();
        if line.len() > MAX_LINE_LENGTH {
            line.truncate(MAX_LINE_LENGTH);
            line.push_str(MAX_LINE_SUFFIX);
        }
        let line_bytes = line.len() + if raw.is_empty() { 0 } else { 1 };
        if bytes_used + line_bytes > MAX_BYTES {
            capped = true;
            more = true;
            continue;
        }
        raw.push(line);
        bytes_used += line_bytes;
    }
    Ok(ReadResult {
        raw,
        total_lines,
        more,
        capped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;
    use serde_json::json;

    #[test]
    fn reads_windowed_lines() {
        let dir = tempdir();
        let path = dir.join("note.txt");
        let content = (1..=5)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, content).unwrap();

        let result = read_windowed(&path, 1, 3).unwrap();
        assert_eq!(result.raw, vec!["line 1", "line 2", "line 3"]);
        assert!(result.more);
        assert_eq!(result.total_lines, 5);
    }

    #[test]
    fn trims_lines_over_max_length() {
        let dir = tempdir();
        let path = dir.join("long.txt");
        let body: String = std::iter::repeat('x').take(MAX_LINE_LENGTH + 50).collect();
        fs::write(&path, &body).unwrap();
        let result = read_windowed(&path, 1, 1).unwrap();
        assert_eq!(result.raw.len(), 1);
        assert!(result.raw[0].ends_with(MAX_LINE_SUFFIX));
    }

    #[test]
    fn execute_requires_file_path() {
        let dir = tempdir();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_png_magic_bytes() {
        let bytes = b"\x89PNG\r\n\x1a\nrest";
        assert!(matches!(
            detect_image_from_bytes(bytes),
            Some(ImageKind::Png)
        ));
    }

    #[test]
    fn detects_jpeg_magic_bytes() {
        let bytes = b"\xff\xd8\xff\xe0";
        assert!(matches!(
            detect_image_from_bytes(bytes),
            Some(ImageKind::Jpeg)
        ));
    }

    #[test]
    fn detects_gif_magic_bytes() {
        assert!(matches!(
            detect_image_from_bytes(b"GIF89a..."),
            Some(ImageKind::Gif)
        ));
    }

    #[test]
    fn detects_webp_magic_bytes() {
        let bytes = b"RIFF\x00\x00\x00\x00WEBPVP8";
        assert!(matches!(
            detect_image_from_bytes(bytes),
            Some(ImageKind::Webp)
        ));
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert!(detect_image_from_bytes(b"hello, world").is_none());
        assert!(detect_image_from_bytes(b"\x89PNG").is_none());
    }

    #[test]
    fn execute_decodes_png_image() {
        let dir = tempdir();
        let path = dir.join("pixel.png");
        let pixel = DynamicImage::new_rgba8(8, 8);
        let mut encoded = Vec::new();
        pixel
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .unwrap();
        fs::write(&path, &encoded).unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({ "filePath": path.to_string_lossy() }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("ok"));
        let png = outcome.image_png.expect("image_png set");
        assert!(!png.is_empty());
        assert!(outcome.display.image_data.is_some());
        assert!(outcome.text.contains("mime: image/png"));
        assert!(outcome.text.contains("image: png attached"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_decodes_jpeg_image() {
        let dir = tempdir();
        let path = dir.join("pixel.jpg");
        let pixel = DynamicImage::new_rgb8(8, 8);
        let mut encoded = Vec::new();
        pixel
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Jpeg)
            .unwrap();
        fs::write(&path, &encoded).unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({ "filePath": path.to_string_lossy() }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("ok"));
        assert!(outcome.image_png.is_some());
        assert!(outcome.text.contains("mime: image/jpeg"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_rejects_oversize_image() {
        let dir = tempdir();
        let path = dir.join("huge.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(MAX_IMAGE_BYTES as usize + 1, 0);
        fs::write(&path, &bytes).unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({ "filePath": path.to_string_lossy() }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.text.contains("exceeds 20 MB ingestion limit"));
        assert!(outcome.image_png.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_rejects_truncated_image_bytes() {
        let dir = tempdir();
        let path = dir.join("truncated.png");
        fs::write(&path, b"\x89PNG\r\n\x1a\ntruncated").unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({ "filePath": path.to_string_lossy() }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.text.contains("could not be decoded"));
        assert!(outcome.image_png.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_treats_image_extension_with_text_as_text() {
        let dir = tempdir();
        let path = dir.join("notes.png");
        fs::write(&path, "just text, not a real png").unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(&json!({ "filePath": path.to_string_lossy() }), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("ok"));
        assert!(outcome.image_png.is_none());
        assert!(outcome.text.contains("just text, not a real png"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn page_request_forces_text_path() {
        let dir = tempdir();
        let path = dir.join("pixel.png");
        let pixel = DynamicImage::new_rgba8(4, 4);
        let mut encoded = Vec::new();
        pixel
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .unwrap();
        fs::write(&path, &encoded).unwrap();

        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ReadTool.execute(
            &json!({ "filePath": path.to_string_lossy(), "offset": 1, "limit": 10 }),
            &ctx,
        );
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.image_png.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    fn tempdir() -> PathBuf {
        let unique = format!(
            "k-agent-read-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
