use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Program and arguments that can actually be started on this host.
///
/// Windows keeps its runnable scripts in `.cmd` and `.bat` files, which the
/// loader cannot execute on its own. They run through the command processor.
pub fn resolve(program: &str, args: &[String]) -> (OsString, Vec<OsString>) {
    #[cfg(windows)]
    {
        return resolve_windows(program, args);
    }
    #[cfg(not(windows))]
    {
        (
            OsString::from(program),
            args.iter().map(OsString::from).collect(),
        )
    }
}

#[cfg(windows)]
fn resolve_windows(program: &str, args: &[String]) -> (OsString, Vec<OsString>) {
    let passthrough: Vec<OsString> = args.iter().map(OsString::from).collect();
    let Some(full) = find_in_path(program) else {
        return (OsString::from(program), passthrough);
    };
    if !is_script(&full) {
        return (OsString::from(full), passthrough);
    }
    let processor = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
    let mut script_args = vec![OsString::from("/C"), OsString::from(&full)];
    script_args.extend(passthrough);
    (processor, script_args)
}

/// Find a program on this host.
pub fn find_in_path(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 {
        return Some(candidate.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let direct = dir.join(program);
        if direct.is_file() {
            return Some(direct);
        }
        for extension in crate::compat::paths::executable_extensions() {
            let with_extension = dir.join(format!("{program}{extension}"));
            if with_extension.is_file() {
                return Some(with_extension);
            }
        }
    }
    None
}

/// Whether the file is a script that needs the command processor.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_script(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        extension == "cmd" || extension == "bat"
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_keeps_the_program_on_unix() {
        let (program, args) = resolve("ls", &["-la".to_string()]);
        assert_eq!(program, OsString::from("ls"));
        assert_eq!(args, vec![OsString::from("-la")]);
    }

    #[test]
    fn find_in_path_locates_the_shell() {
        let found = find_in_path("sh");
        assert!(found.is_some(), "sh must be on PATH");
    }
}
