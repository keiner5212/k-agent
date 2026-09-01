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
pub const MAX_PARALLELISM: usize = 16;

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

        render_tree(&resolved, &rel, recursive, max_depth, ctx.parallelism)
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

fn render_tree(
    root: &Path,
    rel: &str,
    recursive: bool,
    max_depth: usize,
    parallelism: usize,
) -> ToolOutcome {
    let mut lines: Vec<String> = Vec::new();
    let budget = Arc::new(Mutex::new(MAX_OUTPUT_LINES));
    if recursive {
        walk_inner(root, 0, max_depth, parallelism, &budget, &mut lines);
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

fn walk_inner(
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
    let can_recurse = depth + 1 < max_depth;

    let subdir_trees: Vec<Vec<String>> = if can_recurse && parallelism > 1 {
        let subdirs: Vec<PathBuf> = entries
            .iter()
            .filter(|entry| entry.is_dir)
            .map(|entry| dir.join(&entry.name))
            .collect();
        if subdirs.is_empty() {
            Vec::new()
        } else {
            thread::scope(|scope| {
                let mut handles = Vec::with_capacity(subdirs.len());
                for subdir in subdirs {
                    if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                        break;
                    }
                    let subdir = subdir.clone();
                    let budget = Arc::clone(budget);
                    handles.push(scope.spawn(move || {
                        let mut local: Vec<String> = Vec::new();
                        walk_inner(&subdir, depth + 1, max_depth, 1, &budget, &mut local);
                        local
                    }));
                }
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap_or_default())
                    .collect()
            })
        }
    } else {
        Vec::new()
    };

    let mut subdir_idx = 0usize;
    for entry in &entries {
        if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
            lines.push(format!("... (truncated at {} lines)", MAX_OUTPUT_LINES));
            return;
        }
        if entry.is_dir {
            lines.push(format!("{indent}{}/", entry.name));
            if let Ok(mut g) = budget.lock() {
                *g = g.saturating_sub(1);
            }
            if can_recurse {
                if parallelism > 1 {
                    if let Some(tree) = subdir_trees.get(subdir_idx) {
                        for line in tree {
                            if budget.lock().map(|g| *g).unwrap_or(0) == 0 {
                                lines.push(format!(
                                    "... (truncated at {} lines)",
                                    MAX_OUTPUT_LINES
                                ));
                                return;
                            }
                            lines.push(line.clone());
                            if let Ok(mut b) = budget.lock() {
                                *b = b.saturating_sub(1);
                            }
                        }
                        subdir_idx += 1;
                    }
                } else {
                    walk_inner(
                        &dir.join(&entry.name),
                        depth + 1,
                        max_depth,
                        1,
                        budget,
                        lines,
                    );
                }
            }
        } else {
            lines.push(format!("{indent}{}", entry.name));
            if let Ok(mut g) = budget.lock() {
                *g = g.saturating_sub(1);
            }
        }
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

    #[test]
    fn lists_workspace_files() {
        let dir = std::env::temp_dir().join(format!(
            "k-agent-list-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("alpha.txt"), "a").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ListDirectoryTool.execute(&json!({"dirPath": "."}), &ctx);
        assert!(outcome.text.contains("alpha.txt"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recursive_output_groups_children_under_parent() {
        let dir = std::env::temp_dir().join(format!(
            "k-agent-list-dfs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("child")).unwrap();
        fs::write(dir.join("root.txt"), "r").unwrap();
        fs::write(dir.join("child/nested.txt"), "n").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 1);
        let outcome = ListDirectoryTool.execute(
            &json!({"dirPath": ".", "recursive": true, "maxDepth": 3}),
            &ctx,
        );
        let text = &outcome.text;
        let child_idx = text.find("child/").expect("child dir line present");
        let nested_idx = text.find("nested.txt").expect("nested file present");
        let root_idx = text.find("root.txt").expect("root file present");
        assert!(
            child_idx < nested_idx,
            "child/ debe preceder a nested.txt (DFS); got:\n{text}"
        );
        assert!(
            nested_idx < root_idx,
            "nested.txt (bajo child/) debe preceder a root.txt (nivel 1)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recursive_output_groups_children_under_parent_parallel() {
        let dir = std::env::temp_dir().join(format!(
            "k-agent-list-dfs-par-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("alpha")).unwrap();
        fs::create_dir_all(dir.join("beta")).unwrap();
        fs::create_dir_all(dir.join("gamma")).unwrap();
        fs::write(dir.join("root.txt"), "r").unwrap();
        fs::write(dir.join("alpha/a.txt"), "a").unwrap();
        fs::write(dir.join("beta/b.txt"), "b").unwrap();
        fs::write(dir.join("gamma/c.txt"), "c").unwrap();
        let ctx = crate::tools::ToolContext::for_test(dir.clone(), 4);
        let outcome = ListDirectoryTool.execute(
            &json!({"dirPath": ".", "recursive": true, "maxDepth": 3}),
            &ctx,
        );
        let text = &outcome.text;
        for (dir_name, child) in [("alpha", "a.txt"), ("beta", "b.txt"), ("gamma", "c.txt")] {
            let dir_idx = text.find(&format!("{dir_name}/")).unwrap_or_else(|| {
                panic!("{dir_name}/ missing in output:\n{text}")
            });
            let child_idx = text.find(child).unwrap_or_else(|| {
                panic!("{child} missing in output:\n{text}")
            });
            let root_idx = text.find("root.txt").unwrap_or_else(|| {
                panic!("root.txt missing in output:\n{text}")
            });
            assert!(
                dir_idx < child_idx,
                "{dir_name}/ debe preceder a {child} (DFS, parallel)"
            );
            assert!(
                child_idx < root_idx,
                "{child} (bajo {dir_name}/) debe preceder a root.txt (nivel 1)"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
}