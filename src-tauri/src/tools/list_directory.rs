use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{
    yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "list_directory";

const DESCRIPTION: &str = "List directory entries. Path is absolute or workspace-relative (default: workspace root). recursive walks the tree; maxDepth default 3, max 10. Skips noise dirs. Capped at 5000 lines.";

const MAX_ENTRIES_PER_DIR: usize = 2_000;
const MAX_OUTPUT_LINES: usize = 5_000;
const MAX_DEPTH_DEFAULT: usize = 3;

const SKIP_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "target",
    "dist",
    "build",
    "out",
    "output",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".venv",
    "venv",
    ".tox",
    "env",
    "vendor",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".cache",
    ".parcel-cache",
    "coverage",
    ".nyc_output",
    "bin",
    "obj",
    "Pods",
    ".gradle",
    "elm-stuff",
    "_build",
    "deps",
    ".stack-work",
    ".pnpm",
    ".yarn",
    "bower_components",
    "jspm_packages",
    ".serverless",
    ".terraform",
    ".cargo",
    "zig-cache",
    "zig-out",
    ".dart_tool",
    "DerivedData",
    "Carthage",
    ".bundle",
    "htmlcov",
    ".hypothesis",
    "site-packages",
    ".mvn",
    ".idea",
    ".vscode",
    ".vs",
    ".local",
    ".config",
    "snap",
    ".var",
    ".mozilla",
    ".thunderbird",
    ".npm",
    ".nvm",
    "go",
];

pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "dirPath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative dir (default: workspace)"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Walk subdirectories (default false)"
                    },
                    "maxDepth": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Max depth when recursive (default 3, max 10)"
                    }
                }
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let raw_path = args
            .get("dirPath")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let recursive = args
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_depth = match parse_depth(args) {
            Ok(value) => value,
            Err(message) => return super::context_error(None, &message),
        };

        let resolved = match resolve_path(ctx, raw_path) {
            Ok(value) => value,
            Err(message) => return super::context_error(Some(raw_path), &message),
        };
        let rel = ctx.relative_path(&resolved);
        let metadata = match fs::metadata(&resolved) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return super::context_error(
                    Some(&rel),
                    &format!("Directory not found: {}", resolved.display()),
                );
            }
            Err(error) => {
                return super::context_error(
                    Some(&rel),
                    &format!("Unable to stat `{}`: {error}", resolved.display()),
                );
            }
        };
        if !metadata.is_dir() {
            return super::context_error(
                Some(&rel),
                &format!("Path is not a directory: {}", resolved.display()),
            );
        }

        render_tree(&resolved, &rel, recursive, max_depth)
    }
}

fn parse_depth(args: &Value) -> Result<usize, String> {
    let Some(value) = args.get("maxDepth") else {
        return Ok(MAX_DEPTH_DEFAULT);
    };
    let Some(number) = value.as_u64() else {
        return Err("list_directory `maxDepth` must be a non-negative integer.".into());
    };
    let parsed = usize::try_from(number)
        .map_err(|_| "list_directory `maxDepth` is out of range.".to_string())?;
    Ok(parsed.clamp(1, 10))
}

fn resolve_path(ctx: &ToolContext<'_>, raw: &str) -> Result<PathBuf, String> {
    crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref())
}

fn render_tree(root: &Path, rel: &str, recursive: bool, max_depth: usize) -> ToolOutcome {
    let mut lines: Vec<String> = Vec::new();
    if recursive {
        walk_recursive(root, 0, max_depth, &mut lines);
    } else {
        let entries = match collect_entries(root, MAX_ENTRIES_PER_DIR) {
            Ok(value) => value,
            Err(error) => {
                return super::context_error(Some(rel), &format!("Unable to read: {error}"));
            }
        };
        for (name, is_dir) in entries {
            if lines.len() >= MAX_OUTPUT_LINES {
                lines.push(format!("... (truncated at {} lines)", MAX_OUTPUT_LINES));
                break;
            }
            lines.push(if is_dir { format!("{name}/") } else { name });
        }
    }
    let entries = lines.join("\n");
    ToolOutcome {
        text: yaml_doc(&[
            ("path", YamlValue::Str(rel)),
            ("entries", YamlValue::Block(&entries)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(rel.to_string()),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
    }
}

fn walk_recursive(dir: &Path, depth: usize, max_depth: usize, lines: &mut Vec<String>) {
    if depth >= max_depth {
        return;
    }
    let entries = match collect_entries(dir, MAX_ENTRIES_PER_DIR) {
        Ok(value) => value,
        Err(_) => return,
    };
    let indent = "  ".repeat(depth);
    for (name, is_dir) in entries {
        if lines.len() >= MAX_OUTPUT_LINES {
            lines.push(format!(
                "{}... (truncated at {} lines)",
                indent, MAX_OUTPUT_LINES
            ));
            return;
        }
        if is_dir {
            lines.push(format!("{indent}{name}/"));
            walk_recursive(&dir.join(&name), depth + 1, max_depth, lines);
        } else {
            lines.push(format!("{indent}{name}"));
        }
        if lines.len() >= MAX_OUTPUT_LINES {
            return;
        }
    }
}

fn collect_entries(dir: &Path, limit: usize) -> std::io::Result<Vec<(String, bool)>> {
    let mut names: Vec<(String, bool)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_skip(&name) {
            continue;
        }
        let mut is_dir = file_type.is_dir();
        if file_type.is_symlink() {
            if let Ok(target) = fs::metadata(entry.path()) {
                is_dir = target.is_dir();
            }
        }
        names.push((name, is_dir));
        if names.len() >= limit {
            break;
        }
    }
    names.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(names)
}

fn should_skip(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    SKIP_DIR_NAMES
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_max_depth() {
        assert_eq!(parse_depth(&json!({})).unwrap(), MAX_DEPTH_DEFAULT);
        assert_eq!(parse_depth(&json!({"maxDepth": 0})).unwrap(), 1);
        assert_eq!(parse_depth(&json!({"maxDepth": 99})).unwrap(), 10);
        assert_eq!(parse_depth(&json!({"maxDepth": 5})).unwrap(), 5);
    }
}
