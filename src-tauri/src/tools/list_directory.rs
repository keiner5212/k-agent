use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{
    yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "list_directory";

const DESCRIPTION: &str = "List directory entries. Path is absolute or workspace-relative (default: workspace root). recursive walks the tree; maxDepth default 3, max 10. Skips noise dirs. Capped at 5000 lines. Walks subtrees in parallel using available CPU cores with a short-lived cache.";

const MAX_ENTRIES_PER_DIR: usize = 2_000;
const MAX_OUTPUT_LINES: usize = 5_000;
const MAX_DEPTH_DEFAULT: usize = 3;
const CACHE_TTL: Duration = Duration::from_millis(1000);
const HARD_PARALLELISM_CAP: usize = 16;

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

#[derive(Clone)]
struct DirEntry {
    name: String,
    is_dir: bool,
}

struct CacheEntry {
    at: Instant,
    entries: Vec<DirEntry>,
}

static DIR_CACHE: OnceLock<Mutex<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();

fn dir_cache() -> &'static Mutex<HashMap<PathBuf, CacheEntry>> {
    DIR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn read_cached(dir: &Path) -> Option<Vec<DirEntry>> {
    let cache = dir_cache().lock().ok()?;
    let entry = cache.get(dir)?;
    if entry.at.elapsed() < CACHE_TTL {
        return Some(entry.entries.clone());
    }
    None
}

fn write_cached(dir: &Path, entries: &[DirEntry]) {
    if let Ok(mut cache) = dir_cache().lock() {
        cache.insert(
            dir.to_path_buf(),
            CacheEntry {
                at: Instant::now(),
                entries: entries.to_vec(),
            },
        );
    }
}

fn resolve_parallel_cap() -> usize {
    let hardware = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    hardware.min(HARD_PARALLELISM_CAP).max(1)
}

fn collect_entries(dir: &Path, limit: usize) -> std::io::Result<Vec<DirEntry>> {
    if let Some(cached) = read_cached(dir) {
        return Ok(cached);
    }
    let mut names: Vec<DirEntry> = Vec::new();
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
        names.push(DirEntry { name, is_dir });
        if names.len() >= limit {
            break;
        }
    }
    names.sort_by(|left, right| left.name.cmp(&right.name));
    write_cached(dir, &names);
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

fn render_tree(root: &Path, rel: &str, recursive: bool, max_depth: usize) -> ToolOutcome {
    let mut lines: Vec<String> = Vec::new();
    let parallelism = if recursive { resolve_parallel_cap() } else { 1 };
    let budget = Arc::new(Mutex::new(MAX_OUTPUT_LINES));
    if recursive {
        walk_recursive(root, 0, max_depth, parallelism, &budget, &mut lines);
    } else {
        let entries = match collect_entries(root, MAX_ENTRIES_PER_DIR) {
            Ok(value) => value,
            Err(error) => {
                return super::context_error(Some(rel), &format!("Unable to read: {error}"));
            }
        };
        for entry in entries {
            if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                lines.push(format!("... (truncated at {} lines)", MAX_OUTPUT_LINES));
                break;
            }
            lines.push(if entry.is_dir {
                format!("{}/", entry.name)
            } else {
                entry.name
            });
            if let Ok(mut g) = budget.lock() {
                *g = g.saturating_sub(1);
            }
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

fn walk_recursive(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    parallelism: usize,
    budget: &Arc<Mutex<usize>>,
    lines: &mut Vec<String>,
) {
    if depth >= max_depth {
        return;
    }
    if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
        return;
    }
    let entries = match collect_entries(dir, MAX_ENTRIES_PER_DIR) {
        Ok(value) => value,
        Err(_) => return,
    };
    let indent = "  ".repeat(depth);
    let subdirs: Vec<PathBuf> = entries
        .iter()
        .filter(|entry| entry.is_dir)
        .map(|entry| dir.join(&entry.name))
        .collect();
    for entry in &entries {
        if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
            return;
        }
        if entry.is_dir {
            lines.push(format!("{indent}{}/", entry.name));
        } else {
            lines.push(format!("{indent}{}", entry.name));
        }
        if let Ok(mut g) = budget.lock() {
            *g = g.saturating_sub(1);
        }
    }
    if subdirs.is_empty() || depth + 1 >= max_depth {
        return;
    }
    if parallelism <= 1 {
        for subdir in subdirs {
            walk_recursive(&subdir, depth + 1, max_depth, 1, budget, lines);
            if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                return;
            }
        }
        return;
    }
    let collected: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    for chunk in subdirs.chunks(parallelism) {
        if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
            return;
        }
        let chunk_paths: Vec<PathBuf> = chunk.to_vec();
        let budget = Arc::clone(budget);
        let collected = Arc::clone(&collected);
        thread::scope(|scope| {
            for subdir in chunk_paths {
                let collected = Arc::clone(&collected);
                let budget = Arc::clone(&budget);
                scope.spawn(move || {
                    let mut local: Vec<String> = Vec::new();
                    walk_inner(&subdir, depth + 1, max_depth, &budget, &mut local);
                    if let Ok(mut g) = collected.lock() {
                        g.push(local);
                    }
                });
            }
        });
        let mut drained: Vec<Vec<String>> = Vec::new();
        if let Ok(mut g) = collected.lock() {
            drained.append(&mut *g);
        }
        for local in drained {
            if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                return;
            }
            for line in local {
                if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                    lines.push(format!("... (truncated at {} lines)", MAX_OUTPUT_LINES));
                    return;
                }
                lines.push(line);
                if let Ok(mut b) = budget.lock() {
                    *b = b.saturating_sub(1);
                }
            }
        }
    }
}

fn walk_inner(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    budget: &Arc<Mutex<usize>>,
    lines: &mut Vec<String>,
) {
    if depth >= max_depth {
        return;
    }
    if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
        return;
    }
    let entries = match collect_entries(dir, MAX_ENTRIES_PER_DIR) {
        Ok(value) => value,
        Err(_) => return,
    };
    let indent = "  ".repeat(depth);
    let subdirs: Vec<PathBuf> = entries
        .iter()
        .filter(|entry| entry.is_dir)
        .map(|entry| dir.join(&entry.name))
        .collect();
    for entry in &entries {
        if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
            return;
        }
        if entry.is_dir {
            lines.push(format!("{indent}{}/", entry.name));
        } else {
            lines.push(format!("{indent}{}", entry.name));
        }
        if let Ok(mut g) = budget.lock() {
            *g = g.saturating_sub(1);
        }
    }
    for subdir in subdirs {
        walk_inner(&subdir, depth + 1, max_depth, budget, lines);
    }
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