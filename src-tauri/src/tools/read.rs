use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutcome, ToolSpec};

pub const NAME: &str = "read";

const DESCRIPTION: &str = "Read a file or list a directory. Path is absolute or workspace-relative. Optional offset (1-based) and limit (default 2000 lines). Output capped at 50 KB.";

const DEFAULT_LIMIT: usize = 2000;
const MAX_LINE_LENGTH: usize = 2000;
const MAX_LINE_SUFFIX: &str = "... (line truncated to 2000 chars)";
const MAX_BYTES: usize = 50 * 1024;
const MAX_BYTES_LABEL: &str = "50 KB";

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
                        "description": "1-based start line"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Max lines (default 2000)"
                    }
                },
                "required": ["filePath"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(raw_path) = args.get("filePath").and_then(Value::as_str) else {
            return ToolOutcome {
                text: "read tool requires a string `filePath`.".into(),
            };
        };
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return ToolOutcome {
                text: "read tool `filePath` is empty.".into(),
            };
        }
        let offset = match parse_usize_arg(args, "offset", 1, 1) {
            Ok(value) => value,
            Err(message) => return ToolOutcome { text: message },
        };
        let limit = match parse_usize_arg(args, "limit", DEFAULT_LIMIT, 1) {
            Ok(value) => value,
            Err(message) => return ToolOutcome { text: message },
        };

        let resolved = match resolve_path(ctx, trimmed) {
            Ok(value) => value,
            Err(message) => return ToolOutcome { text: message },
        };
        let metadata = match fs::metadata(&resolved) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let suggestions = fuzzy_sibling_suggestions(&resolved);
                let body = if suggestions.is_empty() {
                    format!("File not found: {}", resolved.display())
                } else {
                    format!(
                        "File not found: {}\n\nDid you mean one of these?\n{}",
                        resolved.display(),
                        suggestions.join("\n")
                    )
                };
                return ToolOutcome { text: body };
            }
            Err(error) => {
                return ToolOutcome {
                    text: format!("Unable to stat `{}`: {error}", resolved.display()),
                };
            }
        };

        let file_type = metadata.file_type();
        if file_type.is_dir() {
            return ToolOutcome {
                text: render_directory(&resolved, offset, limit),
            };
        }
        if !file_type.is_file() {
            return ToolOutcome {
                text: format!(
                    "Cannot read `{}`: not a regular file or directory.",
                    resolved.display()
                ),
            };
        }

        if is_likely_binary_path(&resolved) {
            return ToolOutcome {
                text: format!("Cannot read binary file: {}", resolved.display()),
            };
        }

        ToolOutcome {
            text: render_file(&resolved, offset, limit),
        }
    }
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

fn render_directory(path: &Path, offset: usize, limit: usize) -> String {
    let entries = match collect_directory_entries(path) {
        Ok(value) => value,
        Err(error) => return format!("Unable to list `{}`: {error}", path.display()),
    };
    if entries.is_empty() {
        return format!(
            "<path>{}</path>\n<type>directory</type>\n<entries>\n(empty)\n</entries>",
            path.display()
        );
    }
    let start = offset.saturating_sub(1);
    let end = start.saturating_add(limit).min(entries.len());
    let slice = entries.get(start..end).unwrap_or(&[]);
    let truncated = end < entries.len();
    let mut out = String::new();
    out.push_str(&format!("<path>{}</path>\n", path.display()));
    out.push_str("<type>directory</type>\n<entries>\n");
    out.push_str(&slice.join("\n"));
    out.push('\n');
    if truncated {
        out.push_str(&format!(
            "\n(Showing {} of {} entries. Use 'offset' parameter to read beyond entry {})",
            slice.len(),
            entries.len(),
            end + 1
        ));
    } else {
        out.push_str(&format!("\n({} entries)", entries.len()));
    }
    out.push_str("\n</entries>");
    out
}

fn collect_directory_entries(path: &Path) -> std::io::Result<Vec<String>> {
    let mut names: Vec<(String, bool)> = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let mut is_dir = file_type.is_dir();
        if file_type.is_symlink() {
            if let Ok(target) = fs::metadata(entry.path()) {
                is_dir = target.is_dir();
            }
        }
        names.push((name, is_dir));
    }
    names.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(names
        .into_iter()
        .map(|(name, is_dir)| if is_dir { format!("{name}/") } else { name })
        .collect())
}

fn is_likely_binary_path(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
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

struct ReadResult {
    raw: Vec<String>,
    total_lines: usize,
    more: bool,
    capped: bool,
}

fn render_file(path: &Path, offset: usize, limit: usize) -> String {
    let outcome = read_windowed(path, offset, limit);
    let result = match outcome {
        Ok(value) => value,
        Err(error) => {
            return if error.kind() == std::io::ErrorKind::InvalidData {
                format!("Cannot read binary file: {}", path.display())
            } else {
                format!("Unable to read `{}`: {error}", path.display())
            }
        }
    };
    if result.raw.is_empty() && offset > 1 {
        if result.total_lines < offset {
            return format!(
                "Offset {offset} is out of range for this file ({} lines).",
                result.total_lines
            );
        }
    }
    let mut output = String::new();
    output.push_str(&format!("<path>{}</path>\n", path.display()));
    output.push_str("<type>file</type>\n<content>\n");
    for (index, line) in result.raw.iter().enumerate() {
        let line_number = offset + index;
        output.push_str(&format!("{line_number}: {line}\n"));
    }
    let last = offset + result.raw.len().saturating_sub(1);
    if result.capped {
        let next = last + 1;
        output.push_str(&format!(
            "\n(Output capped at {MAX_BYTES_LABEL}. Showing lines {offset}-{last}. Use offset={next} to continue.)"
        ));
    } else if result.more {
        let next = last + 1;
        output.push_str(&format!(
            "\n(Showing lines {offset}-{last} of {}. Use offset={next} to continue.)",
            result.total_lines
        ));
    } else {
        output.push_str(&format!(
            "\n(End of file - total {} lines)",
            result.total_lines
        ));
    }
    output.push_str("\n</content>");
    output
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
        let mut line = buffer.trim_end_matches(['\n', '\r']).to_string();
        if line.len() > MAX_LINE_LENGTH {
            line.truncate(MAX_LINE_LENGTH);
            line.push_str(MAX_LINE_SUFFIX);
        }
        if total_lines > start && raw.len() < limit {
            let line_bytes = line.len() + if raw.is_empty() { 0 } else { 1 };
            if bytes_used + line_bytes > MAX_BYTES {
                capped = true;
                more = true;
                break;
            }
            raw.push(line);
            bytes_used += line_bytes;
            if raw.len() >= limit {
                more = true;
                break;
            }
        }
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
