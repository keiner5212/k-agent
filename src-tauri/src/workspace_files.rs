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
