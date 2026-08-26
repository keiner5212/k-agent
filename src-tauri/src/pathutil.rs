use std::io;
use std::path::{Path, PathBuf};

pub fn canonicalize_path(path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
}

fn home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    None
}

fn unify_separators(raw: &str) -> String {
    raw.replace('\\', "/")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn strip_prefix_ci<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if path.len() >= prefix.len() && path[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&path[prefix.len()..])
    } else {
        None
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn windows_drive_path(drive: char, rest: &str) -> String {
    let rest = rest.trim_start_matches('/');
    if rest.is_empty() {
        format!("{}:/", drive.to_ascii_uppercase())
    } else {
        format!("{}:/{rest}", drive.to_ascii_uppercase())
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn remap_windows_prefix(path: &str) -> String {
    for prefix in ["/cygdrive/", "/mnt/"] {
        if let Some(rest) = strip_prefix_ci(path, prefix) {
            let mut chars = rest.chars();
            if let (Some(drive), Some('/')) = (chars.next(), chars.next()) {
                if drive.is_ascii_alphabetic() {
                    return windows_drive_path(drive, chars.as_str());
                }
            }
        }
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() {
        if bytes[2] == b'/' {
            return windows_drive_path(bytes[1] as char, &path[3..]);
        }
        if bytes[2] == b':' {
            return remap_windows_prefix(&path[1..]);
        }
    }
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        if bytes.len() == 2 {
            return windows_drive_path(bytes[0] as char, "");
        }
        if bytes[2] != b'/' {
            return windows_drive_path(bytes[0] as char, &path[2..]);
        }
    }
    path.to_string()
}

fn apply_windows_map(path: &str) -> String {
    #[cfg(windows)]
    {
        remap_windows_prefix(path)
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

pub fn expand_user(input: &str) -> PathBuf {
    let unified = unify_separators(input.trim());
    if unified == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = unified.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return if rest.is_empty() {
                home
            } else {
                home.join(rest)
            };
        }
    }
    PathBuf::from(apply_windows_map(&unified))
}

pub fn sanitize_path(raw: &str) -> PathBuf {
    expand_user(raw)
}

pub fn resolve_tool_path(raw: &str, workspace: Option<&Path>) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return workspace.map(Path::to_path_buf).ok_or_else(|| {
            "Workspace is not set; pass an absolute path or configure the workspace.".into()
        });
    }
    let candidate = sanitize_path(trimmed);
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    match workspace {
        Some(root) => Ok(root.join(candidate)),
        None => {
            Err("Workspace is not set; pass an absolute path or configure the workspace.".into())
        }
    }
}

pub fn workspace_from_app(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.state::<crate::LocalWorkspace>()
        .path
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_mixed_separators() {
        let path = sanitize_path(r"foo/bar\baz");
        assert_eq!(path, PathBuf::from("foo/bar/baz"));
    }

    #[test]
    fn expand_user_handles_home() {
        let original_home = std::env::var_os("HOME");
        let original_profile = std::env::var_os("USERPROFILE");
        std::env::set_var("HOME", "/tmp/home");
        std::env::remove_var("USERPROFILE");
        assert_eq!(expand_user("~"), PathBuf::from("/tmp/home"));
        assert_eq!(expand_user("~/docs"), PathBuf::from("/tmp/home/docs"));
        assert_eq!(expand_user(r"~\docs"), PathBuf::from("/tmp/home/docs"));
        assert_eq!(expand_user("/abs/path"), PathBuf::from("/abs/path"));
        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match original_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    }

    #[test]
    fn remap_windows_unix_prefixes() {
        assert_eq!(remap_windows_prefix("/c/Users/me"), "C:/Users/me");
        assert_eq!(remap_windows_prefix("/C:/Users/me"), "C:/Users/me");
        assert_eq!(remap_windows_prefix("/cygdrive/c/Users/me"), "C:/Users/me");
        assert_eq!(remap_windows_prefix("/mnt/c/Users/me"), "C:/Users/me");
        assert_eq!(remap_windows_prefix("C:foo/bar"), "C:/foo/bar");
        assert_eq!(remap_windows_prefix("C:/already"), "C:/already");
    }

    #[test]
    fn resolve_relative_joins_workspace() {
        let root = PathBuf::from("/workspace");
        let resolved = resolve_tool_path("src\\lib\\mod.rs", Some(&root)).unwrap();
        assert_eq!(resolved, root.join("src/lib/mod.rs"));
    }

    #[test]
    fn resolve_empty_uses_workspace() {
        let root = PathBuf::from("/workspace");
        let resolved = resolve_tool_path("  ", Some(&root)).unwrap();
        assert_eq!(resolved, root);
    }
}
