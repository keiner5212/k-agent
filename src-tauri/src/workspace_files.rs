use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::LocalWorkspace;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub path: String,
    pub kind: String,
}

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

const MAX_DIR_ENTRIES: usize = 2_000;
const MAX_SEARCH_VISIT: usize = 20_000;
const MAX_SEARCH_DEPTH: usize = 16;
const MAX_SEARCH_HITS: usize = 40;

fn should_skip_dir(name: &str) -> bool {
    if SKIP_DIR_NAMES
        .iter()
        .any(|item| name.eq_ignore_ascii_case(item))
    {
        return true;
    }
    name.starts_with('.') && name != ".agents"
}

fn path_to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_access_denied(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::NotFound
    )
}

fn normalize_relative_dir(input: Option<String>) -> Result<PathBuf, String> {
    let trimmed = input.unwrap_or_default().trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(PathBuf::new());
    }
    if trimmed.starts_with('/') || trimmed.contains("..") {
        return Err("invalid workspace path".to_string());
    }
    Ok(PathBuf::from(trimmed))
}

fn list_dir_level(root: &Path, relative_dir: &Path) -> Result<Vec<WorkspaceEntry>, String> {
    let absolute = root.join(relative_dir);
    if !absolute.is_dir() {
        return Err("workspace path is unavailable".to_string());
    }
    let read = match fs::read_dir(&absolute) {
        Ok(entries) => entries,
        Err(error) if is_access_denied(&error) => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };

    let mut out = Vec::new();
    for entry in read.flatten() {
        if out.len() >= MAX_DIR_ENTRIES {
            break;
        }
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = if file_type.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            "dir"
        } else if file_type.is_file() {
            "file"
        } else {
            continue;
        };
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        out.push(WorkspaceEntry {
            path: path_to_posix(rel),
            kind: kind.to_string(),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

struct ScoredEntry {
    entry: WorkspaceEntry,
    score: u32,
}

fn is_subsequence(path: &str, needle: &str) -> bool {
    let mut rest = path;
    for ch in needle.chars() {
        match rest.find(ch) {
            Some(index) => rest = &rest[index + ch.len_utf8()..],
            None => return false,
        }
    }
    true
}

fn score_path(path: &str, query: &str) -> Option<u32> {
    let path_l = path.to_ascii_lowercase();
    let query_l = query.to_ascii_lowercase();
    let (dir, needle) = match query_l.rsplit_once('/') {
        Some((dir, needle)) if !dir.is_empty() && !needle.is_empty() => (Some(dir), needle),
        _ => (None, query_l.as_str()),
    };
    if needle.is_empty() {
        return None;
    }
    if let Some(dir) = dir {
        let prefix = format!("{dir}/");
        if path_l != dir && !path_l.starts_with(&prefix) {
            return None;
        }
    }
    let name = path_l.rsplit('/').next().unwrap_or(path_l.as_str());
    if name == needle {
        return Some(0);
    }
    if name.starts_with(needle) {
        return Some(1);
    }
    if path_l.contains(query_l.as_str()) {
        return Some(2);
    }
    if name.contains(needle) {
        return Some(3);
    }
    if needle.len() >= 2 && is_subsequence(&path_l, needle) {
        return Some(4);
    }
    None
}

fn search_tree(root: &Path, query: &str) -> Vec<WorkspaceEntry> {
    let query_norm = query.trim().trim_matches('/').replace('\\', "/");
    if query_norm.is_empty() || query_norm.contains("..") || query_norm.starts_with('/') {
        return Vec::new();
    }
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    let mut hits: Vec<ScoredEntry> = Vec::new();
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_SEARCH_DEPTH || visited > MAX_SEARCH_VISIT {
            break;
        }
        let read = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in read.flatten() {
            visited += 1;
            if visited > MAX_SEARCH_VISIT {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(value) => value,
                Err(_) => continue,
            };
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel_posix = path_to_posix(rel);
            if file_type.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                if let Some(score) = score_path(&rel_posix, &query_norm) {
                    hits.push(ScoredEntry {
                        entry: WorkspaceEntry {
                            path: rel_posix.clone(),
                            kind: "dir".to_string(),
                        },
                        score,
                    });
                }
                stack.push((path, depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if let Some(score) = score_path(&rel_posix, &query_norm) {
                hits.push(ScoredEntry {
                    entry: WorkspaceEntry {
                        path: rel_posix,
                        kind: "file".to_string(),
                    },
                    score,
                });
            }
        }
    }
    hits.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.entry.path.cmp(&right.entry.path))
    });
    hits.truncate(MAX_SEARCH_HITS);
    hits.into_iter().map(|hit| hit.entry).collect()
}

#[tauri::command]
pub fn search_workspace_files(
    state: State<'_, LocalWorkspace>,
    query: String,
) -> Result<Vec<WorkspaceEntry>, String> {
    let root = state
        .path
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .ok_or_else(|| "workspace is unavailable".to_string())?;
    if !root.is_dir() {
        return Err("workspace is unavailable".to_string());
    }
    Ok(search_tree(&root, &query))
}

#[tauri::command]
pub fn list_workspace_files(
    state: State<'_, LocalWorkspace>,
    relative_dir: Option<String>,
) -> Result<Vec<WorkspaceEntry>, String> {
    let root = state
        .path
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .ok_or_else(|| "workspace is unavailable".to_string())?;
    if !root.is_dir() {
        return Err("workspace is unavailable".to_string());
    }
    let relative = normalize_relative_dir(relative_dir)?;
    list_dir_level(&root, &relative)
}

#[cfg(test)]
mod tests {
    use super::{score_path, search_tree};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn score_prefers_name_prefix_under_typed_dir() {
        assert_eq!(score_path("src/components/Button.tsx", "src/but"), Some(1));
        assert_eq!(score_path("lib/Button.tsx", "src/but"), None);
        assert_eq!(score_path("notes.txt", "note"), Some(1));
    }

    #[test]
    fn search_finds_nested_name() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("k-agent-mention-{nanos}"));
        let nested = root.join("src").join("components");
        fs::create_dir_all(&nested).expect("dir");
        fs::write(nested.join("Button.tsx"), "x").expect("file");
        fs::write(root.join("readme.md"), "x").expect("file");
        let hits = search_tree(&root, "button");
        let _ = fs::remove_dir_all(&root);
        assert!(hits
            .iter()
            .any(|entry| entry.path == "src/components/Button.tsx"));
        assert!(hits.iter().all(|entry| entry.path != "readme.md"));
    }
}
