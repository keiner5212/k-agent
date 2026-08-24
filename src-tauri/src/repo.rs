use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: String,
    pub is_repo: bool,
    pub branch: Option<String>,
}

const HEAD_REF_PREFIX: &str = "ref: refs/heads/";

#[tauri::command]
pub async fn get_repo_info(path: String) -> Result<RepoInfo, String> {
    let root = PathBuf::from(&path);
    if !root.exists() {
        return Ok(RepoInfo {
            path,
            is_repo: false,
            branch: None,
        });
    }
    let Some(git_dir) = find_git_dir(&root) else {
        return Ok(RepoInfo {
            path,
            is_repo: false,
            branch: None,
        });
    };
    let branch = read_branch(&git_dir);
    Ok(RepoInfo {
        path,
        is_repo: true,
        branch,
    })
}

fn find_git_dir(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };
    loop {
        let candidate = current.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if candidate.is_file() {
            if let Ok(contents) = fs::read_to_string(&candidate) {
                let trimmed = contents.trim();
                if let Some(rest) = trimmed.strip_prefix("gitdir:") {
                    let gitdir = rest.trim();
                    if !gitdir.is_empty() {
                        let resolved = if Path::new(gitdir).is_absolute() {
                            PathBuf::from(gitdir)
                        } else {
                            current.join(gitdir)
                        };
                        return Some(resolved);
                    }
                }
            }
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

fn read_branch(git_dir: &Path) -> Option<String> {
    let head_path = git_dir.join("HEAD");
    let contents = fs::read_to_string(&head_path).ok()?;
    let trimmed = contents.trim();
    let branch = trimmed.strip_prefix(HEAD_REF_PREFIX)?;
    if branch.is_empty() {
        return None;
    }
    Some(branch.to_string())
}